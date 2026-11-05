//! S4 Result: verify screen with drift table, Done gate, and re-check.
//!
//! State + logic live here; painting lands in T4.6. Entry auto-runs
//! [`Backend::status`](crate::backend::Backend::status); the drift
//! verdict joins plan entries (per-package, mapped `package`→`packages`)
//! with the status parts, and the per-part table is shaped for
//! [`DataGrid`](junie_tui::widgets::grid::DataGrid) (`set_rows` over
//! [`GridRows`](junie_tui::widgets::grid::GridRows)) — never the editable
//! `DataTable`. Follow-ups stay read-only and are never auto-executed.
//! `Done` exits 0 only when nothing drifts; `R` re-checks, `B` returns to
//! S2 (selection preserved by the caller), `c` copies the summary.

use std::collections::HashMap;

use junie_tui::theme::Tone;
use junie_tui::widgets::grid::{CellValue, ColumnSpec};
use junie_tui::widgets::props::Prop;

use super::rerun_fmt::rerun_command;
use crate::backend::{Action, Backend, PlanEntry, StatusSnapshot};

/// Per-part drift state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartState {
    /// Clean (status clean and no plan drift).
    Ok,
    /// Drifted (plan non-`noop` or status-derived drift).
    Drift,
}

/// One drift-table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRow {
    /// Part name (e.g. `packages`).
    pub part: String,
    /// Drift state.
    pub state: PartState,
    /// Human note (counts, pending specs, context).
    pub note: String,
}

/// Map a plan kind to its status part (`package`→`packages`, else 1:1).
pub fn plan_kind_to_part(kind: &str) -> &str {
    if kind == "package" { "packages" } else { kind }
}

/// Key intent on the result screen (§3: `R` re-checks, `B` back to S2,
/// `c` copies, `Enter`/`D` dones when all ok).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKey {
    /// Re-run `status --json` (+ plan) now.
    Recheck,
    /// Back to S2 (selection preserved by the caller).
    Back,
    /// Copy the summary.
    Copy,
    /// Done (only when [`ResultScreen::can_done`]).
    Done,
}

/// Screen-level action returned by [`ResultScreen::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultAction {
    /// State changed; stay on screen.
    None,
    /// Back to S2.
    Back,
    /// Done; the caller exits 0 (only reachable when all ok).
    Done,
}

/// S4 verify state. Owns the last `status` + `plan` snapshots.
pub struct ResultScreen {
    backend: std::sync::Arc<dyn Backend>,
    envs: Vec<String>,
    keep: Vec<String>,
    parts: Vec<PartRow>,
    summary: String,
    follow_ups: Vec<String>,
    checks: u32,
    last_error: Option<String>,
    last_drift: Vec<String>,
}

impl ResultScreen {
    /// Build for the selected envs and the S2 keep-list (for reruns).
    pub fn new(backend: std::sync::Arc<dyn Backend>, envs: Vec<String>, keep: Vec<String>) -> Self {
        Self {
            backend,
            envs,
            keep,
            parts: Vec::new(),
            summary: String::new(),
            follow_ups: Vec::new(),
            checks: 0,
            last_error: None,
            last_drift: Vec::new(),
        }
    }

    /// Auto-run on entry: `status --json` plus plan, rebuilding the table.
    /// A backend failure is recorded, never panics.
    pub fn refresh(&mut self, backend: &dyn Backend) {
        match backend.status(&self.envs) {
            Ok(snapshot) => match backend.plan(&self.envs, None) {
                Ok(plan) => {
                    self.rebuild(&snapshot, &plan);
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(format!("plan failed: {e}"));
                }
            },
            Err(e) => {
                self.last_error = Some(format!("status failed: {e}"));
            }
        }
        self.checks += 1;
    }

    /// Map a key to an intent (`Enter`/`D` only fire when all ok).
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<ResultKey> {
        use ratatui::crossterm::event::KeyCode;
        if key.is_char('R') || key.is_char('r') {
            return Some(ResultKey::Recheck);
        }
        if key.is_char('B') || key.is_char('b') {
            return Some(ResultKey::Back);
        }
        if key.is_char('c') {
            return Some(ResultKey::Copy);
        }
        if (key.is(KeyCode::Enter) || key.is_char('D') || key.is_char('d')) && self.can_done() {
            return Some(ResultKey::Done);
        }
        None
    }

    /// Apply an intent. `Recheck` re-runs against the given backend.
    pub fn apply(&mut self, key: ResultKey, backend: &dyn Backend) -> ResultAction {
        match key {
            ResultKey::Recheck => {
                self.refresh(backend);
                ResultAction::None
            }
            ResultKey::Back => ResultAction::Back,
            ResultKey::Copy => ResultAction::None,
            ResultKey::Done => {
                if self.can_done() {
                    ResultAction::Done
                } else {
                    ResultAction::None
                }
            }
        }
    }

    /// Done is enabled only when nothing drifts.
    pub fn can_done(&self) -> bool {
        !self.parts.is_empty() && self.parts.iter().all(|p| p.state == PartState::Ok)
    }

    /// Drifted kept specs (`manager:name`) for the rerun command.
    /// Only kept packages count: omitted entries stay omitted (one-shot).
    pub fn drifted_specs(&self) -> Vec<String> {
        self.last_drift.clone()
    }

    /// Exact rerun command for the drifted subset, if any.
    pub fn rerun(&self) -> Option<String> {
        self.rerun_text()
    }

    /// Summary line (`✓ N of M parts ok · K drift`).
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Read-only follow-ups (never auto-executed).
    pub fn follow_ups(&self) -> &[String] {
        &self.follow_ups
    }

    /// Drift-table rows, in part order.
    pub fn parts(&self) -> &[PartRow] {
        &self.parts
    }

    /// Re-check count.
    pub fn checks(&self) -> u32 {
        self.checks
    }

    /// Last backend error, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Grid columns for the drift table (`part`, `state`, `note`).
    pub fn grid_columns() -> Vec<ColumnSpec> {
        use junie_tui::widgets::grid::CellKind;
        vec![
            ColumnSpec::new("part", CellKind::Text),
            ColumnSpec::new("state", CellKind::Text),
            ColumnSpec::new("note", CellKind::Text),
        ]
    }

    /// Grid rows for the drift table, matching [`grid_columns`](Self::grid_columns).
    pub fn grid_rows(&self) -> Vec<Vec<CellValue>> {
        self.parts
            .iter()
            .map(|p| {
                vec![
                    CellValue::Text(p.part.clone()),
                    CellValue::Text(
                        match p.state {
                            PartState::Ok => "ok",
                            PartState::Drift => "drift",
                        }
                        .to_string(),
                    ),
                    CellValue::Text(p.note.clone()),
                ]
            })
            .collect()
    }

    /// Follow-ups as props for the follow-up pane.
    pub fn follow_up_props(&self) -> Vec<Prop> {
        self.follow_ups
            .iter()
            .map(|f| {
                let mut prop = Prop::new("follow-up", f);
                prop.tone = Tone::Secondary;
                prop
            })
            .collect()
    }

    /// Copy the summary through the shell clipboard (OSC52).
    pub fn copy_summary(
        &self,
        clipboard: &junie_tui::shell::clipboard::Clipboard,
    ) -> junie_tui::shell::clipboard::Copied {
        clipboard.copy(&self.summary)
    }

    fn rebuild(&mut self, snapshot: &StatusSnapshot, plan: &[PlanEntry]) {
        // Only kept packages count: omitted entries stay omitted (one-shot),
        // so they never gate Done or enter the rerun.
        let relevant: Vec<&PlanEntry> = plan
            .iter()
            .filter(|e| self.keep.contains(&plan_spec(e)))
            .collect();
        // Plan drift by part.
        let mut plan_drift: HashMap<String, Vec<String>> = HashMap::new();
        for e in &relevant {
            if e.action != Action::Noop {
                plan_drift
                    .entry(plan_kind_to_part(&e.kind).to_string())
                    .or_default()
                    .push(plan_spec(e));
            }
        }
        // Status states by part.
        let mut status_state: HashMap<String, (String, Vec<String>)> = HashMap::new();
        for p in &snapshot.parts {
            status_state.insert(p.part.clone(), (p.state.clone(), p.drift.clone()));
        }
        // Union of parts, sorted for determinism.
        let mut names: Vec<String> = status_state.keys().cloned().collect();
        for part in plan_drift.keys() {
            if !status_state.contains_key(part) {
                names.push(part.clone());
            }
        }
        names.sort();
        names.dedup();
        let mut parts = Vec::with_capacity(names.len());
        let mut drifted = 0usize;
        for name in &names {
            let drift_specs = plan_drift.get(name).cloned().unwrap_or_default();
            let (status, notes) = status_state.get(name).cloned().unwrap_or_default();
            let (state, note) = if !drift_specs.is_empty() {
                let mut note = format!("{} pending", drift_specs.len());
                if !notes.is_empty() && notes != ["clean"] && notes != ["in sync"] {
                    note.push_str(&format!(" ({})", notes.join("; ")));
                }
                (PartState::Drift, note)
            } else if status == "drift" {
                (PartState::Drift, notes.join("; "))
            } else {
                (PartState::Ok, status_note(name, &notes))
            };
            if state == PartState::Drift {
                drifted += 1;
            }
            parts.push(PartRow {
                part: name.clone(),
                state,
                note,
            });
        }
        let ok = parts.len() - drifted;
        self.summary = format!("✓ {ok} of {} parts ok · {drifted} drift", parts.len());
        self.last_drift = plan_drift.values().flatten().cloned().collect();
        self.follow_ups = follow_ups(snapshot, drifted, self.rerun_text());
        self.parts = parts;
    }

    fn rerun_text(&self) -> Option<String> {
        if self.last_drift.is_empty() {
            return None;
        }
        let mut specs = self.last_drift.clone();
        specs.sort();
        let mut phase = vec![
            "bootstrap".to_string(),
            "packages".to_string(),
            "apply".to_string(),
        ];
        phase.extend(specs);
        Some(rerun_command(
            self.backend.mise_bin(),
            self.backend.config_dir(),
            &self.envs,
            &phase,
        ))
    }
}

/// Map a plan entry to its keep-comparable spec: package names are
/// already `manager:name` qualified, other kinds are `kind:name`.
fn plan_spec(e: &PlanEntry) -> String {
    if e.kind == "package" {
        e.name.clone()
    } else {
        format!("{}:{}", e.kind, e.name)
    }
}

/// Status context note for a clean part.
fn status_note(part: &str, notes: &[String]) -> String {
    if notes.is_empty() || notes == ["clean"] {
        if part == "packages" {
            return "in sync".to_string();
        }
        return "clean".to_string();
    }
    notes.join("; ")
}

/// Read-only follow-ups: fresh login shell when unrecorded, rerun when
/// drifted. Surfaced, never auto-executed.
fn follow_ups(snapshot: &StatusSnapshot, drifted: usize, rerun: Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    if snapshot
        .parts
        .iter()
        .any(|p| p.part == "login_shell" && p.drift.iter().any(|d| d == "not recorded"))
    {
        out.push("open a fresh login shell to verify activation".to_string());
    }
    if drifted > 0 {
        if let Some(cmd) = rerun {
            out.push(format!("rerun: {cmd}"));
        } else {
            out.push("re-check drifted parts, then re-apply".to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MiseBackend;
    use crate::backend::mock::MockBackend;
    use junie_tui::core::event::Key;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn test_backend() -> std::sync::Arc<dyn Backend> {
        std::sync::Arc::new(MiseBackend::new(
            std::path::PathBuf::from("/tmp/emc-test-result"),
            vec!["terminal".to_string()],
        ))
    }

    fn result() -> ResultScreen {
        ResultScreen::new(
            test_backend(),
            vec!["terminal".to_string()],
            vec!["brew-cask:jetbrains-toolbox".to_string()],
        )
    }

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: KeyModifiers::empty(),
        }
    }

    fn char_key(c: char) -> Key {
        Key {
            code: KeyCode::Char(c),
            mods: KeyModifiers::empty(),
        }
    }

    #[test]
    fn refresh_clean_fixture_enables_done() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(screen.checks(), 1);
        assert!(screen.last_error().is_none());
        assert!(!screen.parts().is_empty());
        assert!(screen.can_done(), "clean fixture has no drift");
        assert!(screen.summary().contains("0 drift"));
        assert!(screen.rerun().is_none());
        assert!(
            screen
                .follow_ups()
                .iter()
                .any(|f| f.contains("fresh login shell")),
            "unrecorded login shell surfaces a follow-up"
        );
        assert_eq!(ResultScreen::grid_columns().len(), 3);
        assert_eq!(screen.grid_rows().len(), screen.parts().len());
        assert!(!screen.follow_up_props().is_empty());
        let copied = screen.copy_summary(&junie_tui::shell::clipboard::Clipboard::new());
        assert!(
            matches!(copied, junie_tui::shell::clipboard::Copied::Copied { .. }),
            "summary copies through the shell clipboard"
        );
    }

    #[test]
    fn drift_blocks_done_with_rerun() {
        let mut screen = result();
        let snapshot =
            crate::backend::snapshot_from_raw(r#"{"packages": {"brew": {"packages": []}}}"#);
        let plan = vec![PlanEntry {
            kind: "package".to_string(),
            name: "brew:jq".to_string(),
            current: "absent".to_string(),
            desired: "present".to_string(),
            action: Action::Create,
        }];
        // Keep the drifted package: it gates Done and enters the rerun.
        screen.keep = vec!["brew:jq".to_string()];
        screen.rebuild(&snapshot, &plan);
        let packages = screen
            .parts()
            .iter()
            .find(|p| p.part == "packages")
            .expect("packages row");
        assert_eq!(packages.state, PartState::Drift);
        assert!(!screen.can_done());
        assert_eq!(
            screen.map_key(&key(KeyCode::Enter)),
            None,
            "Done gated while drifted"
        );
        let rerun = screen.rerun().expect("drifted rerun");
        assert!(
            rerun.contains("packages apply brew:jq"),
            "rerun is exact: {rerun}"
        );
        assert!(screen.follow_ups().iter().any(|f| f.contains("rerun:")));
        assert_eq!(screen.drifted_specs(), vec!["brew:jq".to_string()]);
    }

    #[test]
    fn omitted_packages_never_gate_done() {
        let mut screen = result();
        // Empty keep: a non-noop plan entry for an omitted package is out
        // of scope for this one-shot run.
        screen.keep = Vec::new();
        let snapshot = crate::backend::snapshot_from_raw(r#"{"files": []}"#);
        let plan = vec![PlanEntry {
            kind: "package".to_string(),
            name: "brew:jq".to_string(),
            current: "absent".to_string(),
            desired: "present".to_string(),
            action: Action::Create,
        }];
        screen.rebuild(&snapshot, &plan);
        assert!(screen.can_done());
        assert!(screen.rerun().is_none());
    }

    #[test]
    fn keys_match_section_3() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(screen.map_key(&char_key('R')), Some(ResultKey::Recheck));
        assert_eq!(screen.map_key(&char_key('r')), Some(ResultKey::Recheck));
        assert_eq!(screen.map_key(&char_key('B')), Some(ResultKey::Back));
        assert_eq!(screen.map_key(&char_key('c')), Some(ResultKey::Copy));
        assert_eq!(screen.map_key(&key(KeyCode::Enter)), Some(ResultKey::Done));
        assert_eq!(screen.map_key(&char_key('D')), Some(ResultKey::Done));
        assert_eq!(screen.apply(ResultKey::Back, &mock), ResultAction::Back);
        assert_eq!(screen.apply(ResultKey::Done, &mock), ResultAction::Done);
    }

    #[test]
    fn recheck_reruns_backend() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(screen.apply(ResultKey::Recheck, &mock), ResultAction::None);
        assert_eq!(screen.checks(), 2);
    }

    #[test]
    fn plan_kind_mapping() {
        assert_eq!(plan_kind_to_part("package"), "packages");
        assert_eq!(plan_kind_to_part("files"), "files");
    }
}
