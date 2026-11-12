//! Typed follow-ups: things worth telling the person after an apply that are
//! not "an area still differs". They come from the status report (the login
//! shell) and from the apply log (relaunch hints, skipped plugin hooks); the
//! text and the copyable command are derived here, once.

use crate::status::{Note, StatusReport};

/// One follow-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUp {
    /// The login shell changed (or was not recorded): open a new window.
    OpenShell,
    /// macOS preferences changed; these apps read them at launch.
    Relaunch {
        /// Process names, for example `Dock`.
        apps: Vec<String>,
    },
    /// A plugin hook declined to overwrite an existing install.
    HookSkipped {
        /// The hook as the log names it.
        name: String,
    },
}

impl FollowUp {
    /// One line for the "Optional" section.
    pub fn text(&self) -> String {
        match self {
            Self::OpenShell => "Open a new terminal window to start using your shell".to_string(),
            Self::Relaunch { apps } => format!("Restart {} to show new settings", join_words(apps)),
            Self::HookSkipped { name } => {
                format!("Plugin hook skipped: {name} (your existing install was left as it is)")
            }
        }
    }

    /// The exact command to copy, when there is one.
    pub fn command(&self) -> Option<String> {
        match self {
            Self::Relaunch { apps } => Some(format!("killall {}", apps.join(" "))),
            Self::OpenShell | Self::HookSkipped { .. } => None,
        }
    }
}

fn join_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// Follow-ups the status report implies.
pub fn from_report(report: &StatusReport) -> Vec<FollowUp> {
    let mut out = Vec::new();
    if report.notes.contains(&Note::LoginShellNotRecorded) {
        out.push(FollowUp::OpenShell);
    }
    out
}

/// Follow-ups found in the apply log. Unrecognised lines are ignored.
pub fn from_log<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<FollowUp> {
    let mut out: Vec<FollowUp> = Vec::new();
    for line in lines {
        let found = relaunch(line).or_else(|| hook_skipped(line));
        if let Some(item) = found
            && !out.contains(&item)
        {
            out.push(item);
        }
    }
    out
}

/// `... relaunch apps that read changed macOS defaults (killall Dock / Finder)`.
fn relaunch(line: &str) -> Option<FollowUp> {
    let lower = line.to_ascii_lowercase();
    if !lower.contains("relaunch") {
        return None;
    }
    let after = &line[lower.find("killall")? + "killall".len()..];
    let list = after.split(')').next()?;
    let apps: Vec<String> = list
        .split(|c: char| c == '/' || c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|w| !w.is_empty() && *w != "and" && w.chars().all(char::is_alphanumeric))
        .map(str::to_string)
        .collect();
    (!apps.is_empty()).then_some(FollowUp::Relaunch { apps })
}

/// `PENDING <hook>: <reason>`.
fn hook_skipped(line: &str) -> Option<FollowUp> {
    let rest = &line[line.find("PENDING ")? + "PENDING ".len()..];
    let name = rest.split(':').next()?.trim();
    (!name.is_empty()).then(|| FollowUp::HookSkipped {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relaunch_line_becomes_a_typed_follow_up_with_an_exact_command() {
        let log = [
            "bootstrap: follow-up - relaunch apps that read changed macOS defaults (killall Dock / Finder / SystemUIServer)",
        ];
        let found = from_log(log);
        assert_eq!(
            found,
            vec![FollowUp::Relaunch {
                apps: vec!["Dock".into(), "Finder".into(), "SystemUIServer".into()]
            }]
        );
        assert_eq!(
            found[0].command().as_deref(),
            Some("killall Dock Finder SystemUIServer")
        );
        assert_eq!(
            found[0].text(),
            "Restart Dock, Finder and SystemUIServer to show new settings"
        );
    }

    #[test]
    fn pending_hook_lines_become_skipped_hooks_once() {
        let log = [
            "PENDING example-plugin/Tool: concurrent writes can lose changes; existing installations were not changed",
            "PENDING example-plugin/Tool: concurrent writes can lose changes; existing installations were not changed",
            "PENDING other-plugin/Tool: something",
        ];
        let found = from_log(log);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            FollowUp::HookSkipped {
                name: "example-plugin/Tool".into()
            }
        );
        assert!(found[0].command().is_none());
    }

    #[test]
    fn unrelated_lines_and_a_relaunch_without_apps_are_ignored() {
        assert!(from_log(["all installed", "relaunch something later"]).is_empty());
    }

    #[test]
    fn login_shell_note_becomes_open_shell() {
        let report = StatusReport::parse(r#"{"login_shell":null}"#, "t").unwrap();
        assert_eq!(from_report(&report), vec![FollowUp::OpenShell]);
        let none = StatusReport::parse("{}", "t").unwrap();
        assert!(from_report(&none).is_empty());
    }
}
