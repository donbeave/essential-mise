//! Step 4: Done / Needs attention.
//!
//! The screen reads one typed [`StatusReport`](crate::status::StatusReport). It leads with a verdict and
//! ONE primary action, then groups what matters: "Needs your attention"
//! first, "Optional, for your information" next, and "All good" collapsed.
//! The state lives in [`ResultScreen`](crate::screens::result::ResultScreen) (pure, testable); `ResultOwner` only
//! paints it and turns keys into [`ResultAction`](crate::screens::result::ResultAction)s that the app executes.
//!
//! Exit semantics: 0 when nothing differs, 3 when changes are pending, 1 when
//! the check itself failed.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::response::Response;
use junie_tui::shell::clipboard::Clipboard;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use super::chrome::render_step_header;
use super::paint::{self, Marks, Row, bold, footer, primary_line, put, seg};
use crate::followup::{self, FollowUp};
use crate::status::{AreaKey, AreaStatus, StatusReport};

/// Exit code: nothing differs.
pub const EXIT_CLEAN: u8 = 0;
/// Exit code: changes are still pending, or the check failed.
pub const EXIT_PENDING: u8 = 3;
/// Exit code: the check failed.
pub const EXIT_FAILED: u8 = 1;

/// What the app should do after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultAction {
    /// Stay on the screen.
    None,
    /// Leave the console with this exit code.
    Quit(u8),
    /// Check this Mac again.
    Recheck,
    /// Copy this text to the clipboard.
    Copy(String),
    /// Apply again, for exactly the areas that differ.
    Reapply,
}

/// Which view is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Verdict and grouped sections.
    Summary,
    /// Every pending change as `setting | now | wanted`.
    Details,
}

/// Step 4 state.
pub struct ResultScreen {
    report: Option<StatusReport>,
    error: Option<String>,
    checks: u32,
    expanded: bool,
    view: View,
    cursor: usize,
    confirm_quit: bool,
    toast: Option<String>,
    checking: bool,
    reapplied: bool,
    still_different: Option<String>,
    reapply_commands: Vec<String>,
    log_follow_ups: Vec<FollowUp>,
}

impl ResultScreen {
    /// A screen with no check yet.
    pub fn new() -> Self {
        Self {
            report: None,
            error: None,
            checks: 0,
            expanded: false,
            view: View::Summary,
            cursor: 0,
            confirm_quit: false,
            toast: None,
            checking: true,
            reapplied: false,
            still_different: None,
            reapply_commands: Vec::new(),
            log_follow_ups: Vec::new(),
        }
    }

    /// Record follow-ups found in the apply log.
    pub fn set_log_follow_ups(&mut self, items: Vec<FollowUp>) {
        self.log_follow_ups = items;
    }

    /// Every follow-up: from the report, then from the apply log.
    pub fn follow_ups(&self) -> Vec<FollowUp> {
        let mut all = self
            .report
            .as_ref()
            .map(followup::from_report)
            .unwrap_or_default();
        for item in &self.log_follow_ups {
            if !all.contains(item) {
                all.push(item.clone());
            }
        }
        all
    }

    /// The toast to show after a copy.
    pub fn toast(&self) -> Option<&str> {
        self.toast.as_deref()
    }

    /// Show a short confirmation until the next key.
    pub fn set_toast(&mut self, text: impl Into<String>) {
        self.toast = Some(text.into());
    }

    /// The commands a re-apply would run (shown in the details view and
    /// copied by `c`); empty when nothing is pending.
    pub fn set_reapply_commands(&mut self, commands: Vec<String>) {
        self.reapply_commands = commands;
    }

    /// The commands a re-apply would run.
    pub fn reapply_commands(&self) -> &[String] {
        &self.reapply_commands
    }

    /// The next check follows a re-apply: if the same items still differ the
    /// screen stops offering another round.
    pub fn mark_reapplied(&mut self) {
        self.reapplied = true;
    }

    /// Items that still differ after a re-apply.
    pub fn still_different(&self) -> Option<&str> {
        self.still_different.as_deref()
    }

    /// A check has started (the screen shows a spinner until it finishes).
    pub fn begin_check(&mut self) {
        self.checking = true;
        self.confirm_quit = false;
        self.view = View::Summary;
    }

    /// Whether a check is running.
    pub fn is_checking(&self) -> bool {
        self.checking
    }

    /// Record a finished check.
    pub fn finish_check(&mut self, result: Result<StatusReport, String>) {
        self.checking = false;
        self.checks += 1;
        match result {
            Ok(report) => {
                self.report = Some(report);
                self.error = None;
            }
            Err(reason) => self.error = Some(reason),
        }
        self.cursor = 0;
        self.confirm_quit = false;
        self.still_different = None;
        let pending = self.pending_areas();
        if pending.is_empty() {
            self.view = View::Summary;
            self.reapplied = false;
        } else if self.reapplied {
            let mut items: Vec<String> = pending
                .iter()
                .flat_map(|a| {
                    a.changes()
                        .into_iter()
                        .map(|c| format!("{} ({})", c.item, a.key.label()))
                })
                .collect();
            let more = items.len().saturating_sub(3);
            items.truncate(3);
            let mut text = items.join(", ");
            if more > 0 {
                text.push_str(&format!(" and {more} more"));
            }
            self.still_different = Some(text);
        }
    }

    /// How many checks have finished.
    pub fn checks(&self) -> u32 {
        self.checks
    }

    /// The latest report.
    pub fn report(&self) -> Option<&StatusReport> {
        self.report.as_ref()
    }

    /// The latest error, when the last check failed.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The current view.
    pub fn view(&self) -> View {
        self.view
    }

    /// Whether "All good" is expanded.
    pub fn expanded(&self) -> bool {
        self.expanded
    }

    /// Areas that need changes.
    pub fn pending_areas(&self) -> Vec<&AreaStatus> {
        match (&self.error, &self.report) {
            (None, Some(report)) => report.needs_changes(),
            _ => Vec::new(),
        }
    }

    /// Exit code for "leave now": 0 clean, 3 pending, 1 failed.
    pub fn exit_code(&self) -> u8 {
        if self.error.is_some() || self.report.is_none() {
            EXIT_FAILED
        } else if self.pending_areas().is_empty() {
            EXIT_CLEAN
        } else {
            EXIT_PENDING
        }
    }

    /// The primary action as `(key, label)`; `None` while nothing can be done.
    pub fn primary(&self) -> Option<(&'static str, String)> {
        if self.checking {
            return None;
        }
        if self.error.is_some() {
            return Some(("Enter", "Check again".to_string()));
        }
        self.report.as_ref()?;
        let pending = self.pending_areas();
        Some(if pending.is_empty() {
            ("Enter", "Finish and exit".to_string())
        } else if self.still_different.is_some() || self.reapply_commands.is_empty() {
            ("Enter", "Finish anyway".to_string())
        } else {
            (
                "Enter",
                format!(
                    "Apply the {} pending {} again",
                    pending.len(),
                    plural(pending.len(), "area", "areas")
                ),
            )
        })
    }

    /// Footer hints for the keys that do something right now.
    pub fn hints(&self) -> Vec<(&'static str, String)> {
        let mut hints: Vec<(&'static str, String)> = Vec::new();
        if self.confirm_quit {
            return vec![("y", "quit anyway".into()), ("n", "stay".into())];
        }
        if self.view == View::Details {
            hints.push(("↑↓", "scroll".into()));
            if self.copy_text().is_some() {
                hints.push(("c", "copy command".into()));
            }
            hints.push(("Esc", "back".into()));
            hints.push(("?", "help".into()));
            return hints;
        }
        if self.checking || (self.report.is_none() && self.error.is_none()) {
            return vec![("?", "help".into()), ("q", "quit".into())];
        }
        if self.can_move() {
            hints.push(("↑↓", "move".into()));
        }
        if !self.pending_areas().is_empty() {
            hints.push(("d", "details".into()));
        }
        if self.can_expand() {
            hints.push(("e", "expand".into()));
        }
        if self.copy_text().is_some() {
            hints.push(("c", "copy command".into()));
        }
        hints.push(("R", "check again".into()));
        hints.push(("?", "help".into()));
        hints.push(("q", "quit".into()));
        hints
    }

    fn can_move(&self) -> bool {
        self.selectable_rows() > 1
    }

    fn can_expand(&self) -> bool {
        self.report
            .as_ref()
            .is_some_and(|r| !r.up_to_date().is_empty())
            && self.error.is_none()
    }

    fn commandable(&self) -> Vec<FollowUp> {
        self.follow_ups()
            .into_iter()
            .filter(|f| f.command().is_some())
            .collect()
    }

    fn selectable_rows(&self) -> usize {
        self.pending_areas().len() + self.commandable().len()
    }

    /// The command `c` copies: the focused row's command, else the first
    /// command on the screen.
    pub fn copy_text(&self) -> Option<String> {
        if self.error.is_some() {
            return None;
        }
        let areas = self.pending_areas().len();
        let items = self.commandable();
        if self.view == View::Summary
            && self.cursor >= areas
            && let Some(item) = items.get(self.cursor - areas)
        {
            return item.command();
        }
        if !self.reapply_commands.is_empty() && !self.pending_areas().is_empty() {
            return Some(crate::reapply::command_line(&self.reapply_commands));
        }
        items.first().and_then(FollowUp::command)
    }

    /// Handle a key.
    pub fn on_key(&mut self, key: &Key) -> ResultAction {
        self.toast = None;
        if self.checking {
            if key.is_char('q') || key.is(KeyCode::Esc) {
                return ResultAction::Quit(self.exit_code());
            }
            return ResultAction::None;
        }
        if self.confirm_quit {
            if key.is_char('y') || key.is_char('Y') {
                return ResultAction::Quit(self.exit_code());
            }
            self.confirm_quit = false;
            return ResultAction::None;
        }
        if self.view == View::Details {
            return self.on_details_key(key);
        }
        if key.is(KeyCode::Enter) {
            return self.primary_action();
        }
        if key.is_char('q') || key.is(KeyCode::Esc) {
            return self.try_quit();
        }
        if key.is_char('R') || key.is_char('r') {
            return ResultAction::Recheck;
        }
        if key.is_char('c')
            && let Some(text) = self.copy_text()
        {
            return ResultAction::Copy(text);
        }
        if key.is_char('e') && self.can_expand() {
            self.expanded = !self.expanded;
        } else if key.is_char('d') && !self.pending_areas().is_empty() {
            self.view = View::Details;
            self.cursor = 0;
        } else if key.is(KeyCode::Down) && self.can_move() {
            self.cursor = (self.cursor + 1).min(self.selectable_rows() - 1);
        } else if key.is(KeyCode::Up) && self.can_move() {
            self.cursor = self.cursor.saturating_sub(1);
        }
        ResultAction::None
    }

    fn on_details_key(&mut self, key: &Key) -> ResultAction {
        if key.is(KeyCode::Enter) {
            return self.primary_action();
        }
        if key.is_char('c')
            && let Some(text) = self.copy_text()
        {
            return ResultAction::Copy(text);
        }
        if key.is(KeyCode::Esc) || key.is_char('q') {
            self.view = View::Summary;
            self.cursor = 0;
        } else if key.is(KeyCode::Down) {
            self.cursor += 1;
        } else if key.is(KeyCode::Up) {
            self.cursor = self.cursor.saturating_sub(1);
        }
        ResultAction::None
    }

    fn primary_action(&mut self) -> ResultAction {
        if self.error.is_some() {
            return ResultAction::Recheck;
        }
        if self.report.is_none() {
            return ResultAction::None;
        }
        if !self.pending_areas().is_empty()
            && self.still_different.is_none()
            && !self.reapply_commands.is_empty()
        {
            return ResultAction::Reapply;
        }
        ResultAction::Quit(self.exit_code())
    }

    /// `q`/`Esc`: leave at once unless changes are pending.
    fn try_quit(&mut self) -> ResultAction {
        if self.pending_areas().is_empty() {
            ResultAction::Quit(self.exit_code())
        } else {
            self.confirm_quit = true;
            ResultAction::None
        }
    }

    /// Whether the "quit anyway?" question is open.
    pub fn confirming_quit(&self) -> bool {
        self.confirm_quit
    }

    fn verdict_row(&self, marks: &Marks) -> Row {
        if let Some(reason) = &self.error {
            return vec![
                bold(format!("{} ", marks.fail), Tone::Error),
                bold(format!("Could not check: {reason}"), Tone::Error),
            ];
        }
        let Some(report) = &self.report else {
            return vec![seg("Checking your Mac", Tone::Muted)];
        };
        let pending = report.needs_changes().len();
        let good = report.up_to_date().len();
        let unknown = report.unknown().len();
        if pending == 0 {
            let mut text = format!(
                "Your Mac is set up.  {} {} checked, all up to date.",
                report.areas.len() - unknown,
                plural(report.areas.len() - unknown, "area", "areas")
            );
            if unknown > 0 {
                text = format!(
                    "Your Mac is set up.  {good} up to date, {unknown} could not be checked."
                );
            }
            vec![
                bold(format!("{} ", marks.ok), Tone::Success),
                bold(text, Tone::Success),
            ]
        } else {
            vec![
                bold(format!("{} ", marks.ok), Tone::Success),
                seg(
                    format!("{good} of {} areas up to date", report.areas.len()),
                    Tone::Normal,
                ),
                seg("     ", Tone::Normal),
                bold(format!("{} ", marks.warn), Tone::Warning),
                bold(
                    format!(
                        "{pending} need{} changes",
                        if pending == 1 { "s" } else { "" }
                    ),
                    Tone::Warning,
                ),
            ]
        }
    }

    /// What to print to the normal terminal after the console closes, so the
    /// outcome and the commands survive the alternate screen. Plain text,
    /// one line each; empty until a check has finished.
    pub fn exit_lines(&self) -> Vec<String> {
        let marks = Marks::unicode();
        let mut lines = Vec::new();
        if let Some(reason) = &self.error {
            lines.push(format!("Could not check this Mac: {reason}"));
            return lines;
        }
        let Some(report) = &self.report else {
            return lines;
        };
        let verdict = paint::row_text(&self.verdict_row(&marks));
        lines.push(verdict.split_whitespace().collect::<Vec<_>>().join(" "));
        for area in self.pending_areas() {
            lines.push(format!("Needs changes: {}", area.key.label()));
            for change in area.changes() {
                lines.push(format!(
                    "  {} (now {}, wanted {})",
                    change.item, change.now, change.wanted
                ));
            }
        }
        if let Some(items) = &self.still_different {
            lines.push(format!("Still different after applying: {items}"));
        }
        for item in self.follow_ups() {
            lines.push(item.text());
            if let Some(command) = item.command() {
                lines.push(format!("  $ {command}"));
            }
        }
        for area in report.unknown() {
            lines.push(format!(
                "Could not check {} automatically",
                area.key.label()
            ));
        }
        if !self.pending_areas().is_empty() && !self.reapply_commands.is_empty() {
            lines.push("To apply the remaining changes:".to_string());
            for command in &self.reapply_commands {
                lines.push(format!("  $ {command}"));
            }
        }
        lines
    }

    /// The title shown in the header: Done, Needs attention, or Checking.
    pub fn step_name(&self) -> &'static str {
        if self.checking {
            return "Checking";
        }
        match (&self.error, &self.report) {
            (Some(_), _) => "Needs attention",
            (None, None) => "Checking",
            (None, Some(_)) if self.pending_areas().is_empty() => "Done",
            _ => "Needs attention",
        }
    }

    fn attention_rows(&self, marks: &Marks, width: usize) -> Vec<Row> {
        let areas = self.pending_areas();
        let focusable = areas.len() > 1;
        areas
            .iter()
            .enumerate()
            .map(|(i, area)| attention_row(area, marks, focusable && i == self.cursor, width))
            .collect()
    }

    fn optional_rows(&self, marks: &Marks, width: usize) -> Vec<Row> {
        let mut rows = Vec::new();
        let areas = self.pending_areas().len();
        let focusable = self.selectable_rows() > 1;
        let mut command_index = 0;
        for item in self.follow_ups() {
            let command = item.command();
            let focused = focusable
                && command.is_some()
                && self.cursor == areas + command_index
                && self.cursor >= areas;
            if command.is_some() {
                command_index += 1;
            }
            for (i, part) in paint::wrap(&item.text(), width.saturating_sub(8).max(10))
                .into_iter()
                .enumerate()
            {
                if i == 0 {
                    rows.push(vec![
                        seg(
                            format!(" {} ", if focused { marks.focus } else { " " }),
                            Tone::Active,
                        ),
                        seg(format!("{} ", marks.bullet), Tone::Muted),
                        seg(part, Tone::Normal),
                    ]);
                } else {
                    rows.push(vec![seg(format!("     {part}"), Tone::Normal)]);
                }
            }
            if let Some(command) = command {
                let room = width.saturating_sub(10).max(10);
                for (i, part) in paint::wrap(&command, room).into_iter().enumerate() {
                    let lead = if i == 0 { "$ " } else { "  " };
                    rows.push(vec![seg(format!("       {lead}{part}"), Tone::Secondary)]);
                }
            }
        }
        if let Some(report) = &self.report {
            for area in report.unknown() {
                rows.push(bullet(
                    marks,
                    &format!("Could not check {} automatically", area.key.label()),
                ));
            }
        }
        rows
    }

    /// Lines of the summary view, top to bottom.
    pub fn summary_lines(&self, marks: &Marks, width: usize) -> Vec<Row> {
        let mut lines = Vec::new();
        let attention = self.attention_rows(marks, width);
        if let Some(items) = &self.still_different {
            let text =
                format!("Still different after applying: {items}. They may be managed elsewhere.");
            for part in paint::wrap(&text, width.saturating_sub(3)) {
                lines.push(vec![seg(format!(" {part}"), Tone::Warning)]);
            }
            lines.push(Vec::new());
        }
        if !attention.is_empty() {
            lines.push(heading(&format!(
                "Needs your attention ({})",
                attention.len()
            )));
            lines.extend(attention);
            lines.push(Vec::new());
        }
        let optional = self.optional_rows(marks, width);
        if !optional.is_empty() {
            lines.push(heading(&format!(
                "Optional, for your information ({})",
                optional.len()
            )));
            lines.extend(optional);
            lines.push(Vec::new());
        }
        if let Some(report) = &self.report {
            let good = report.up_to_date();
            if !good.is_empty() {
                let glyph = if self.expanded {
                    marks.open
                } else {
                    marks.closed
                };
                let hint = if self.expanded {
                    "e to collapse"
                } else {
                    "e to expand"
                };
                lines.push(vec![
                    seg(format!(" {glyph} "), Tone::Muted),
                    bold(format!("All good ({} areas)", good.len()), Tone::Normal),
                    seg(format!("   {hint}"), Tone::Muted),
                ]);
                if self.expanded {
                    for area in good {
                        lines.push(vec![
                            seg(format!("     {} ", marks.ok), Tone::Success),
                            seg(format!("{:<20}", area.key.label()), Tone::Normal),
                            seg(good_note(area), Tone::Muted),
                        ]);
                    }
                }
            }
        }
        lines
    }

    /// Lines of the details view: every pending change, grouped by area.
    pub fn details_lines(&self, marks: &Marks, width: usize) -> Vec<Row> {
        let mut lines = Vec::new();
        let pending = self.pending_areas();
        lines.push(heading(&format!(
            "Changes to apply ({})",
            pending.iter().map(|a| a.changes().len()).sum::<usize>()
        )));
        for area in pending {
            let changes = area.changes();
            lines.push(Vec::new());
            lines.push(vec![
                bold(
                    format!(" {} {}", marks.warn, area.key.label()),
                    Tone::Warning,
                ),
                seg(
                    format!("  {} of {}", changes.len(), area.total()),
                    Tone::Muted,
                ),
            ]);
            let item_w = changes
                .iter()
                .map(|c| c.item.chars().count())
                .max()
                .unwrap_or(0)
                .clamp(7, width.saturating_sub(30).max(7));
            let now_w = changes
                .iter()
                .map(|c| c.now.chars().count())
                .max()
                .unwrap_or(0)
                .clamp(3, 18);
            lines.push(vec![seg(
                format!("   {:<item_w$}  {:<now_w$}  wanted", "setting", "now"),
                Tone::Muted,
            )]);
            for change in changes {
                lines.push(vec![
                    seg("   ", Tone::Normal),
                    seg(
                        format!(
                            "{:<item_w$}",
                            paint::fit(&change.item, item_w, marks.ellipsis)
                        ),
                        Tone::Normal,
                    ),
                    seg("  ", Tone::Normal),
                    seg(
                        format!("{:<now_w$}", paint::fit(&change.now, now_w, marks.ellipsis)),
                        Tone::Warning,
                    ),
                    seg("  ", Tone::Normal),
                    seg(change.wanted.clone(), Tone::Success),
                ]);
            }
        }
        if !self.reapply_commands.is_empty() {
            lines.push(Vec::new());
            lines.push(heading("Command that Enter runs"));
            for command in &self.reapply_commands {
                for (i, part) in paint::wrap(command, width.saturating_sub(8))
                    .into_iter()
                    .enumerate()
                {
                    let lead = if i == 0 { "$ " } else { "  " };
                    lines.push(vec![seg(format!("   {lead}{part}"), Tone::Secondary)]);
                }
            }
        }
        lines
    }
}

impl Default for ResultScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

fn heading(text: &str) -> Row {
    vec![bold(format!(" {text}"), Tone::Normal)]
}

fn bullet(marks: &Marks, text: &str) -> Row {
    vec![
        seg(format!("   {} ", marks.bullet), Tone::Muted),
        seg(text.to_string(), Tone::Normal),
    ]
}

fn noun(key: AreaKey) -> (&'static str, &'static str) {
    match key {
        AreaKey::MacosDefaults => ("value", "values"),
        AreaKey::LoginItems => ("item", "items"),
        AreaKey::Packages => ("package", "packages"),
        AreaKey::Tools => ("tool", "tools"),
        AreaKey::Dotfiles => ("file", "files"),
        AreaKey::Repos => ("repository", "repositories"),
        _ => ("entry", "entries"),
    }
}

fn good_note(area: &AreaStatus) -> String {
    let (one, many) = noun(area.key);
    let total = area.total();
    let mut note = format!("{total} {}", plural(total, one, many));
    if area.skipped() > 0 {
        note.push_str(&format!(", {} skipped this time", area.skipped()));
    }
    note
}

fn attention_row(area: &AreaStatus, marks: &Marks, focused: bool, width: usize) -> Row {
    let changes = area.changes();
    let (one, many) = noun(area.key);
    let verb = if changes.len() == 1 {
        "differs"
    } else {
        "differ"
    };
    let count = format!(
        "{} of {} {} {verb}",
        changes.len(),
        area.total(),
        plural(area.total(), one, many)
    );
    let more = changes.len().saturating_sub(2);
    let tail = if more > 0 {
        format!("; +{more} more (d)")
    } else {
        String::new()
    };
    let full = changes
        .iter()
        .take(2)
        .map(|c| format!("{} (now {}, wanted {})", c.item, c.now, c.wanted))
        .collect::<Vec<_>>()
        .join("; ");
    let names = changes
        .iter()
        .take(2)
        .map(|c| c.item.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let label = format!("{:<17}", area.key.label());
    let room = width.saturating_sub(6 + label.chars().count());
    let candidates = [
        format!("{count}: {full}{tail}"),
        format!("{count}: {names}{tail}"),
        format!("{count} (d for details)"),
    ];
    let detail = candidates
        .iter()
        .find(|c| c.chars().count() <= room)
        .unwrap_or(&candidates[2])
        .clone();
    vec![
        seg(
            format!(" {} ", if focused { marks.focus } else { " " }),
            Tone::Active,
        ),
        bold(format!("{} ", marks.warn), Tone::Warning),
        bold(label, Tone::Normal),
        seg(detail, Tone::Normal),
    ]
}

const S4_PAGE: WidgetId = WidgetId::of("emc.s4.page");

/// Paints [`ResultScreen`] and maps keys to [`ResultAction`]s.
pub(crate) struct ResultOwner {
    pub(crate) state: ResultScreen,
}

impl ResultOwner {
    pub(crate) fn new(state: ResultScreen) -> Self {
        Self { state }
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        S4_PAGE
    }

    pub(crate) fn key(&mut self, key: &Key, clipboard: &Clipboard) -> Response<ResultAction> {
        let before = self.state.summary_snapshot();
        let mut action = self.state.on_key(key);
        if let ResultAction::Copy(text) = &action {
            clipboard.copy(text);
            self.state.set_toast("Copied to the clipboard");
            action = ResultAction::None;
        }
        match action {
            ResultAction::None
                if before == self.state.summary_snapshot() && self.state.toast().is_none() =>
            {
                Response::from(Outcome::Ignored)
            }
            ResultAction::None => Response::changed(),
            other => Response::emit(other),
        }
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let theme: Theme = *ctx.theme;
        let bg = theme.canvas;
        let marks = Marks::for_theme(&theme);
        paint::clear(buf, area, bg);
        render_step_header(
            4,
            self.state.step_name(),
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let body_h = area.height.saturating_sub(3) as usize;
        let width = area.width.saturating_sub(2);
        let mut lines = vec![self.state.verdict_row(&marks)];
        if self.state.is_checking() {
            lines = vec![vec![
                seg(
                    format!("{} ", marks.frame(ctx.interaction.tick)),
                    Tone::Active,
                ),
                bold(
                    "Checking your Mac".to_string() + marks.ellipsis,
                    Tone::Normal,
                ),
            ]];
        } else if let Some(report) = self.state.report() {
            let stamp = if self.state.checks() > 1 {
                format!(
                    "Checked {} (check #{})",
                    report.checked_at,
                    self.state.checks()
                )
            } else {
                format!("Checked {}", report.checked_at)
            };
            let used: usize = lines[0].iter().map(|s| s.text.chars().count()).sum();
            if used + stamp.chars().count() + 3 <= width as usize {
                let pad = width as usize - used - stamp.chars().count();
                lines[0].push(seg(" ".repeat(pad), Tone::Normal));
                lines[0].push(seg(stamp, Tone::Muted));
            } else {
                lines.push(vec![seg(format!(" {stamp}"), Tone::Muted)]);
            }
        }
        lines.push(Vec::new());
        match self.state.view() {
            _ if self.state.is_checking() => {}
            View::Summary => lines.extend(self.state.summary_lines(&marks, width as usize)),
            View::Details => lines.extend(self.state.details_lines(&marks, width as usize)),
        }
        let skip = if lines.len() > body_h {
            self.state.scroll_for(lines.len(), body_h)
        } else {
            0
        };
        for (i, row) in lines.iter().skip(skip).take(body_h).enumerate() {
            put(
                buf,
                area.x + 1,
                area.y + 1 + i as u16,
                width,
                row,
                &theme,
                bg,
            );
        }
        let primary_y = area.bottom().saturating_sub(2);
        if self.state.confirming_quit() {
            let n = self.state.pending_areas().len();
            let question = format!(
                "Quit with {n} {} still needing changes?  y yes, n no",
                plural(n, "area", "areas")
            );
            put(
                buf,
                area.x + 1,
                primary_y,
                width,
                &[bold(question, Tone::Warning)],
                &theme,
                bg,
            );
        } else if let Some((key, label)) = self.state.primary() {
            primary_line(
                Rect::new(area.x, primary_y, area.width, 1),
                buf,
                &theme,
                bg,
                key,
                &label,
            );
        }
        if let Some(toast) = self.state.toast() {
            let len = toast.chars().count() as u16;
            if len + 2 < area.width {
                put(
                    buf,
                    area.right() - len - 1,
                    primary_y,
                    len,
                    &[bold(toast.to_string(), Tone::Success)],
                    &theme,
                    bg,
                );
            }
        }
        footer(
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            &theme,
            &self.state.hints(),
        );
    }
}

impl ResultScreen {
    /// A cheap fingerprint of what the screen shows, so the owner can tell
    /// whether a key changed anything.
    fn summary_snapshot(&self) -> (View, bool, usize, bool, u32) {
        (
            self.view,
            self.expanded,
            self.cursor,
            self.confirm_quit,
            self.checks,
        )
    }

    /// First visible line for a scrolled view.
    fn scroll_for(&self, total: usize, height: usize) -> usize {
        let max = total.saturating_sub(height);
        match self.view {
            View::Details => self.cursor.min(max),
            View::Summary => (self.cursor + 4).saturating_sub(height).min(max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::StatusReport;
    use ratatui::crossterm::event::KeyModifiers;

    fn fixture(name: &str) -> StatusReport {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .expect("fixture");
        StatusReport::parse(&text, "12:04:31").expect("parses")
    }

    fn screen(name: &str) -> ResultScreen {
        let mut s = ResultScreen::new();
        s.finish_check(Ok(fixture(name)));
        s
    }

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: KeyModifiers::empty(),
        }
    }

    fn ch(c: char) -> Key {
        key(KeyCode::Char(c))
    }

    fn text(lines: &[Row]) -> String {
        lines
            .iter()
            .map(|r| paint::row_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn clean_result_is_done_with_finish_and_exit_zero() {
        let mut s = screen("status-applied.json");
        assert_eq!(s.step_name(), "Done");
        assert_eq!(s.primary().unwrap().1, "Finish and exit");
        assert_eq!(s.exit_code(), EXIT_CLEAN);
        assert_eq!(s.on_key(&key(KeyCode::Enter)), ResultAction::Quit(0));
        assert_eq!(s.on_key(&ch('q')), ResultAction::Quit(0));
        assert_eq!(s.on_key(&key(KeyCode::Esc)), ResultAction::Quit(0));
    }

    #[test]
    fn pending_result_names_the_difference_and_exits_three() {
        let mut s = screen("status-one-differs.json");
        assert_eq!(s.step_name(), "Needs attention");
        let lines = text(&s.summary_lines(&Marks::unicode(), 100));
        assert!(lines.contains("Needs your attention (1)"), "{lines}");
        assert!(lines.contains("1 of 67 values differs"), "{lines}");
        assert!(
            lines.contains("com.apple.dock tilesize (now 48, wanted 36)"),
            "{lines}"
        );
        assert!(lines.find("Needs your attention") < lines.find("All good"));
        assert_eq!(s.exit_code(), EXIT_PENDING);
        // q asks first, then y quits with 3; n stays.
        assert_eq!(s.on_key(&ch('q')), ResultAction::None);
        assert!(s.confirming_quit());
        assert_eq!(s.on_key(&ch('n')), ResultAction::None);
        assert!(!s.confirming_quit());
        s.on_key(&key(KeyCode::Esc));
        assert_eq!(s.on_key(&ch('y')), ResultAction::Quit(3));
    }

    #[test]
    fn all_good_is_collapsed_and_e_expands_it() {
        let mut s = screen("status-one-differs.json");
        let marks = Marks::unicode();
        assert!(text(&s.summary_lines(&marks, 100)).contains("All good (5 areas)"));
        assert!(!text(&s.summary_lines(&marks, 100)).contains("Dotfiles"));
        s.on_key(&ch('e'));
        assert!(text(&s.summary_lines(&marks, 100)).contains("Dotfiles"));
    }

    #[test]
    fn details_view_lists_setting_now_wanted_and_esc_goes_back() {
        let mut s = screen("status-one-differs.json");
        s.on_key(&ch('d'));
        assert_eq!(s.view(), View::Details);
        let lines = text(&s.details_lines(&Marks::unicode(), 100));
        assert!(lines.contains("setting"), "{lines}");
        assert!(lines.contains("com.apple.dock tilesize"), "{lines}");
        s.on_key(&key(KeyCode::Esc));
        assert_eq!(s.view(), View::Summary);
    }

    #[test]
    fn d_is_ignored_when_nothing_is_pending() {
        let mut s = screen("status-applied.json");
        s.on_key(&ch('d'));
        assert_eq!(s.view(), View::Summary);
        assert!(s.hints().iter().all(|(k, _)| *k != "d"));
    }

    #[test]
    fn failed_check_offers_check_again_and_exits_one() {
        let mut s = ResultScreen::new();
        s.finish_check(Err("mise exited 1".to_string()));
        assert_eq!(s.step_name(), "Needs attention");
        assert_eq!(s.primary().unwrap().1, "Check again");
        assert_eq!(s.on_key(&key(KeyCode::Enter)), ResultAction::Recheck);
        assert_eq!(s.exit_code(), EXIT_FAILED);
        assert_eq!(s.on_key(&ch('q')), ResultAction::Quit(1));
    }

    #[test]
    fn unknown_areas_are_not_pending() {
        let mut s = ResultScreen::new();
        let raw = r#"{"mystery":[{"a":1}]}"#;
        s.finish_check(Ok(StatusReport::parse(raw, "t").unwrap()));
        assert_eq!(s.exit_code(), EXIT_CLEAN);
        let lines = text(&s.summary_lines(&Marks::unicode(), 100));
        assert!(
            lines.contains("Could not check Other settings automatically"),
            "{lines}"
        );
    }

    #[test]
    fn every_listed_key_does_something() {
        for name in ["status-applied.json", "status-one-differs.json"] {
            let base = screen(name);
            for (key_label, _) in base.hints() {
                let mut s = screen(name);
                let before = s.summary_snapshot();
                let k = match key_label {
                    "↑↓" => key(KeyCode::Down),
                    "d" => ch('d'),
                    "e" => ch('e'),
                    "R" => ch('R'),
                    "q" => ch('q'),
                    "?" => continue,
                    other => panic!("unmapped hint {other}"),
                };
                let action = s.on_key(&k);
                assert!(
                    action != ResultAction::None || before != s.summary_snapshot(),
                    "{name}: {key_label} did nothing"
                );
            }
        }
    }
}
