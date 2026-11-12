//! Step 2: Review and confirm.
//!
//! Lists only what will change: packages that are not installed yet (and
//! settings groups that differ), with the summary of the whole run on top.
//! Already-installed packages are collapsed (`h` shows them). The person
//! chooses Install or Skip this time per package; nothing runs before the
//! explicit Enter ("Apply now"), which is ignored while the check is still
//! running and for a moment after the screen opens, so a held key cannot
//! start a real apply.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::response::Response;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use super::chrome::render_step_header;
use super::paint::{self, Marks, Row, bold, footer, framed, primary_line, put, seg};
use crate::backend::ResId;
use crate::status::{AreaKey, StatusReport, Verdict};

/// How long a freshly opened Review ignores Enter.
pub const DEFAULT_ARM_DELAY: Duration = Duration::from_millis(400);

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

/// Where a package stands, from the status check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Not installed (or not as configured): applying changes it.
    ToInstall,
    /// Already as configured.
    Installed,
    /// The check could not tell (or has not run): treated as to-install.
    Unknown,
}

/// Key intent on the review screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageAction {
    /// State changed; stay on screen.
    None,
    /// Apply now.
    Apply,
    /// Back to choosing modules.
    Back,
    /// Leave the console (nothing has run).
    Quit,
}

/// Review state: package rows plus what the check found.
#[derive(Debug, Clone)]
pub struct PackagesState {
    /// Every declared package, in module order.
    pub rows: Vec<PackageRow>,
    states: Vec<RowState>,
    install: Vec<bool>,
    modules: usize,
    checking: bool,
    error: Option<String>,
    settings_groups: usize,
    show_installed: bool,
    cursor: usize,
    created: Instant,
    arm_delay: Duration,
}

impl PackagesState {
    /// A review of `rows` for `modules` selected modules; the check is
    /// running until [`apply_check`](Self::apply_check) is called.
    pub fn new(rows: Vec<PackageRow>, modules: usize) -> Self {
        let len = rows.len();
        Self {
            rows,
            states: vec![RowState::Unknown; len],
            install: vec![true; len],
            modules,
            checking: true,
            error: None,
            settings_groups: 0,
            show_installed: false,
            cursor: 0,
            created: Instant::now(),
            arm_delay: DEFAULT_ARM_DELAY,
        }
    }

    /// Override how long Enter is ignored after opening (tests, tools).
    #[must_use]
    pub fn with_arm_delay(mut self, delay: Duration) -> Self {
        self.arm_delay = delay;
        self
    }

    /// Take the result of the status check.
    pub fn apply_check(&mut self, result: Result<StatusReport, String>) {
        self.checking = false;
        match result {
            Ok(report) => {
                self.error = None;
                let packages = report.areas.iter().find(|a| a.key == AreaKey::Packages);
                for (i, row) in self.rows.iter().enumerate() {
                    let spec = row.spec();
                    let verdict = packages
                        .and_then(|a| a.entries.iter().find(|e| e.spec.as_deref() == Some(&spec)))
                        .map(|e| e.verdict);
                    self.states[i] = match verdict {
                        Some(Verdict::Satisfied) => RowState::Installed,
                        Some(Verdict::Differs) => RowState::ToInstall,
                        _ => RowState::Unknown,
                    };
                }
                self.settings_groups = report
                    .needs_changes()
                    .iter()
                    .filter(|a| a.key != AreaKey::Packages)
                    .count();
            }
            Err(reason) => {
                self.error = Some(reason);
                self.states.iter_mut().for_each(|s| *s = RowState::Unknown);
            }
        }
        self.cursor = 0;
    }

    /// Whether the check is still running.
    pub fn is_checking(&self) -> bool {
        self.checking
    }

    /// Rows shown in the list: everything that changes, plus installed ones
    /// when `h` expanded them.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.show_installed || self.states[i] != RowState::Installed)
            .collect()
    }

    /// Package state of a row.
    pub fn row_state(&self, index: usize) -> RowState {
        self.states[index]
    }

    /// Whether a row will be installed.
    pub fn will_install(&self, index: usize) -> bool {
        self.install[index]
    }

    /// Packages that change, kept or not.
    fn changing(&self) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.states[i] != RowState::Installed)
            .collect()
    }

    /// Packages that will be installed: `manager:name`, in row order.
    pub fn install_specs(&self) -> Vec<String> {
        self.changing()
            .into_iter()
            .filter(|&i| self.install[i])
            .map(|i| self.rows[i].spec())
            .collect()
    }

    /// Packages the person chose to skip this time.
    pub fn skipped_specs(&self) -> Vec<String> {
        self.changing()
            .into_iter()
            .filter(|&i| !self.install[i])
            .map(|i| self.rows[i].spec())
            .collect()
    }

    /// Whether the package is installed or will be (for tests).
    pub fn is_kept(&self, manager: &str, name: &str) -> bool {
        self.rows
            .iter()
            .position(|r| r.manager == manager && r.name == name)
            .is_some_and(|i| self.install[i])
    }

    fn installed_count(&self) -> usize {
        self.states
            .iter()
            .filter(|s| **s == RowState::Installed)
            .count()
    }

    /// The summary lines of the box.
    pub fn summary(&self) -> Vec<String> {
        let modules = format!(
            "{} {}",
            self.modules,
            if self.modules == 1 {
                "module"
            } else {
                "modules"
            }
        );
        if self.checking {
            return vec![
                format!("{modules} · checking what is already installed"),
                "Asks for your administrator password once, at the start.".to_string(),
            ];
        }
        let to_install = self.changing().len();
        let mut line = format!(
            "{modules} · {to_install} {} to install",
            if to_install == 1 {
                "package"
            } else {
                "packages"
            }
        );
        if self.error.is_none() {
            line.push_str(&format!(" ({} already installed)", self.installed_count()));
            line.push_str(&format!(
                " · {} {} of settings to update",
                self.settings_groups,
                if self.settings_groups == 1 {
                    "group"
                } else {
                    "groups"
                }
            ));
        }
        let mut lines = vec![line];
        if let Some(reason) = &self.error {
            lines.push(format!(
                "Could not check this Mac ({reason}); every package is listed."
            ));
        }
        lines.push(
            "Asks for your administrator password once, at the start. Usually 5-15 minutes."
                .to_string(),
        );
        lines
    }

    fn armed(&self) -> bool {
        !self.checking && self.created.elapsed() >= self.arm_delay
    }

    /// Footer hints for the keys that work right now.
    pub fn hints(&self) -> Vec<(&'static str, String)> {
        let mut hints: Vec<(&'static str, String)> = Vec::new();
        if self.checking {
            return vec![
                ("Esc", "back".into()),
                ("?", "help".into()),
                ("q", "quit".into()),
            ];
        }
        if self.visible().len() > 1 {
            hints.push(("↑↓", "move".into()));
        }
        if !self.changing().is_empty() {
            hints.push(("Space", "install or skip".into()));
            hints.push(("a", "install all".into()));
            hints.push(("n", "skip all".into()));
        }
        if self.installed_count() > 0 {
            hints.push((
                "h",
                if self.show_installed {
                    "hide installed".into()
                } else {
                    "show installed".into()
                },
            ));
        }
        hints.push(("Esc", "back".into()));
        hints.push(("?", "help".into()));
        hints.push(("q", "quit".into()));
        hints
    }

    /// Handle a key.
    pub fn on_key(&mut self, key: &Key) -> PackageAction {
        if key.is(KeyCode::Enter) {
            return if self.armed() {
                PackageAction::Apply
            } else {
                PackageAction::None
            };
        }
        if key.is(KeyCode::Esc) {
            return PackageAction::Back;
        }
        if key.is_char('q') {
            return PackageAction::Quit;
        }
        if self.checking {
            return PackageAction::None;
        }
        let visible = self.visible();
        if key.is(KeyCode::Down) {
            self.cursor = (self.cursor + 1).min(visible.len().saturating_sub(1));
        } else if key.is(KeyCode::Up) {
            self.cursor = self.cursor.saturating_sub(1);
        } else if key.is_char(' ') {
            if let Some(&i) = visible.get(self.cursor)
                && self.states[i] != RowState::Installed
            {
                self.install[i] = !self.install[i];
            }
        } else if key.is_char('a') || key.is_char('n') {
            let on = key.is_char('a');
            for i in self.changing() {
                self.install[i] = on;
            }
        } else if key.is_char('h') && self.installed_count() > 0 {
            self.show_installed = !self.show_installed;
            self.cursor = self.cursor.min(self.visible().len().saturating_sub(1));
        }
        PackageAction::None
    }

    /// Rows of the list, each `(row index, painted row)`.
    fn list_rows(&self, marks: &Marks, width: usize) -> Vec<Row> {
        let visible = self.visible();
        visible
            .iter()
            .enumerate()
            .map(|(pos, &i)| {
                let row = &self.rows[i];
                let focus = if pos == self.cursor { marks.focus } else { " " };
                let (check, note, tone) = match self.states[i] {
                    RowState::Installed => (
                        format!("{} ", marks.ok),
                        "already installed".to_string(),
                        Tone::Muted,
                    ),
                    _ if self.install[i] => (
                        "[x]".to_string(),
                        format!("from {}", row.group()),
                        Tone::Normal,
                    ),
                    _ => (
                        "[ ]".to_string(),
                        "skipped this time (not saved)".to_string(),
                        Tone::Muted,
                    ),
                };
                let name_w = width.saturating_sub(44).clamp(12, 32);
                vec![
                    seg(format!(" {focus} {check} "), Tone::Active),
                    seg(format!("{:<10}", row.manager), Tone::Secondary),
                    seg(
                        format!("{:<name_w$}", paint::fit(&row.name, name_w, marks.ellipsis)),
                        tone,
                    ),
                    seg(note, Tone::Muted),
                ]
            })
            .collect()
    }
}

const S2_PAGE: WidgetId = WidgetId::of("emc.s2.page");

/// Paints [`PackagesState`] and maps keys.
pub(crate) struct PackagesOwner {
    pub(crate) state: PackagesState,
}

impl PackagesOwner {
    pub(crate) fn new(state: PackagesState) -> Self {
        Self { state }
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        S2_PAGE
    }

    pub(crate) fn hints(&self) -> Vec<(&'static str, String)> {
        self.state.hints()
    }

    pub(crate) fn key(&mut self, key: &Key) -> Response<PackageAction> {
        let before = (
            self.state.cursor,
            self.state.show_installed,
            self.state.install.clone(),
        );
        let action = self.state.on_key(key);
        if action != PackageAction::None {
            return Response::emit(action);
        }
        let after = (
            self.state.cursor,
            self.state.show_installed,
            self.state.install.clone(),
        );
        if before == after {
            Response::from(Outcome::Ignored)
        } else {
            Response::changed()
        }
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let theme: Theme = *ctx.theme;
        let bg = theme.canvas;
        let marks = Marks::for_theme(&theme);
        paint::clear(buf, area, bg);
        render_step_header(
            2,
            "Review and confirm",
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let width = area.width.saturating_sub(2);
        let headline = if self.state.is_checking() {
            "Checking what this Mac already has...".to_string()
        } else if self.state.changing().is_empty() && self.state.settings_groups == 0 {
            "This Mac already matches. Applying again is safe: it only changes what differs."
                .to_string()
        } else {
            "This will change this Mac. Review, then apply.".to_string()
        };
        let lead = if self.state.is_checking() {
            format!("{} ", marks.frame(ctx.interaction.tick))
        } else {
            String::new()
        };
        put(
            buf,
            area.x + 1,
            area.y + 1,
            width,
            &[bold(format!("{lead}{headline}"), Tone::Normal)],
            &theme,
            bg,
        );
        let summary = self.state.summary();
        let box_area = Rect::new(area.x, area.y + 2, area.width, summary.len() as u16 + 2);
        let inner = framed(box_area, buf, &theme, bg, "What will happen");
        for (i, line) in summary.iter().enumerate() {
            put(
                buf,
                inner.x,
                inner.y + i as u16,
                inner.width,
                &[seg(line.clone(), Tone::Normal)],
                &theme,
                bg,
            );
        }
        let list_top = box_area.bottom();
        let list_bottom = area.bottom().saturating_sub(2);
        if !self.state.is_checking() && list_bottom > list_top + 1 {
            let changing = self.state.changing().len();
            let title = if changing == 0 {
                "No packages to install".to_string()
            } else {
                format!("Packages to install ({changing})")
            };
            let mut head = vec![bold(format!(" {title}"), Tone::Normal)];
            if changing > 0 {
                head.push(seg("   Space: Install / Skip this time", Tone::Muted));
            }
            put(buf, area.x, list_top, area.width, &head, &theme, bg);
            let rows = self.state.list_rows(&marks, width as usize);
            let room = (list_bottom - list_top - 1) as usize;
            let start = self.state.cursor.saturating_sub(room.saturating_sub(1));
            for (i, row) in rows.iter().skip(start).take(room).enumerate() {
                put(
                    buf,
                    area.x,
                    list_top + 1 + i as u16,
                    area.width,
                    row,
                    &theme,
                    bg,
                );
            }
            if rows.len() > start + room {
                put(
                    buf,
                    area.x,
                    list_bottom - 1,
                    area.width,
                    &[seg(
                        format!("   {} more below", rows.len() - start - room),
                        Tone::Muted,
                    )],
                    &theme,
                    bg,
                );
            }
        }
        let primary_y = area.bottom().saturating_sub(2);
        if self.state.is_checking() {
            put(
                buf,
                area.x + 1,
                primary_y,
                width,
                &[seg("Checking before you can apply...", Tone::Muted)],
                &theme,
                bg,
            );
        } else {
            primary_line(
                Rect::new(area.x, primary_y, area.width, 1),
                buf,
                &theme,
                bg,
                "Enter",
                "Apply now",
            );
        }
        footer(
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            &theme,
            &self.state.hints(),
        );
    }
}

/// Group rows by the first declaring module (used by tests and summaries).
pub fn rows_by_module(rows: &[PackageRow]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        map.entry(row.group().to_string())
            .or_default()
            .push(row.spec());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: KeyModifiers::empty(),
        }
    }

    fn ch(c: char) -> Key {
        key(KeyCode::Char(c))
    }

    fn origins() -> HashMap<ResId, Vec<String>> {
        let mut map = HashMap::new();
        for (kind, name, module) in [
            ("brew", "bat", "alpha"),
            ("brew", "jq", "alpha"),
            ("brew-cask", "example-editor", "beta"),
        ] {
            map.insert(
                ResId {
                    kind: kind.to_string(),
                    name: name.to_string(),
                },
                vec![module.to_string()],
            );
        }
        map
    }

    const STATUS: &str = r#"{"packages":{
      "brew":{"available":true,"packages":[
        {"package":"bat","desired_state":"present","state":"installed"},
        {"package":"jq","desired_state":"present","state":"missing"}]},
      "brew-cask":{"available":true,"packages":[
        {"package":"example-editor","desired_state":"present","state":"missing"}]}},
      "macos_defaults":{"entries":[{"domain":"d","key":"k","value":1,"current":"2","state":"differs"}]}}"#;

    fn reviewed() -> PackagesState {
        let rows = package_rows(&["alpha".to_string(), "beta".to_string()], &origins());
        let mut state = PackagesState::new(rows, 2).with_arm_delay(Duration::ZERO);
        state.apply_check(Ok(StatusReport::parse(STATUS, "t").unwrap()));
        state
    }

    #[test]
    fn lists_only_what_changes_and_counts_the_rest() {
        let state = reviewed();
        let shown: Vec<String> = state
            .visible()
            .iter()
            .map(|&i| state.rows[i].spec())
            .collect();
        assert_eq!(shown, vec!["brew:jq", "brew-cask:example-editor"]);
        let summary = state.summary().join("\n");
        assert!(
            summary.contains("2 modules · 2 packages to install (1 already installed) · 1 group of settings to update"),
            "{summary}"
        );
        assert!(summary.contains("administrator password once"), "{summary}");
    }

    #[test]
    fn space_toggles_install_and_skip_and_skipped_specs_are_named() {
        let mut state = reviewed();
        state.on_key(&key(KeyCode::Down));
        state.on_key(&ch(' '));
        assert_eq!(state.install_specs(), vec!["brew:jq"]);
        assert_eq!(state.skipped_specs(), vec!["brew-cask:example-editor"]);
        state.on_key(&ch('n'));
        assert!(state.install_specs().is_empty());
        state.on_key(&ch('a'));
        assert_eq!(state.install_specs().len(), 2);
    }

    #[test]
    fn h_shows_installed_packages_and_they_cannot_be_toggled() {
        let mut state = reviewed();
        state.on_key(&ch('h'));
        assert_eq!(state.visible().len(), 3);
        let installed = state.rows.iter().position(|r| r.name == "bat").unwrap();
        let pos = state
            .visible()
            .iter()
            .position(|&i| i == installed)
            .unwrap();
        state.cursor = pos;
        state.on_key(&ch(' '));
        assert!(
            state.will_install(installed),
            "installed rows keep their state"
        );
        assert!(state.skipped_specs().is_empty());
    }

    #[test]
    fn enter_is_ignored_while_checking_and_right_after_opening() {
        let rows = package_rows(&["alpha".to_string()], &origins());
        let mut checking = PackagesState::new(rows.clone(), 1).with_arm_delay(Duration::ZERO);
        assert_eq!(checking.on_key(&key(KeyCode::Enter)), PackageAction::None);
        let mut fresh = PackagesState::new(rows, 1).with_arm_delay(Duration::from_secs(60));
        fresh.apply_check(Ok(StatusReport::parse(STATUS, "t").unwrap()));
        assert_eq!(
            fresh.on_key(&key(KeyCode::Enter)),
            PackageAction::None,
            "held key"
        );
        let mut armed = reviewed();
        assert_eq!(armed.on_key(&key(KeyCode::Enter)), PackageAction::Apply);
    }

    #[test]
    fn failed_check_lists_every_package_and_says_so() {
        let rows = package_rows(&["alpha".to_string()], &origins());
        let mut state = PackagesState::new(rows, 1).with_arm_delay(Duration::ZERO);
        state.apply_check(Err("mise exited 1".to_string()));
        assert_eq!(state.visible().len(), 2);
        assert!(
            state
                .summary()
                .join("\n")
                .contains("Could not check this Mac (mise exited 1)")
        );
    }

    #[test]
    fn keys_back_and_quit() {
        let mut state = reviewed();
        assert_eq!(state.on_key(&key(KeyCode::Esc)), PackageAction::Back);
        assert_eq!(state.on_key(&ch('q')), PackageAction::Quit);
    }

    #[test]
    fn every_listed_key_does_something() {
        let base = reviewed();
        for (label, _) in base.hints() {
            let mut s = reviewed();
            let before = (s.cursor, s.show_installed, s.install.clone());
            let k = match label {
                "↑↓" => key(KeyCode::Down),
                "Space" => ch(' '),
                "a" => ch('n'),
                "n" => ch('n'),
                "h" => ch('h'),
                "Esc" => key(KeyCode::Esc),
                "q" => ch('q'),
                "?" => continue,
                other => panic!("unmapped hint {other}"),
            };
            let action = s.on_key(&k);
            assert!(
                action != PackageAction::None
                    || before != (s.cursor, s.show_installed, s.install.clone()),
                "{label} did nothing"
            );
        }
    }

    #[test]
    fn rows_dedupe_shared_packages_under_first_module() {
        let mut map = origins();
        map.insert(
            ResId {
                kind: "brew".into(),
                name: "bat".into(),
            },
            vec!["alpha".into(), "beta".into()],
        );
        let rows = package_rows(&["alpha".to_string(), "beta".to_string()], &map);
        assert_eq!(rows.iter().filter(|r| r.name == "bat").count(), 1);
        assert_eq!(rows_by_module(&rows)["alpha"].len(), 2);
    }
}
