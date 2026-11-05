//! S2 Packages: per-package checklist grouped by declaring module.
//!
//! State + logic live here; painting lands in T4.6. Rows are data
//! ([`ListItem`](junie_tui::widgets::list::ListItem) values built per
//! group section, never `Checkbox`-in-`ListBox`), group headers are section
//! titles rendered as [`Panel::card`](junie_tui::widgets::panel::Panel)
//! (non-focusable by construction), and filtering reuses the shared
//! [`FilterState`](super::filter::FilterState) — hidden rows keep state.
//! Checked state crosses to the widget by index at render time
//! (`ListBox::set_checked`); the source of truth here is the stable
//! `(manager, name)` keep-set.
//!
//! A package declared by several selected envs appears once, under its
//! first declaring env; the detail pane lists every declaring module.

use std::collections::{HashMap, HashSet};

use junie_tui::theme::Tone;
use junie_tui::widgets::list::ListItem;

use super::filter::FilterState;
use crate::backend::ResId;

/// One checklist row, deduplicated by `(manager, name)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRow {
    /// Package manager (e.g. `brew`).
    pub manager: String,
    /// Package name (e.g. `ripgrep`).
    pub name: String,
    /// Declaring envs, in selection order.
    pub declared_by: Vec<String>,
}

impl PackageRow {
    /// `manager:name` form used by `packages apply`.
    pub fn spec(&self) -> String {
        format!("{}:{}", self.manager, self.name)
    }

    /// Display group: the first declaring env.
    pub fn group(&self) -> &str {
        self.declared_by.first().map(String::as_str).unwrap_or("")
    }
}

/// Build deduped rows for the selected envs (in order) from an origin map.
/// Within an env, rows sort by `(manager, name)` for determinism.
pub fn package_rows(
    selected_envs: &[String],
    origins: &HashMap<ResId, Vec<String>>,
) -> Vec<PackageRow> {
    let mut rows: Vec<PackageRow> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for env in selected_envs {
        let mut mine: Vec<&ResId> = origins
            .iter()
            .filter(|(_, decl)| decl.contains(env))
            .map(|(id, _)| id)
            .collect();
        mine.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
        for id in mine {
            let key = (id.kind.clone(), id.name.clone());
            if seen.insert(key.clone()) {
                rows.push(PackageRow {
                    manager: key.0,
                    name: key.1,
                    declared_by: origins.get(id).cloned().unwrap_or_default(),
                });
            }
        }
    }
    rows
}

/// Key intent on the packages screen (§3: `Space` toggles, `A` keep-all,
/// `N` keep-none, `/` filters, `Enter` applies, `Bksp` goes back).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKey {
    /// Toggle the cursor row.
    Toggle,
    /// Keep all visible (filtered) rows.
    KeepAllVisible,
    /// Omit all visible (filtered) rows.
    KeepNoneVisible,
    /// Enter filter typing.
    FilterMode,
    /// Apply the keep-list.
    Apply,
    /// Back to S1 (selection preserved by the caller).
    Back,
    /// Leave filter typing (back to navigation).
    ExitFilter,
}

/// Screen-level action returned by [`PackagesState::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageAction {
    /// State changed; stay on screen.
    None,
    /// Apply the keep-list (empty means apply base only).
    Apply,
    /// Back to S1.
    Back,
}

/// S2 checklist state. The keep-set lives on stable `(manager, name)` ids
/// outside the filter, so hidden rows keep state. Pre-ticked: everything.
#[derive(Debug, Clone)]
pub struct PackagesState {
    /// All rows, in group order.
    pub rows: Vec<PackageRow>,
    keep: HashSet<(String, String)>,
    filter: FilterState,
    cursor: usize,
    filtering: bool,
}

impl PackagesState {
    /// Build from rows; everything starts kept.
    pub fn new(rows: Vec<PackageRow>) -> Self {
        let keep: HashSet<(String, String)> = rows
            .iter()
            .map(|r| (r.manager.clone(), r.name.clone()))
            .collect();
        Self {
            rows,
            keep,
            filter: FilterState::new(),
            cursor: 0,
            filtering: false,
        }
    }

    /// Map a key to an intent (§3). Typing and `Bksp` go to the filter
    /// input while filtering; `Esc` leaves filter typing.
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<PackageKey> {
        use ratatui::crossterm::event::KeyCode;
        if self.filtering {
            if key.is(KeyCode::Esc) {
                return Some(PackageKey::ExitFilter);
            }
            return None;
        }
        if key.is_char(' ') {
            return Some(PackageKey::Toggle);
        }
        if key.is_char('A') || key.is_char('a') {
            return Some(PackageKey::KeepAllVisible);
        }
        if key.is_char('N') || key.is_char('n') {
            return Some(PackageKey::KeepNoneVisible);
        }
        if key.is_char('/') {
            return Some(PackageKey::FilterMode);
        }
        if key.is(KeyCode::Enter) {
            return Some(PackageKey::Apply);
        }
        if key.is(KeyCode::Backspace) {
            return Some(PackageKey::Back);
        }
        None
    }

    /// Apply an intent; returns the screen-level action.
    pub fn apply(&mut self, key: PackageKey) -> PackageAction {
        match key {
            PackageKey::Toggle => self.toggle_cursor(),
            PackageKey::KeepAllVisible => {
                for i in self.visible_indices() {
                    let r = &self.rows[i];
                    self.keep.insert((r.manager.clone(), r.name.clone()));
                }
            }
            PackageKey::KeepNoneVisible => {
                for i in self.visible_indices() {
                    let r = &self.rows[i];
                    self.keep.remove(&(r.manager.clone(), r.name.clone()));
                }
            }
            PackageKey::FilterMode => self.filtering = true,
            PackageKey::ExitFilter => self.filtering = false,
            PackageKey::Apply => return PackageAction::Apply,
            PackageKey::Back => return PackageAction::Back,
        }
        self.clamp_cursor();
        PackageAction::None
    }

    /// Indices into [`PackagesState::rows`] visible under the filter,
    /// matching `manager:name` text.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                self.filter.matches(&r.spec())
                    || self.filter.matches(&r.name)
                    || self.filter.matches(&r.manager)
            })
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

    /// Replace the query (resets the cursor into the filtered view).
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.filter.set_query(query);
        self.cursor = 0;
    }

    /// `(kept, total)` counts.
    pub fn counts(&self) -> (usize, usize) {
        (self.keep.len(), self.rows.len())
    }

    /// True when nothing is kept: apply runs base parts only
    /// (`--skip packages`, never a bare `packages apply`).
    pub fn apply_base_only(&self) -> bool {
        self.keep.is_empty()
    }

    /// Kept `manager:name` specs in row order.
    pub fn kept_specs(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| self.keep.contains(&(r.manager.clone(), r.name.clone())))
            .map(|r| r.spec())
            .collect()
    }

    /// Whether a row is kept.
    pub fn is_kept(&self, manager: &str, name: &str) -> bool {
        self.keep.contains(&(manager.to_string(), name.to_string()))
    }

    /// Group sections for render: `(group, items)` in first-seen order.
    /// Headers render as `Panel::card` titles, never as rows.
    pub fn group_sections(&self) -> Vec<(String, Vec<ListItem>)> {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<ListItem>> = HashMap::new();
        for i in self.visible_indices() {
            let r = &self.rows[i];
            let kept = self.is_kept(&r.manager, &r.name);
            let item = ListItem::new(&r.name)
                .meta(&format!(
                    "{} · {}",
                    r.manager,
                    if kept { "kept" } else { "omitted" }
                ))
                .meta_tone(if kept { Tone::Normal } else { Tone::Muted });
            let group = r.group().to_string();
            if !groups.contains_key(&group) {
                order.push(group.clone());
            }
            groups.entry(group).or_default().push(item);
        }
        order
            .into_iter()
            .map(|g| {
                let items = groups.remove(&g).unwrap_or_default();
                (g, items)
            })
            .collect()
    }

    /// Detail props for the cursor row: origin modules + manager + keep state.
    pub fn detail_props(&self) -> Vec<junie_tui::widgets::props::Prop> {
        use junie_tui::widgets::props::Prop;
        let Some(i) = self.visible_indices().get(self.cursor).copied() else {
            return Vec::new();
        };
        let r = &self.rows[i];
        let origins = r
            .declared_by
            .iter()
            .map(|e| format!("config.{e}.toml"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut state = Prop::new(
            "keep",
            if self.is_kept(&r.manager, &r.name) {
                "kept — included in `packages apply <kept…>`"
            } else {
                "omitted — one-shot, never recorded"
            },
        );
        state.tone = Tone::Secondary;
        vec![
            Prop::new("origin", origins),
            Prop::new("manager", &r.manager),
            state,
        ]
    }

    fn toggle_cursor(&mut self) {
        if let Some(i) = self.visible_indices().get(self.cursor).copied() {
            let r = &self.rows[i];
            let key = (r.manager.clone(), r.name.clone());
            if !self.keep.remove(&key) {
                self.keep.insert(key);
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
    use junie_tui::core::event::Key;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn origins() -> HashMap<ResId, Vec<String>> {
        let mut map = HashMap::new();
        map.insert(
            ResId {
                kind: "brew".to_string(),
                name: "ripgrep".to_string(),
            },
            vec!["neovim".to_string()],
        );
        map.insert(
            ResId {
                kind: "brew".to_string(),
                name: "starship".to_string(),
            },
            vec!["neovim".to_string(), "shell".to_string()],
        );
        map.insert(
            ResId {
                kind: "npm".to_string(),
                name: "pyright".to_string(),
            },
            vec!["shell".to_string()],
        );
        map
    }

    fn selected() -> Vec<String> {
        vec!["neovim".to_string(), "shell".to_string()]
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
    fn rows_dedupe_shared_packages_under_first_env() {
        let rows = package_rows(&selected(), &origins());
        assert_eq!(rows.len(), 3);
        let starship = rows
            .iter()
            .find(|r| r.name == "starship")
            .expect("starship row");
        assert_eq!(
            starship.declared_by,
            vec!["neovim".to_string(), "shell".to_string()]
        );
        assert_eq!(starship.group(), "neovim");
        // Deterministic within-env order: brew:ripgrep before brew:starship.
        let neovim: Vec<&str> = rows
            .iter()
            .filter(|r| r.group() == "neovim")
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(neovim, vec!["ripgrep", "starship"]);
    }

    #[test]
    fn toggle_flips_keep_and_counts() {
        let mut state = PackagesState::new(package_rows(&selected(), &origins()));
        assert_eq!(state.counts(), (3, 3));
        assert_eq!(state.apply(PackageKey::Toggle), PackageAction::None);
        assert_eq!(state.counts(), (2, 3));
        assert!(!state.is_kept("brew", "ripgrep"));
        assert_eq!(state.apply(PackageKey::Toggle), PackageAction::None);
        assert_eq!(state.counts(), (3, 3));
    }

    #[test]
    fn keep_all_and_none_scope_to_visible() {
        let mut state = PackagesState::new(package_rows(&selected(), &origins()));
        state.set_query("starship");
        assert_eq!(
            state.apply(PackageKey::KeepNoneVisible),
            PackageAction::None
        );
        assert_eq!(state.kept_specs().len(), 2);
        assert!(!state.is_kept("brew", "starship"));
        assert_eq!(state.apply(PackageKey::KeepAllVisible), PackageAction::None);
        assert_eq!(state.counts(), (3, 3));
    }

    #[test]
    fn empty_keep_means_apply_base_only() {
        let mut state = PackagesState::new(package_rows(&selected(), &origins()));
        assert!(!state.apply_base_only());
        assert_eq!(
            state.apply(PackageKey::KeepNoneVisible),
            PackageAction::None
        );
        // No filter: all rows visible, all cleared.
        assert!(state.apply_base_only());
        assert!(state.kept_specs().is_empty());
    }

    #[test]
    fn hidden_rows_keep_state() {
        let mut state = PackagesState::new(package_rows(&selected(), &origins()));
        assert_eq!(state.apply(PackageKey::Toggle), PackageAction::None);
        assert!(!state.is_kept("brew", "ripgrep"));
        state.set_query("pyright");
        assert_eq!(state.visible_indices().len(), 1);
        assert!(!state.is_kept("brew", "ripgrep"));
        state.set_query("");
        assert!(!state.is_kept("brew", "ripgrep"));
        assert!(state.is_kept("brew", "starship"));
    }

    #[test]
    fn keys_match_section_3() {
        let state = PackagesState::new(package_rows(&selected(), &origins()));
        assert_eq!(state.map_key(&char_key(' ')), Some(PackageKey::Toggle));
        assert_eq!(
            state.map_key(&char_key('A')),
            Some(PackageKey::KeepAllVisible)
        );
        assert_eq!(
            state.map_key(&char_key('a')),
            Some(PackageKey::KeepAllVisible)
        );
        assert_eq!(
            state.map_key(&char_key('N')),
            Some(PackageKey::KeepNoneVisible)
        );
        assert_eq!(state.map_key(&char_key('/')), Some(PackageKey::FilterMode));
        assert_eq!(state.map_key(&key(KeyCode::Enter)), Some(PackageKey::Apply));
        assert_eq!(
            state.map_key(&key(KeyCode::Backspace)),
            Some(PackageKey::Back)
        );
        assert_eq!(state.map_key(&key(KeyCode::Esc)), None);
        assert_eq!(state.map_key(&char_key('q')), None);
    }

    #[test]
    fn groups_render_as_sections_with_meta() {
        let state = PackagesState::new(package_rows(&selected(), &origins()));
        let sections = state.group_sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "neovim");
        assert_eq!(sections[1].0, "shell");
        assert!(
            sections[0]
                .1
                .iter()
                .all(|i| i.meta.as_deref().is_some_and(|m| m.contains("kept")))
        );
        assert!(!state.detail_props().is_empty());
    }

    #[test]
    fn apply_and_back_actions() {
        let mut state = PackagesState::new(package_rows(&selected(), &origins()));
        assert_eq!(state.apply(PackageKey::Apply), PackageAction::Apply);
        assert_eq!(state.apply(PackageKey::Back), PackageAction::Back);
    }
}
