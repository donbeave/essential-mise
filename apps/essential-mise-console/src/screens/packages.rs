//! S2 Packages: per-package checklist grouped by declaring module.
//!
//! The screen owner retains one stable list per declaring group and an outer
//! `ScrollView`, so filtered and off-screen groups keep their interaction
//! state. Rows are data, never `Checkbox`-in-`ListBox`; group headers remain
//! non-focusable `Panel::card` titles. The stable `(manager, name)` keep-set
//! is authoritative, and hidden rows always keep state.
//!
//! A package declared by several selected envs appears once, under its
//! first declaring env; the detail pane lists every declaring module.

use std::collections::{BTreeMap, HashMap, HashSet};

use junie_tui::core::edit::EditEntry;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{BlurAction, Focus, FocusChange, FocusRing};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Response;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::scroll_view::{Axes, ScrollView};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;

use super::chrome::{render_header, render_hints, render_status};
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

    /// Current filter query (for the filter input widget).
    pub fn query(&self) -> &str {
        self.filter.query()
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

    /// Move the flattened visible-row cursor.
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.clamp_cursor();
    }

    /// Set one visible row's stable keep flag.
    pub fn set_kept(&mut self, row: usize, kept: bool) {
        if let Some(row) = self.rows.get(row) {
            let key = (row.manager.clone(), row.name.clone());
            if kept {
                self.keep.insert(key);
            } else {
                self.keep.remove(&key);
            }
        }
    }

    /// Group sections for render: `(group, items)` in first-seen order.
    /// Headers render as `Panel::card` titles, never as rows.
    pub fn group_sections(&self) -> Vec<(String, Vec<ListItem>)> {
        self.group_order()
            .into_iter()
            .map(|(group, indices)| {
                let items = indices.iter().map(|&i| self.list_item(i)).collect();
                (group, items)
            })
            .collect()
    }

    /// Groups with visible row indices, in first-seen order.
    pub fn group_order(&self) -> Vec<(String, Vec<usize>)> {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for i in self.visible_indices() {
            let group = self.rows[i].group().to_string();
            if !groups.contains_key(&group) {
                order.push(group.clone());
            }
            groups.entry(group).or_default().push(i);
        }
        order
            .into_iter()
            .map(|g| {
                let indices = groups.remove(&g).unwrap_or_default();
                (g, indices)
            })
            .collect()
    }

    /// One checklist row as an inventoried [`ListItem`].
    pub fn list_item(&self, row_idx: usize) -> ListItem {
        let r = &self.rows[row_idx];
        let kept = self.is_kept(&r.manager, &r.name);
        ListItem::new(&r.name)
            .meta(&format!(
                "{} · {}",
                r.manager,
                if kept { "kept" } else { "omitted" }
            ))
            .meta_tone(if kept { Tone::Normal } else { Tone::Muted })
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

const S2_FILTER: WidgetId = WidgetId::of("emc.s2.filter");
const S2_LIST: WidgetId = WidgetId::of("emc.s2.list");
const S2_GROUPS: WidgetId = WidgetId::of("emc.s2.groups");

fn group_height(rows: usize) -> u16 {
    (rows as u16).saturating_add(3)
}

#[derive(Clone)]
pub(crate) struct PackagesOwner {
    pub(crate) state: PackagesState,
    filter: TextInput,
    lists: BTreeMap<String, ListBox>,
    scroll: ScrollView,
}

impl PackagesOwner {
    pub(crate) fn new(state: PackagesState) -> Self {
        let filter = TextInput::new(S2_FILTER, "Filter")
            .value(state.query())
            .edit_entry(EditEntry::Immediate)
            .blur_action(BlurAction::Commit);
        let mut owner = Self {
            state,
            filter,
            lists: BTreeMap::new(),
            scroll: ScrollView::new(S2_GROUPS)
                .axes(Axes::VERTICAL)
                .follow_focus(true),
        };
        owner.project_domain_to_widgets();
        owner
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        if self.state.filtering() {
            return S2_FILTER;
        }
        self.group_at_cursor()
            .map(|(group, _)| S2_LIST.sub(&group))
            .unwrap_or(S2_FILTER)
    }

    fn group_at_cursor(&self) -> Option<(String, usize)> {
        let mut start = 0;
        for (group, indices) in self.state.group_order() {
            if self.state.cursor() >= start && self.state.cursor() < start + indices.len() {
                return Some((group, self.state.cursor() - start));
            }
            start += indices.len();
        }
        None
    }

    fn focused_group(&self, focus: Option<WidgetId>) -> Option<String> {
        self.lists
            .keys()
            .find(|group| focus == Some(S2_LIST.sub(group)))
            .cloned()
            .or_else(|| self.group_at_cursor().map(|(group, _)| group))
    }

    fn project_domain_to_widgets(&mut self) {
        let mut start = 0;
        for (group, indices) in self.state.group_order() {
            let items: Vec<ListItem> = indices.iter().map(|&i| self.state.list_item(i)).collect();
            let checks: Vec<bool> = indices
                .iter()
                .map(|&i| {
                    let row = &self.state.rows[i];
                    self.state.is_kept(&row.manager, &row.name)
                })
                .collect();
            let cursor = self
                .state
                .cursor()
                .checked_sub(start)
                .filter(|&cursor| cursor < indices.len());
            let list = self.lists.entry(group.clone()).or_insert_with(|| {
                ListBox::new(S2_LIST.sub(&group), items.clone(), SelectMode::Multi)
            });
            list.items = items;
            if let Some(cursor) = cursor {
                list.core.cursor = cursor.min(list.items.len().saturating_sub(1));
            }
            for (i, checked) in checks.into_iter().enumerate() {
                list.set_checked(i, checked);
            }
            start += indices.len();
        }
    }

    fn sync_from_group(&mut self, group: &str) -> Outcome {
        let Some((_, indices)) = self
            .state
            .group_order()
            .into_iter()
            .find(|(name, _)| name == group)
        else {
            return Outcome::Ignored;
        };
        let before = self.state.cursor();
        let mut changed = false;
        let mut local = 0;
        let mut start = 0;
        for (name, rows) in self.state.group_order() {
            if name == group {
                let Some(list) = self.lists.get(group) else {
                    return Outcome::Ignored;
                };
                local = list.core.cursor.min(indices.len().saturating_sub(1));
                for (i, row) in indices.iter().copied().enumerate() {
                    let checked = list.is_checked(i);
                    let package = &self.state.rows[row];
                    if self.state.is_kept(&package.manager, &package.name) != checked {
                        self.state.set_kept(row, checked);
                        changed = true;
                    }
                }
                break;
            }
            start += rows.len();
        }
        self.state.set_cursor(start + local);
        if changed || before != self.state.cursor() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn sync_query_from_input(&mut self) -> Outcome {
        if self.filter.text() == self.state.query() {
            return Outcome::Ignored;
        }
        self.state.set_query(self.filter.text());
        self.project_domain_to_widgets();
        Outcome::Changed
    }

    fn leave_filter(&mut self, focus: &mut Focus) -> Outcome {
        let out = self.sync_query_from_input();
        self.filter.commit();
        self.state.filtering = false;
        let id = self.primary_focus();
        focus.focus(id);
        out.or(Outcome::Changed)
    }

    pub(crate) fn key(
        &mut self,
        key: &Key,
        focus: &mut Focus,
        _ring: &FocusRing,
    ) -> Response<PackageAction> {
        if self.state.filtering() {
            if key.is(KeyCode::Esc) || key.is(KeyCode::Enter) {
                return self.leave_filter(focus).into();
            }
            let (out, action) = self.filter.on_key(key);
            let out = out.or(self.sync_query_from_input());
            if matches!(
                action,
                Some(InputAction::Committed | InputAction::CommittedTab { .. })
            ) {
                self.state.filtering = false;
            }
            return out.into();
        }

        if key.is_char('/') {
            self.state.filtering = true;
            focus.focus(S2_FILTER);
            self.filter.begin_edit();
            return Outcome::Changed.into();
        }
        if key.is(KeyCode::Enter) {
            return Response::emit(PackageAction::Apply);
        }
        if key.is(KeyCode::Backspace) {
            return Response::emit(PackageAction::Back);
        }
        if key.is_char('A') || key.is_char('a') {
            self.state.apply(PackageKey::KeepAllVisible);
            self.project_domain_to_widgets();
            return Outcome::Changed.into();
        }
        if key.is_char('N') || key.is_char('n') {
            self.state.apply(PackageKey::KeepNoneVisible);
            self.project_domain_to_widgets();
            return Outcome::Changed.into();
        }

        let group = self.focused_group(focus.current());
        if let Some(group) = group {
            let before = self.state.cursor();
            let out = match self.lists.get_mut(&group) {
                Some(list) => list.on_key(key),
                None => Outcome::Ignored,
            };
            let out = out.or(self.sync_from_group(&group));
            if matches!(key.code, KeyCode::Up | KeyCode::Down) && self.state.cursor() == before {
                let boundary = self.move_group_boundary(key, focus);
                if boundary != Outcome::Ignored {
                    return boundary.into();
                }
            }
            if out != Outcome::Ignored {
                return out.into();
            }
        }
        self.scroll.on_key(key).unwrap_or(Outcome::Ignored).into()
    }

    fn move_group_boundary(&mut self, key: &Key, focus: &mut Focus) -> Outcome {
        let visible = self.state.visible_indices().len();
        let target = match key.code {
            KeyCode::Up => self.state.cursor().checked_sub(1),
            KeyCode::Down if self.state.cursor() + 1 < visible => Some(self.state.cursor() + 1),
            _ => None,
        };
        let Some(target) = target else {
            return Outcome::Ignored;
        };
        self.state.set_cursor(target);
        let Some((group, local)) = self.group_at_cursor() else {
            return Outcome::Ignored;
        };
        if let Some(list) = self.lists.get_mut(&group) {
            list.core.cursor = local;
        }
        focus.focus(S2_LIST.sub(&group));
        Outcome::Changed
    }

    pub(crate) fn paste(&mut self, text: &str, focus: &Focus) -> Outcome {
        if !self.state.filtering() && focus.current() != Some(S2_FILTER) {
            return Outcome::Ignored;
        }
        self.filter.on_paste(text).or(self.sync_query_from_input())
    }

    pub(crate) fn blur(&mut self, change: FocusChange) -> Outcome {
        if change.from != S2_FILTER {
            return Outcome::Ignored;
        }
        let _ = self.filter.on_blur();
        let out = self.sync_query_from_input();
        self.state.filtering = false;
        out
    }

    pub(crate) fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
    ) -> Response<PackageAction> {
        if let Some(response) = self.filter.route(ev) {
            if ev.is(Phase::Click) {
                self.state.filtering = true;
            }
            let out = response.outcome().or(self.sync_query_from_input());
            return out.into();
        }
        let groups: Vec<String> = self.lists.keys().cloned().collect();
        for group in groups {
            let response = self.lists.get_mut(&group).and_then(|list| list.route(ev));
            if let Some(response) = response {
                if ev.is(Phase::Click) {
                    focus.focus(S2_LIST.sub(&group));
                }
                let out = response.outcome().or(self.sync_from_group(&group));
                return out.into();
            }
        }
        if let Some(response) = self.scroll.pointer(ev) {
            return response.lift();
        }
        Response::ignored()
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.project_domain_to_widgets();
        let theme: Theme = *ctx.theme;
        let bg: Color = theme.canvas;
        render_header(
            2,
            "Packages",
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(3),
        );
        let input_area = Rect::new(body.x, body.y, body.width, 3.min(body.height));
        self.filter.render(input_area, buf, ctx, bg);
        let groups_area = Rect::new(
            body.x,
            body.y + 3.min(body.height),
            body.width,
            body.height.saturating_sub(3),
        );
        let groups = self.state.group_order();
        let height = groups.iter().fold(0u16, |height, (_, indices)| {
            height.saturating_add(group_height(indices.len()))
        });
        let lists = &mut self.lists;
        self.scroll.render(
            groups_area,
            Size::new(groups_area.width, height),
            buf,
            ctx,
            |content, buf, ctx| {
                let mut y = content.y;
                for (group, indices) in &groups {
                    let group_area =
                        Rect::new(content.x, y, content.width, group_height(indices.len()));
                    let inner = Panel::card(Some(group.as_str())).render(group_area, buf, &theme);
                    if let Some(list) = lists.get_mut(group) {
                        list.render(inner, buf, ctx, bg);
                    }
                    y = y.saturating_add(group_height(indices.len()));
                }
            },
        );
        let (kept, total) = self.state.counts();
        render_hints(
            &[
                ("Space", "toggle"),
                ("A", "keep-all"),
                ("N", "keep-none"),
                ("/", "filter"),
                ("Enter", "apply"),
                ("Bksp", "back"),
            ],
            Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1),
            buf,
            &theme,
        );
        render_status(
            &format!("S2 · keeping {kept} of {total}"),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            ctx,
        );
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
