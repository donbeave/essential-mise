//! Applying again, but only what still differs.
//!
//! The plan is built from a [`StatusReport`]: pending packages go to
//! `bootstrap packages apply <specs>`, other pending areas go to
//! `bootstrap --only <names>` through the explicit status-key to `--only`
//! table in [`AreaKey`]. An area that has no `--only` name makes the base
//! phase run in full (`--skip packages`) instead of guessing a name.
//!
//! The command the person sees and copies is formatted from the same argv the
//! backend executes ([`Backend::phase1_argv`] / [`Backend::phase2_argv`]), so
//! the two cannot drift apart.

use crate::backend::{ApplyRequest, Backend};
use crate::screens::rerun_fmt::rerun_command;
use crate::status::{AreaKey, StatusReport};

/// What a re-apply will run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapplyPlan {
    /// The request handed to the backend.
    pub request: ApplyRequest,
    /// Pending areas that have no `--only` name (they force a full base phase).
    pub unmapped: Vec<AreaKey>,
}

/// Build the re-apply for every area that needs changes, or `None` when
/// nothing does.
pub fn plan(report: &StatusReport, envs: &[String]) -> Option<ReapplyPlan> {
    let pending = report.needs_changes();
    if pending.is_empty() {
        return None;
    }
    let mut specs: Vec<String> = Vec::new();
    let mut only: Vec<String> = Vec::new();
    let mut unmapped = Vec::new();
    for area in pending {
        if area.key == AreaKey::Packages {
            specs.extend(area.changes().into_iter().filter_map(|e| e.spec.clone()));
            continue;
        }
        match area.key.only_name() {
            Some(name) => {
                if !only.iter().any(|n| n == name) {
                    only.push(name.to_string());
                }
            }
            None => unmapped.push(area.key),
        }
    }
    specs.sort();
    let base_pending = !only.is_empty() || !unmapped.is_empty();
    let request = ApplyRequest {
        envs: envs.to_vec(),
        keep: specs,
        manager: None,
        skip_parts: Vec::new(),
        // An unmapped area means the whole base phase runs.
        only_parts: if unmapped.is_empty() {
            only
        } else {
            Vec::new()
        },
        run_base: base_pending,
        // A re-apply fixes what differs; refreshing and upgrading everything
        // again is not what was asked for.
        update: false,
        upgrade: false,
        dry_run: false,
    };
    Some(ReapplyPlan { request, unmapped })
}

/// The commands a plan runs, one per phase, exactly as the backend runs them.
pub fn commands(backend: &dyn Backend, request: &ApplyRequest) -> Vec<String> {
    let mut lines = Vec::new();
    if request.run_base {
        lines.push(rerun_command(
            backend.mise_bin(),
            backend.config_dir(),
            &request.envs,
            &backend.phase1_argv(request),
        ));
    }
    if let Some(argv) = backend.phase2_argv(request) {
        lines.push(rerun_command(
            backend.mise_bin(),
            backend.config_dir(),
            &request.envs,
            &argv,
        ));
    }
    lines
}

/// The commands as one line to copy: phases joined with `&&`, so the second
/// only runs if the first succeeded (as the console does).
pub fn command_line(commands: &[String]) -> String {
    commands.join(" && ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MiseBackend;

    fn report(raw: &str) -> StatusReport {
        StatusReport::parse(raw, "t").expect("parses")
    }

    const DEFAULTS_AND_PACKAGE: &str = r#"{
      "macos_defaults":{"entries":[{"domain":"d","key":"k","value":1,"current":"2","state":"differs"}]},
      "dotfiles":{"files":[{"target":"~/a","state":"not applied"}],"edits":[]},
      "packages":{"brew":{"available":true,"packages":[
        {"package":"b","desired_state":"present","state":"missing"},
        {"package":"a","desired_state":"present","state":"missing"}]}}
    }"#;

    #[test]
    fn pending_areas_map_to_only_names_and_package_specs() {
        let plan = plan(&report(DEFAULTS_AND_PACKAGE), &["alpha".to_string()]).expect("pending");
        assert_eq!(plan.request.only_parts, vec!["dotfiles", "macos-defaults"]);
        assert_eq!(plan.request.keep, vec!["brew:a", "brew:b"]);
        assert!(plan.request.run_base);
        assert!(!plan.request.update && !plan.request.upgrade);
        assert!(plan.unmapped.is_empty());
    }

    #[test]
    fn packages_only_skips_the_base_phase() {
        let raw = r#"{"packages":{"brew":{"available":true,"packages":[
          {"package":"a","desired_state":"present","state":"missing"}]}}}"#;
        let plan = plan(&report(raw), &["alpha".to_string()]).expect("pending");
        assert!(!plan.request.run_base);
        assert_eq!(plan.request.keep, vec!["brew:a"]);
    }

    #[test]
    fn an_area_without_an_only_name_runs_the_whole_base_phase() {
        let raw = r#"{"macos_login_items":{"entries":[{"path":"/Applications/Example App.app","state":"unset"}]},
          "macos_defaults":{"entries":[{"domain":"d","key":"k","value":1,"current":"2","state":"differs"}]}}"#;
        let plan = plan(&report(raw), &["alpha".to_string()]).expect("pending");
        assert_eq!(plan.unmapped, vec![AreaKey::LoginItems]);
        assert!(plan.request.only_parts.is_empty(), "no partial --only");
        assert!(plan.request.run_base);
    }

    #[test]
    fn nothing_pending_means_no_plan() {
        assert!(plan(&report(r#"{"tools":[{"tool":"a","installed":true}]}"#), &[]).is_none());
    }

    #[test]
    fn every_status_key_maps_to_an_only_name_or_is_declared_full_apply() {
        for key in AreaKey::all() {
            match key.only_name() {
                Some(name) => assert!(!name.contains('_'), "{name} is a --only name"),
                None => assert!(
                    [
                        AreaKey::Secrets,
                        AreaKey::UserServices,
                        AreaKey::LoginItems,
                        AreaKey::PluginDeps
                    ]
                    .contains(&key),
                    "{key:?} has no --only name and is not declared full-apply"
                ),
            }
        }
    }

    #[test]
    fn shown_command_equals_the_executed_one() {
        let backend = MiseBackend::new("/Users/test/.config/mise").with_mise_bin("mise");
        let plan = plan(&report(DEFAULTS_AND_PACKAGE), &["alpha".to_string()]).expect("pending");
        let shown = commands(&backend, &plan.request);
        assert_eq!(
            shown,
            vec![
                "mise -C /Users/test/.config/mise -E alpha bootstrap --only dotfiles,macos-defaults",
                "mise -C /Users/test/.config/mise -E alpha bootstrap packages apply brew:a brew:b",
            ]
        );
        // The dry-run preview prints the argv the worker executes; it must
        // match what is shown, line for line (the preview only adds --dry-run).
        let (tx, rx) = junie_tui::runtime::external_channel();
        let mut dry = plan.request.clone();
        dry.dry_run = true;
        backend
            .spawn_apply(&dry, tx)
            .expect("dry run starts")
            .join();
        let executed: Vec<String> = rx
            .drain()
            .into_iter()
            .filter_map(|m| {
                m.downcast_ref::<crate::backend::ApplyLine>()
                    .map(|l| l.text.clone())
            })
            .filter_map(|t| {
                t.strip_prefix("dry-run: ")
                    .map(|c| c.replace(" --dry-run", ""))
            })
            .collect();
        assert_eq!(executed, shown);
        assert_eq!(command_line(&shown), shown.join(" && "));
    }
}
