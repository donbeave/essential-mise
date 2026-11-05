//! S1 Modules: environment multi-select with search and badges.
//!
//! State + logic live here; painting lands in T4.6 against the tuiscotti
//! harness. Row/badge widgets use inventoried components only
//! ([`ListRow`](junie_tui::widgets::list_detail::ListRow),
//! [`Prop`](junie_tui::widgets::props::Prop)) via the shared
//! [`FilterState`](crate::screens::filter::FilterState) and [`chrome`]
//! helpers — no per-screen widget copies.
//!
//! Environment names/descriptions are parsed from `bootstrap.sh`
//! `environment_names`/`environment_descriptions` (space-split words and
//! single-quoted lines; no `regex` dependency, plain line scan). Pre-ticks
//! come from the `env` key of `miserc.local.toml` (read-only).
//! Badges: status drift anywhere conservative-marks drift; otherwise an
//! env with no packages is pending and all-`noop` packages is installed
//! (packages-only, since plan is blind to repos/dotfiles/shell).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use junie_tui::core::event::Key;
use junie_tui::theme::{StatusKind, Tone};
use junie_tui::widgets::list_detail::ListRow;
use junie_tui::widgets::props::Prop;
use ratatui::crossterm::event::KeyCode;

use super::filter::FilterState;
use crate::backend::{Action, PlanEntry, ResId};

/// One selectable environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    /// Environment name (stable id, e.g. `terminal`).
    pub name: String,
    /// One-line description.
    pub description: String,
}

/// Parse `environment_names` (space-split) and `environment_descriptions`
/// (single-quoted lines) from `bootstrap.sh` text, zipped pairwise.
pub fn parse_environments(text: &str) -> Vec<ModuleInfo> {
    let names = bash_words(text, "environment_names");
    let descriptions = bash_quoted_lines(text, "environment_descriptions");
    names
        .into_iter()
        .zip(descriptions)
        .map(|(name, description)| ModuleInfo { name, description })
        .collect()
}

/// Space-separated bare words between `name=(` and `)`.
fn bash_words(text: &str, name: &str) -> Vec<String> {
    let open = format!("{name}=(");
    let mut words = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if !inside {
            if t == open {
                inside = true;
            }
            continue;
        }
        if t == ")" {
            break;
        }
        words.extend(t.split_whitespace().map(|w| w.to_string()));
    }
    words
}

/// Single-quoted one-per-line strings between `name=(` and `)`.
fn bash_quoted_lines(text: &str, name: &str) -> Vec<String> {
    let open = format!("{name}=(");
    let mut items = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if !inside {
            if t == open {
                inside = true;
            }
            continue;
        }
        if t == ")" {
            break;
        }
        if let Some(rest) = t.strip_prefix('\'')
            && let Some(end) = rest.find('\'')
        {
            items.push(rest[..end].to_string());
        }
    }
    items
}

/// Read pre-ticked envs from the `env` key of `miserc.local.toml`
/// (read-only). Missing file or invalid TOML yields no pre-ticks.
pub fn read_preticks(path: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };
    #[derive(Default, serde::Deserialize)]
    struct Local {
        #[serde(default)]
        env: Vec<String>,
    }
    toml::from_str::<Local>(&text)
        .map(|l| l.env)
        .unwrap_or_default()
}

/// Per-environment badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvBadge {
    /// All known state clean (packages-only when plan-derived).
    Installed,
    /// Status drift, or a non-`noop` plan action.
    Drift,
    /// The env declares no packages (e.g. `js`, `system-preferences`).
    Pending,
}

impl EnvBadge {
    /// Short label for headers and detail panes.
    pub fn label(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Drift => "drift",
            Self::Pending => "pending",
        }
    }
}

/// Aggregate an env badge: status drift wins, then empty means pending,
/// all-`noop` means installed, anything else is drift.
pub fn badge_for_env(actions: &[Action], parts_ok: bool) -> EnvBadge {
    if !parts_ok {
        return EnvBadge::Drift;
    }
    if actions.is_empty() {
        return EnvBadge::Pending;
    }
    if actions.iter().all(|a| *a == Action::Noop) {
        EnvBadge::Installed
    } else {
        EnvBadge::Drift
    }
}

/// Key intent on the modules screen (§3 keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKey {
    /// Move to the previous visible row.
    Up,
    /// Move to the next visible row.
    Down,
    /// Toggle the cursor row.
    Toggle,
    /// Enter filter typing.
    FilterMode,
    /// Select all visible (filtered) rows.
    SelectAllVisible,
    /// Clear all visible (filtered) rows.
    SelectNoneVisible,
    /// Continue with the selection (disabled while empty).
    Confirm,
    /// Quit to the quit dialog.
    Quit,
    /// Leave filter typing (back to navigation).
    ExitFilter,
}

/// Screen-level action returned by [`ModulesState::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleAction {
    /// State changed; stay on screen.
    None,
    /// Selection confirmed (only when [`ModulesState::can_continue`]).
    Confirm,
    /// Quit requested (caller opens the quit dialog).
    Quit,
}

/// S1 selection state. Selection lives on stable env names outside the
/// filter, so hidden rows keep state.
#[derive(Debug, Clone)]
pub struct ModulesState {
    /// All known environments, in script order.
    pub envs: Vec<ModuleInfo>,
    selected: HashSet<String>,
    filter: FilterState,
    cursor: usize,
    filtering: bool,
    badges: HashMap<String, EnvBadge>,
}

impl ModulesState {
    /// Build from parsed envs, pre-ticking known names only.
    pub fn new(envs: Vec<ModuleInfo>, preticks: &[String]) -> Self {
        let known: HashSet<&str> = envs.iter().map(|e| e.name.as_str()).collect();
        let selected: HashSet<String> = preticks
            .iter()
            .filter(|e| known.contains(e.as_str()))
            .cloned()
            .collect();
        Self {
            envs,
            selected,
            filter: FilterState::new(),
            cursor: 0,
            filtering: false,
            badges: HashMap::new(),
        }
    }

    /// Map a key to an intent (§3: arrows move, `Space` toggles, `/`
    /// filters, `a`/`n` scope to the visible rows, `Enter` continues,
    /// `Esc` quits). Typing goes to the filter input while filtering.
    pub fn map_key(&self, key: &Key) -> Option<ModuleKey> {
        if self.filtering {
            if key.is(KeyCode::Esc) {
                return Some(ModuleKey::ExitFilter);
            }
            return None;
        }
        if key.is(KeyCode::Up) {
            return Some(ModuleKey::Up);
        }
        if key.is(KeyCode::Down) {
            return Some(ModuleKey::Down);
        }
        if key.is_char(' ') {
            return Some(ModuleKey::Toggle);
        }
        if key.is_char('/') {
            return Some(ModuleKey::FilterMode);
        }
        if key.is_char('a') {
            return Some(ModuleKey::SelectAllVisible);
        }
        if key.is_char('n') {
            return Some(ModuleKey::SelectNoneVisible);
        }
        if key.is(KeyCode::Enter) {
            return Some(ModuleKey::Confirm);
        }
        if key.is(KeyCode::Esc) {
            return Some(ModuleKey::Quit);
        }
        None
    }

    /// Apply an intent; returns the screen-level action.
    pub fn apply(&mut self, key: ModuleKey) -> ModuleAction {
        match key {
            ModuleKey::Up => self.move_cursor(-1),
            ModuleKey::Down => self.move_cursor(1),
            ModuleKey::Toggle => self.toggle_cursor(),
            ModuleKey::FilterMode => self.filtering = true,
            ModuleKey::ExitFilter => self.filtering = false,
            ModuleKey::SelectAllVisible => {
                for i in self.visible_indices() {
                    self.selected.insert(self.envs[i].name.clone());
                }
                self.clamp_cursor();
            }
            ModuleKey::SelectNoneVisible => {
                for i in self.visible_indices() {
                    self.selected.remove(&self.envs[i].name);
                }
                self.clamp_cursor();
            }
            ModuleKey::Confirm => {
                if self.can_continue() {
                    return ModuleAction::Confirm;
                }
            }
            ModuleKey::Quit => return ModuleAction::Quit,
        }
        self.clamp_cursor();
        ModuleAction::None
    }

    /// Indices into [`ModulesState::envs`] visible under the filter,
    /// matching name or description.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.envs
            .iter()
            .enumerate()
            .filter(|(_, e)| self.filter.matches(&e.name) || self.filter.matches(&e.description))
            .map(|(i, _)| i)
            .collect()
    }

    /// Cursor row within the visible list.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether filter typing is active.
    pub fn filtering(&self) -> bool {
        self.filtering
    }

    /// Current filter query (for the filter input widget).
    pub fn query(&self) -> &str {
        self.filter.query()
    }

    /// Cursor env, if any row is visible.
    pub fn cursor_env(&self) -> Option<&ModuleInfo> {
        self.visible_indices()
            .get(self.cursor)
            .and_then(|&i| self.envs.get(i))
    }

    /// Access the live filter (the app routes typing to the `TextInput`).
    pub fn filter_mut(&mut self) -> &mut FilterState {
        &mut self.filter
    }

    /// Replace the query (resets the cursor into the filtered view).
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.filter.set_query(query);
        self.cursor = 0;
    }

    /// Continue is enabled only with a non-empty selection.
    pub fn can_continue(&self) -> bool {
        !self.selected.is_empty()
    }

    /// Selected env names in script order.
    pub fn selected_envs(&self) -> Vec<String> {
        self.envs
            .iter()
            .filter(|e| self.selected.contains(&e.name))
            .map(|e| e.name.clone())
            .collect()
    }

    /// `(visible, total, selected)` counts for the header.
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.visible_indices().len(),
            self.envs.len(),
            self.selected.len(),
        )
    }

    /// Recompute badges from plan entries joined with declaring modules.
    /// `parts_ok` is false when `status --json` reports any drift.
    pub fn refresh_badges(
        &mut self,
        plan: &[PlanEntry],
        origins: &HashMap<ResId, Vec<String>>,
        parts_ok: bool,
    ) {
        for env in &self.envs {
            let actions: Vec<Action> = plan
                .iter()
                .filter(|e| {
                    origins
                        .get(&ResId {
                            kind: e.kind.clone(),
                            name: e.name.clone(),
                        })
                        .is_some_and(|decl| decl.contains(&env.name))
                })
                .map(|e| e.action)
                .collect();
            self.badges
                .insert(env.name.clone(), badge_for_env(&actions, parts_ok));
        }
    }

    /// Badge for an env (defaults to pending before the first refresh).
    pub fn badge(&self, name: &str) -> EnvBadge {
        self.badges.get(name).copied().unwrap_or(EnvBadge::Pending)
    }

    /// Master-pane rows as inventoried [`ListRow`]s: name label,
    /// description detail, `[x]`/`[ ]` meta, badge mark.
    pub fn master_rows(&self) -> Vec<ListRow> {
        self.visible_indices()
            .into_iter()
            .map(|i| {
                let env = &self.envs[i];
                let mark = match self.badge(&env.name) {
                    EnvBadge::Installed => StatusKind::Ok,
                    EnvBadge::Drift => StatusKind::Warn,
                    EnvBadge::Pending => StatusKind::Pending,
                };
                let (meta, tone) = if self.selected.contains(&env.name) {
                    ("[x]", Tone::Normal)
                } else {
                    ("[ ]", Tone::Muted)
                };
                ListRow::new(&env.name)
                    .detail(&env.description)
                    .meta(meta, tone)
                    .mark(mark)
            })
            .collect()
    }

    /// Detail-pane props for the cursor env.
    pub fn detail_props(&self) -> Vec<Prop> {
        let Some(i) = self.visible_indices().get(self.cursor).copied() else {
            return Vec::new();
        };
        let env = &self.envs[i];
        let (status, tone) = match self.badge(&env.name) {
            EnvBadge::Installed => ("installed (packages)", Tone::Normal),
            EnvBadge::Drift => ("drift — re-check in S4", Tone::Warning),
            EnvBadge::Pending => ("pending — no packages declared", Tone::Muted),
        };
        let mut kept = Prop::new(
            "selection",
            if self.selected.contains(&env.name) {
                "kept"
            } else {
                "omitted"
            },
        );
        kept.tone = Tone::Secondary;
        let mut status_prop = Prop::new("status", status);
        status_prop.tone = tone;
        vec![
            Prop::new("description", &env.description),
            status_prop,
            kept,
        ]
    }

    fn move_cursor(&mut self, delta: isize) {
        let len = self.visible_indices().len();
        if len == 0 {
            return;
        }
        let next = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        self.cursor = next;
    }

    fn toggle_cursor(&mut self) {
        if let Some(i) = self.visible_indices().get(self.cursor).copied() {
            let name = self.envs[i].name.clone();
            if !self.selected.remove(&name) {
                self.selected.insert(name);
            }
        }
    }

    fn clamp_cursor(&mut self) {
        let len = self.visible_indices().len();
        self.cursor = self.cursor.min(len.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    const TWO_ENVS: &str = "environment_names=(\n  alpha beta\n)\nenvironment_descriptions=(\n  'First env'\n  'Second env'\n)\n";

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

    fn two_envs() -> Vec<ModuleInfo> {
        parse_environments(TWO_ENVS)
    }

    #[test]
    fn parse_pairs_names_with_descriptions() {
        let envs = two_envs();
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].name, "alpha");
        assert_eq!(envs[0].description, "First env");
        assert_eq!(envs[1].name, "beta");
    }

    #[test]
    fn parse_real_bootstrap_arrays() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../bootstrap.sh"))
                .expect("bootstrap.sh readable");
        let envs = parse_environments(&text);
        // Spec §0: 29 environments; grep bootstrap.sh:159-195 to re-verify.
        assert_eq!(envs.len(), 29);
        assert_eq!(envs[0].name, "terminal");
        assert!(envs[0].description.contains("Ghostty"));
        assert!(envs.iter().all(|e| !e.description.is_empty()));
    }

    #[test]
    fn preticks_read_env_key_and_ignore_unknown() {
        let dir = std::env::temp_dir().join(format!("emc-test-pretick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("miserc.local.toml");
        std::fs::write(&path, "env = [\"alpha\", \"no-such-env\"]\n").expect("write toml");
        assert_eq!(
            read_preticks(&path),
            vec!["alpha".to_string(), "no-such-env".to_string()]
        );
        let state = ModulesState::new(two_envs(), &read_preticks(&path));
        assert_eq!(state.selected_envs(), vec!["alpha".to_string()]);
        assert!(read_preticks(&dir.join("missing.toml")).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn badges_cover_installed_drift_pending() {
        assert_eq!(
            badge_for_env(&[Action::Noop, Action::Noop], true),
            EnvBadge::Installed
        );
        assert_eq!(
            badge_for_env(&[Action::Noop, Action::Create], true),
            EnvBadge::Drift
        );
        assert_eq!(badge_for_env(&[Action::Noop], false), EnvBadge::Drift);
        assert_eq!(badge_for_env(&[], true), EnvBadge::Pending);
        assert_eq!(badge_for_env(&[Action::Unknown], true), EnvBadge::Drift);
    }

    #[test]
    fn keys_match_section_3() {
        let state = ModulesState::new(two_envs(), &[]);
        assert_eq!(state.map_key(&key(KeyCode::Up)), Some(ModuleKey::Up));
        assert_eq!(state.map_key(&key(KeyCode::Down)), Some(ModuleKey::Down));
        assert_eq!(state.map_key(&char_key(' ')), Some(ModuleKey::Toggle));
        assert_eq!(state.map_key(&char_key('/')), Some(ModuleKey::FilterMode));
        assert_eq!(
            state.map_key(&char_key('a')),
            Some(ModuleKey::SelectAllVisible)
        );
        assert_eq!(
            state.map_key(&char_key('n')),
            Some(ModuleKey::SelectNoneVisible)
        );
        assert_eq!(
            state.map_key(&key(KeyCode::Enter)),
            Some(ModuleKey::Confirm)
        );
        assert_eq!(state.map_key(&key(KeyCode::Esc)), Some(ModuleKey::Quit));
        assert_eq!(state.map_key(&char_key('q')), None);
    }

    #[test]
    fn toggle_move_and_confirm_gate() {
        let mut state = ModulesState::new(two_envs(), &[]);
        assert!(!state.can_continue());
        assert_eq!(state.apply(ModuleKey::Confirm), ModuleAction::None);
        assert_eq!(state.apply(ModuleKey::Toggle), ModuleAction::None);
        assert_eq!(state.selected_envs(), vec!["alpha".to_string()]);
        assert!(state.can_continue());
        assert_eq!(state.apply(ModuleKey::Down), ModuleAction::None);
        assert_eq!(state.apply(ModuleKey::Toggle), ModuleAction::None);
        assert_eq!(
            state.selected_envs(),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(state.apply(ModuleKey::Confirm), ModuleAction::Confirm);
        assert_eq!(state.apply(ModuleKey::Quit), ModuleAction::Quit);
    }

    #[test]
    fn select_all_and_none_scope_to_visible() {
        let mut state = ModulesState::new(two_envs(), &[]);
        state.set_query("alpha");
        assert_eq!(state.apply(ModuleKey::SelectAllVisible), ModuleAction::None);
        assert_eq!(state.selected_envs(), vec!["alpha".to_string()]);
        assert_eq!(
            state.apply(ModuleKey::SelectNoneVisible),
            ModuleAction::None
        );
        assert!(state.selected_envs().is_empty());
    }

    #[test]
    fn hidden_rows_keep_state() {
        let mut state = ModulesState::new(two_envs(), &["alpha".to_string()]);
        state.set_query("beta");
        assert_eq!(state.counts(), (1, 2, 1));
        assert_eq!(state.selected_envs(), vec!["alpha".to_string()]);
        state.set_query("");
        assert_eq!(state.selected_envs(), vec!["alpha".to_string()]);
        assert_eq!(state.counts(), (2, 2, 1));
    }

    #[test]
    fn rows_carry_badge_marks() {
        let mut state = ModulesState::new(two_envs(), &["alpha".to_string()]);
        let mut origins = HashMap::new();
        origins.insert(
            ResId {
                kind: "brew".to_string(),
                name: "jq".to_string(),
            },
            vec!["alpha".to_string()],
        );
        let plan = vec![PlanEntry {
            kind: "brew".to_string(),
            name: "jq".to_string(),
            current: "present".to_string(),
            desired: "present".to_string(),
            action: Action::Noop,
        }];
        state.refresh_badges(&plan, &origins, true);
        assert_eq!(state.badge("alpha"), EnvBadge::Installed);
        assert_eq!(state.badge("beta"), EnvBadge::Pending);
        let rows = state.master_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].mark, Some(StatusKind::Ok));
        assert_eq!(rows[1].mark, Some(StatusKind::Pending));
        assert_eq!(rows[0].meta, "[x]");
        assert_eq!(rows[1].meta, "[ ]");
        assert!(!state.detail_props().is_empty());
    }
}
