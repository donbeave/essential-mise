//! Typed view of `mise bootstrap status --json`.
//!
//! `status --json` is an *inventory*: every configured entry is listed, the
//! satisfied ones included (`--missing` only changes the exit code). This
//! module reduces each area to per-entry verdicts, so "up to date" means
//! every entry reports a satisfied state and "needs changes" means at least
//! one entry really differs. A shape or state string that is not recognised
//! is [`Verdict::Unrecognized`] (shown as "could not check"), never a
//! difference.
//!
//! [`AreaKey`] is also the single table that maps a status key to its
//! user-facing label and to the `--only` name of `mise bootstrap`.

use serde_json::Value;

/// One kind of thing a module manages (an "area" in the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AreaKey {
    /// Secret inputs.
    Secrets,
    /// Homebrew and other package managers.
    Packages,
    /// System users and groups.
    Accounts,
    /// System files and directories.
    Files,
    /// System services.
    Services,
    /// User-level services.
    UserServices,
    /// Firewall rules.
    Firewall,
    /// Compose projects.
    Compose,
    /// Git repositories.
    Repos,
    /// Dotfiles and file edits.
    Dotfiles,
    /// Shell activation lines.
    ShellActivation,
    /// macOS login items.
    LoginItems,
    /// macOS preferences.
    MacosDefaults,
    /// macOS LaunchAgents.
    LaunchAgents,
    /// systemd user units.
    SystemdUnits,
    /// The login shell.
    LoginShell,
    /// Tools managed by mise.
    Tools,
    /// System dependencies of tools.
    PluginDeps,
    /// A top-level key this console does not know.
    Other,
}

/// Every known area with its `status --json` key, `--only` name and label.
/// `only` is `None` where `mise bootstrap --only` has no matching name; a
/// re-apply that touches such an area runs the whole base phase instead.
const AREAS: &[(AreaKey, &str, Option<&str>, &str)] = &[
    (AreaKey::Secrets, "secrets", None, "Secrets"),
    (
        AreaKey::Packages,
        "packages",
        Some("packages"),
        "Apps and packages",
    ),
    (
        AreaKey::Accounts,
        "accounts",
        Some("accounts"),
        "User accounts",
    ),
    (AreaKey::Files, "files", Some("files"), "System files"),
    (
        AreaKey::Services,
        "services",
        Some("services"),
        "System services",
    ),
    (
        AreaKey::UserServices,
        "user_services",
        None,
        "User services",
    ),
    (
        AreaKey::Firewall,
        "firewall",
        Some("firewall"),
        "Firewall rules",
    ),
    (
        AreaKey::Compose,
        "compose",
        Some("compose"),
        "Compose projects",
    ),
    (AreaKey::Repos, "repos", Some("repos"), "Git repositories"),
    (AreaKey::Dotfiles, "dotfiles", Some("dotfiles"), "Dotfiles"),
    (
        AreaKey::ShellActivation,
        "mise_shell_activate",
        Some("mise-shell-activate"),
        "Shell activation",
    ),
    (
        AreaKey::LoginItems,
        "macos_login_items",
        None,
        "Login items",
    ),
    (
        AreaKey::MacosDefaults,
        "macos_defaults",
        Some("macos-defaults"),
        "macOS settings",
    ),
    (
        AreaKey::LaunchAgents,
        "launchd",
        Some("macos-launchd-agents"),
        "Launch agents",
    ),
    (
        AreaKey::SystemdUnits,
        "systemd",
        Some("linux-systemd-units"),
        "Systemd units",
    ),
    (
        AreaKey::LoginShell,
        "login_shell",
        Some("user"),
        "Login shell",
    ),
    (AreaKey::Tools, "tools", Some("tools"), "Command-line tools"),
    (
        AreaKey::PluginDeps,
        "plugin_deps",
        None,
        "Tool dependencies",
    ),
];

impl AreaKey {
    /// Every known area, in display order.
    pub fn all() -> impl Iterator<Item = Self> {
        AREAS.iter().map(|row| row.0)
    }

    /// Resolve a top-level `status --json` key.
    pub fn from_status_key(key: &str) -> Self {
        AREAS
            .iter()
            .find(|row| row.1 == key)
            .map_or(Self::Other, |row| row.0)
    }

    fn row(self) -> Option<&'static (AreaKey, &'static str, Option<&'static str>, &'static str)> {
        AREAS.iter().find(|row| row.0 == self)
    }

    /// The key `status --json` uses for this area.
    pub fn status_key(self) -> &'static str {
        self.row().map_or("", |row| row.1)
    }

    /// The `mise bootstrap --only` name, when one exists.
    pub fn only_name(self) -> Option<&'static str> {
        self.row().and_then(|row| row.2)
    }

    /// User-facing label (internal ids are never shown).
    pub fn label(self) -> &'static str {
        self.row().map_or("Other settings", |row| row.3)
    }
}

/// How one inventory entry compares with the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The entry already matches (for example a default that is already set).
    Satisfied,
    /// The entry really differs and applying would change it.
    Differs,
    /// The state string is not one this console understands.
    Unrecognized,
}

/// One inventory entry of an area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Human name, for example `com.apple.dock tilesize` or `Example App`.
    pub item: String,
    /// Package spec (`manager:name`) for package entries.
    pub spec: Option<String>,
    /// How it compares with the configuration.
    pub verdict: Verdict,
    /// Observed value, in words.
    pub now: String,
    /// Configured value, in words.
    pub wanted: String,
    /// The person chose to skip this package this time.
    pub skipped: bool,
}

/// One area's inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaStatus {
    /// Which area.
    pub key: AreaKey,
    /// Every configured entry, satisfied ones included.
    pub entries: Vec<Entry>,
    /// The whole area has a shape this console does not understand.
    pub malformed: bool,
    /// For [`AreaKey::Other`]: the raw key, kept for the details view only.
    pub raw_key: Option<String>,
}

/// Aggregate state of an area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Every entry matches.
    UpToDate,
    /// At least one entry differs.
    NeedsChanges,
    /// Nothing differs, but something could not be interpreted.
    Unknown,
    /// Everything in the area was skipped by the person.
    Skipped,
}

impl AreaStatus {
    /// Entries that count (not skipped by the person).
    pub fn counted(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| !e.skipped)
    }

    /// Number of counted entries.
    pub fn total(&self) -> usize {
        self.counted().count()
    }

    /// Entries that really differ.
    pub fn changes(&self) -> Vec<&Entry> {
        self.counted()
            .filter(|e| e.verdict == Verdict::Differs)
            .collect()
    }

    /// Number of package entries the person skipped.
    pub fn skipped(&self) -> usize {
        self.entries.iter().filter(|e| e.skipped).count()
    }

    /// Items that could not be interpreted.
    pub fn unrecognized(&self) -> Vec<&Entry> {
        self.counted()
            .filter(|e| e.verdict == Verdict::Unrecognized)
            .collect()
    }

    /// Aggregate state: differences win, then unknowns, then skipped.
    pub fn health(&self) -> Health {
        if !self.changes().is_empty() {
            Health::NeedsChanges
        } else if self.malformed || !self.unrecognized().is_empty() {
            Health::Unknown
        } else if self.total() == 0 && self.skipped() > 0 {
            Health::Skipped
        } else {
            Health::UpToDate
        }
    }
}

/// Something the person may want to know that is not an area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// `login_shell` is `null`: mise did not record a login-shell change.
    LoginShellNotRecorded,
}

/// A typed `status --json` document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    /// Configured areas only (areas with no entries are left out).
    pub areas: Vec<AreaStatus>,
    /// Non-area notes.
    pub notes: Vec<Note>,
    /// Wall-clock time of the check, `HH:MM:SS`.
    pub checked_at: String,
}

/// `status --json` was not a JSON object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusParseError(pub String);

impl std::fmt::Display for StatusParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "status --json: {}", self.0)
    }
}

impl std::error::Error for StatusParseError {}

impl StatusReport {
    /// Parse raw `status --json` text. Never panics; an unrecognised area is
    /// kept as "could not check" instead of being dropped or counted as a
    /// difference.
    pub fn parse(raw: &str, checked_at: impl Into<String>) -> Result<Self, StatusParseError> {
        let value: Value =
            serde_json::from_str(raw).map_err(|e| StatusParseError(e.to_string()))?;
        let Some(object) = value.as_object() else {
            return Err(StatusParseError("expected a JSON object".to_string()));
        };
        let mut areas = Vec::new();
        let mut notes = Vec::new();
        for (key, value) in object {
            let area = AreaKey::from_status_key(key);
            if area == AreaKey::LoginShell && value.is_null() {
                notes.push(Note::LoginShellNotRecorded);
                continue;
            }
            if let Some(mut status) = parse_area(area, value) {
                if area == AreaKey::Other {
                    status.raw_key = Some(key.clone());
                }
                areas.push(status);
            }
        }
        let order = |key: AreaKey| AreaKey::all().position(|k| k == key).unwrap_or(usize::MAX);
        areas.sort_by_key(|a| order(a.key));
        Ok(Self {
            areas,
            notes,
            checked_at: checked_at.into(),
        })
    }

    /// Mark packages outside `keep` (`manager:name`) as skipped this time.
    /// They no longer count towards totals or differences.
    #[must_use]
    pub fn scoped(mut self, keep: &[String]) -> Self {
        for area in &mut self.areas {
            if area.key != AreaKey::Packages {
                continue;
            }
            for entry in &mut area.entries {
                if let Some(spec) = &entry.spec {
                    entry.skipped = !keep.contains(spec);
                }
            }
        }
        self
    }

    /// Areas that really differ.
    pub fn needs_changes(&self) -> Vec<&AreaStatus> {
        self.areas
            .iter()
            .filter(|a| a.health() == Health::NeedsChanges)
            .collect()
    }

    /// Areas that could not be interpreted (and nothing else differs there).
    pub fn unknown(&self) -> Vec<&AreaStatus> {
        self.areas
            .iter()
            .filter(|a| a.health() == Health::Unknown)
            .collect()
    }

    /// Areas that are up to date (or skipped by the person).
    pub fn up_to_date(&self) -> Vec<&AreaStatus> {
        self.areas
            .iter()
            .filter(|a| matches!(a.health(), Health::UpToDate | Health::Skipped))
            .collect()
    }

    /// Whether nothing differs anywhere.
    pub fn is_clean(&self) -> bool {
        self.needs_changes().is_empty()
    }
}

/// Satisfied state strings (`set`, `applied`, `current`, `installed`, ...).
const SATISFIED: &[&str] = &[
    "set",
    "applied",
    "current",
    "installed",
    "present",
    "loaded",
    "running",
    "enabled",
    "active",
    "ok",
    "up-to-date",
    "up_to_date",
];

/// State strings that mean "applying would change this". The wording is
/// taken from strings in `mise`; entries not listed here are
/// [`Verdict::Unrecognized`] rather than guessed.
const DIFFERS: &[&str] = &[
    "unset",
    "differs",
    "missing",
    "not_set",
    "not-set",
    "not set",
    "drifted",
    "outdated",
    "behind",
    "dirty",
    "diverged",
    "not_installed",
    "not-installed",
    "not installed",
    "absent",
    "unloaded",
    "not_loaded",
    "not loaded",
    "stopped",
    "disabled",
    "inactive",
    "needs_repair",
    "needs repair",
    "not_applied",
    "not applied",
    "modified",
    "conflict",
    "not_cloned",
    "not cloned",
];

fn classify(state: Option<&str>) -> Verdict {
    match state {
        Some(s) if SATISFIED.contains(&s) => Verdict::Satisfied,
        Some(s) if DIFFERS.contains(&s) => Verdict::Differs,
        _ => Verdict::Unrecognized,
    }
}

fn humanize(state: &str) -> String {
    state.replace(['_', '-'], " ")
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Render a JSON scalar for the "now / wanted" columns.
fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "unset".to_string(),
        other => other.to_string(),
    }
}

fn array<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    value.get(key).and_then(Value::as_array)
}

/// Reduce one area; `None` means "nothing configured here".
fn parse_area(key: AreaKey, value: &Value) -> Option<AreaStatus> {
    let entries: Result<Vec<Entry>, ()> = match key {
        AreaKey::Packages => packages(value),
        AreaKey::Tools => tools(value),
        AreaKey::Dotfiles => dotfiles(value),
        AreaKey::Repos => list(value).map(|rows| rows.into_iter().map(repo).collect()),
        AreaKey::MacosDefaults => keyed(value, "entries", defaults_entry),
        AreaKey::LoginItems => keyed(value, "entries", login_item),
        AreaKey::LaunchAgents => keyed(value, "agents", generic_entry),
        AreaKey::SystemdUnits => keyed(value, "units", generic_entry),
        _ => list(value).map(|rows| rows.into_iter().map(generic_entry).collect()),
    };
    match entries {
        Ok(entries) if entries.is_empty() => None,
        Ok(entries) => Some(AreaStatus {
            key,
            entries,
            malformed: false,
            raw_key: None,
        }),
        Err(()) => Some(AreaStatus {
            key,
            entries: Vec::new(),
            malformed: true,
            raw_key: None,
        }),
    }
}

/// `null` is empty; an array is its rows; an object is the rows of every
/// array it holds; anything else is a shape this console does not know.
fn list(value: &Value) -> Result<Vec<&Value>, ()> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(rows) => Ok(rows.iter().collect()),
        Value::Object(map) => Ok(map.values().filter_map(Value::as_array).flatten().collect()),
        _ => Err(()),
    }
}

fn keyed(value: &Value, field: &str, entry: fn(&Value) -> Entry) -> Result<Vec<Entry>, ()> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Object(_) => Ok(array(value, field)
            .map(|rows| rows.iter().map(entry).collect())
            .unwrap_or_default()),
        _ => Err(()),
    }
}

fn entry(item: String, state: Option<&str>, now: String, wanted: &str) -> Entry {
    Entry {
        item,
        spec: None,
        verdict: classify(state),
        now,
        wanted: wanted.to_string(),
        skipped: false,
    }
}

fn state_words(row: &Value) -> String {
    text(row, "state").map_or_else(|| "unknown".to_string(), |s| humanize(&s))
}

fn generic_entry(row: &Value) -> Entry {
    let name = ["name", "label", "id", "path", "target", "unit", "package"]
        .iter()
        .find_map(|k| text(row, k))
        .unwrap_or_else(|| "item".to_string());
    entry(name, text(row, "state").as_deref(), state_words(row), "ok")
}

fn repo(row: &Value) -> Entry {
    let name = text(row, "path_raw")
        .or_else(|| text(row, "path"))
        .unwrap_or_else(|| "repository".to_string());
    let state = text(row, "state");
    let now = match text(row, "reason").filter(|r| !r.is_empty()) {
        Some(reason) => reason,
        None => state_words(row),
    };
    entry(name, state.as_deref(), now, "current")
}

fn login_item(row: &Value) -> Entry {
    let path = text(row, "path").unwrap_or_default();
    let name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("login item")
        .to_string();
    let state = text(row, "state");
    let now = match state.as_deref() {
        Some("unset" | "missing" | "not_set" | "absent") => "missing".to_string(),
        _ => state_words(row),
    };
    entry(name, state.as_deref(), now, "present")
}

fn defaults_entry(row: &Value) -> Entry {
    let domain = text(row, "domain").unwrap_or_default();
    let key = text(row, "key").unwrap_or_default();
    let item = format!("{domain} {key}").trim().to_string();
    let item = if item.is_empty() {
        "setting".to_string()
    } else {
        item
    };
    let now = row.get("current").map_or_else(|| state_words(row), scalar);
    let wanted = row.get("value").map_or_else(|| "set".to_string(), scalar);
    entry(item, text(row, "state").as_deref(), now, &wanted)
}

fn dotfiles(value: &Value) -> Result<Vec<Entry>, ()> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Object(_) => {
            let mut rows = Vec::new();
            for row in array(value, "files").into_iter().flatten() {
                let name = text(row, "target").unwrap_or_else(|| "file".to_string());
                rows.push(entry(
                    name,
                    text(row, "state").as_deref(),
                    state_words(row),
                    "applied",
                ));
            }
            for row in array(value, "edits").into_iter().flatten() {
                let name = text(row, "path").unwrap_or_else(|| "file".to_string());
                rows.push(entry(
                    name,
                    text(row, "state").as_deref(),
                    state_words(row),
                    "applied",
                ));
            }
            Ok(rows)
        }
        _ => Err(()),
    }
}

fn tools(value: &Value) -> Result<Vec<Entry>, ()> {
    Ok(list(value)?
        .into_iter()
        .map(|row| {
            let name = text(row, "tool").unwrap_or_else(|| "tool".to_string());
            let verdict = match row.get("installed").and_then(Value::as_bool) {
                Some(true) => Verdict::Satisfied,
                Some(false) => Verdict::Differs,
                None => Verdict::Unrecognized,
            };
            Entry {
                item: name,
                spec: None,
                verdict,
                now: if verdict == Verdict::Differs {
                    "not installed".to_string()
                } else {
                    state_words(row)
                },
                wanted: "installed".to_string(),
                skipped: false,
            }
        })
        .collect())
}

/// `packages` is `{manager: {available, packages: [...]}}`. A manager that
/// is not available here cannot be changed, so its entries are unrecognised
/// (could not check), not differences.
fn packages(value: &Value) -> Result<Vec<Entry>, ()> {
    let map = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Object(map) => map,
        _ => return Err(()),
    };
    let mut managers: Vec<&String> = map.keys().collect();
    managers.sort();
    let mut rows = Vec::new();
    for manager in managers {
        let section = &map[manager];
        let available = section
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        for row in array(section, "packages").into_iter().flatten() {
            let name = text(row, "package").unwrap_or_else(|| "package".to_string());
            let state = text(row, "state");
            let desired = text(row, "desired_state").unwrap_or_else(|| "present".to_string());
            let installed = state.as_deref() == Some("installed");
            let verdict = if !available {
                Verdict::Unrecognized
            } else if desired == "absent" {
                if installed {
                    Verdict::Differs
                } else {
                    Verdict::Satisfied
                }
            } else {
                classify(state.as_deref())
            };
            rows.push(Entry {
                spec: Some(format!("{manager}:{name}")),
                item: name,
                verdict,
                now: if available {
                    state_words(row)
                } else {
                    "manager not available".to_string()
                },
                wanted: if desired == "absent" {
                    "removed".to_string()
                } else {
                    "installed".to_string()
                },
                skipped: false,
            });
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .expect("fixture readable")
    }

    fn area(report: &StatusReport, key: AreaKey) -> &AreaStatus {
        report
            .areas
            .iter()
            .find(|a| a.key == key)
            .unwrap_or_else(|| panic!("{key:?} missing"))
    }

    #[test]
    fn sixty_seven_already_set_defaults_are_up_to_date() {
        let report = StatusReport::parse(&fixture("status-applied.json"), "12:00:00").unwrap();
        let defaults = area(&report, AreaKey::MacosDefaults);
        assert_eq!(defaults.total(), 67);
        assert_eq!(defaults.health(), Health::UpToDate);
        assert!(defaults.changes().is_empty());
        assert_eq!(
            area(&report, AreaKey::LoginItems).health(),
            Health::UpToDate
        );
        assert!(report.is_clean());
        assert!(report.needs_changes().is_empty());
    }

    #[test]
    fn one_differing_default_is_named() {
        let report = StatusReport::parse(&fixture("status-one-differs.json"), "12:00:00").unwrap();
        let defaults = area(&report, AreaKey::MacosDefaults);
        assert_eq!(defaults.health(), Health::NeedsChanges);
        assert_eq!(defaults.total(), 67);
        let changes = defaults.changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].item, "com.apple.dock tilesize");
        assert_eq!(changes[0].now, "48");
        assert_eq!(changes[0].wanted, "36");
        assert_eq!(report.needs_changes().len(), 1);
        assert!(!report.is_clean());
    }

    #[test]
    fn unconfigured_areas_are_not_listed_and_login_shell_null_is_a_note() {
        let report = StatusReport::parse(&fixture("status-applied.json"), "t").unwrap();
        assert!(report.areas.iter().all(|a| a.key != AreaKey::Accounts));
        assert_eq!(report.notes, vec![Note::LoginShellNotRecorded]);
    }

    #[test]
    fn unrecognised_state_is_unknown_never_a_difference() {
        let raw = r#"{"macos_defaults":{"entries":[{"domain":"d","key":"k","value":1,"current":"1","state":"quantum"}]}}"#;
        let report = StatusReport::parse(raw, "t").unwrap();
        let defaults = area(&report, AreaKey::MacosDefaults);
        assert_eq!(defaults.health(), Health::Unknown);
        assert!(report.is_clean());
        assert_eq!(report.unknown().len(), 1);
    }

    #[test]
    fn unrecognised_shapes_are_unknown_never_a_difference() {
        let raw = r#"{"macos_defaults":"weird","tools":7,"mystery":[{"a":1}]}"#;
        let report = StatusReport::parse(raw, "t").unwrap();
        assert!(report.is_clean());
        assert_eq!(report.unknown().len(), 3);
        let other = area(&report, AreaKey::Other);
        assert_eq!(other.raw_key.as_deref(), Some("mystery"));
        assert_eq!(other.key.label(), "Other settings");
    }

    #[test]
    fn invalid_json_is_an_error_not_an_empty_report() {
        assert!(StatusReport::parse("not json", "t").is_err());
        assert!(StatusReport::parse("[]", "t").is_err());
    }

    #[test]
    fn per_area_rules_match_real_state_names() {
        let raw = r#"{
          "tools":[{"tool":"a","installed":true,"state":"installed"},{"tool":"b","installed":false,"state":"missing"}],
          "repos":[{"path_raw":"~/x","state":"current","reason":""},{"path_raw":"~/y","state":"behind","reason":"3 commits behind"}],
          "dotfiles":{"files":[{"target":"~/a","state":"applied"}],"edits":[{"path":"~/.zshrc","state":"not applied"}]},
          "macos_login_items":{"entries":[{"path":"/Applications/Example App.app","state":"unset"}]}
        }"#;
        let report = StatusReport::parse(raw, "t").unwrap();
        assert_eq!(area(&report, AreaKey::Tools).changes()[0].item, "b");
        let repos = area(&report, AreaKey::Repos).changes();
        assert_eq!(
            (repos[0].item.as_str(), repos[0].now.as_str()),
            ("~/y", "3 commits behind")
        );
        assert_eq!(
            area(&report, AreaKey::Dotfiles).changes()[0].item,
            "~/.zshrc"
        );
        let items = area(&report, AreaKey::LoginItems).changes();
        assert_eq!(
            (
                items[0].item.as_str(),
                items[0].now.as_str(),
                items[0].wanted.as_str()
            ),
            ("Example App", "missing", "present")
        );
    }

    #[test]
    fn omitted_packages_never_count() {
        let raw = r#"{"packages":{"brew":{"available":true,"packages":[
          {"package":"a","desired_state":"present","state":"installed"},
          {"package":"b","desired_state":"present","state":"missing"}]}}}"#;
        let report = StatusReport::parse(raw, "t").unwrap();
        assert_eq!(
            area(&report, AreaKey::Packages).health(),
            Health::NeedsChanges
        );
        let scoped = report.scoped(&["brew:a".to_string()]);
        let packages = area(&scoped, AreaKey::Packages);
        assert_eq!(packages.health(), Health::UpToDate);
        assert_eq!((packages.total(), packages.skipped()), (1, 1));
        let none = scoped.scoped(&[]);
        assert_eq!(area(&none, AreaKey::Packages).health(), Health::Skipped);
    }

    #[test]
    fn unavailable_package_manager_is_unknown() {
        let raw = r#"{"packages":{"apt":{"available":false,"packages":[
          {"package":"a","desired_state":"present","state":"missing"}]}}}"#;
        let report = StatusReport::parse(raw, "t").unwrap();
        assert_eq!(area(&report, AreaKey::Packages).health(), Health::Unknown);
    }

    #[test]
    fn every_status_key_has_a_label_and_a_declared_only_name() {
        for key in AreaKey::all() {
            assert!(!key.label().is_empty());
            assert_eq!(AreaKey::from_status_key(key.status_key()), key);
        }
        assert_eq!(AreaKey::MacosDefaults.only_name(), Some("macos-defaults"));
        assert_eq!(
            AreaKey::ShellActivation.only_name(),
            Some("mise-shell-activate")
        );
        assert_eq!(AreaKey::LoginItems.only_name(), None);
    }
}
