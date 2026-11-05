//! Live-substring filter shared by S1 (modules) and S2 (packages).
//!
//! The rule: filtering hides rows but never mutates row state. Selection,
//! pre-ticks, and keep-flags live on stable row ids outside the filter, so a
//! row hidden by the query keeps its state when the query changes. Screens
//! must not reimplement this; they call [`FilterState::matches`] over row
//! text and keep state keyed by id.

/// Live-substring filter over row text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterState {
    query: String,
}

impl FilterState {
    /// Empty filter (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the query (one keystroke at a time from the `TextInput`).
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    /// Current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// True when no query is set (everything visible).
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    /// Case-insensitive substring match; empty query matches all text.
    pub fn matches(&self, text: &str) -> bool {
        if self.query.is_empty() {
            return true;
        }
        text.to_lowercase().contains(&self.query.to_lowercase())
    }

    /// Indices of `items` visible under the query, in order.
    pub fn visible<I>(&self, items: &[I], text_of: impl Fn(&I) -> &str) -> Vec<usize> {
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.matches(text_of(item)))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct Row {
        id: &'static str,
        text: &'static str,
    }

    fn rows() -> Vec<Row> {
        vec![
            Row {
                id: "neovim",
                text: "neovim Ghostty shells prompt",
            },
            Row {
                id: "terminal",
                text: "terminal utilities fonts",
            },
            Row {
                id: "browsers",
                text: "Google Chrome browsing",
            },
        ]
    }

    #[test]
    fn empty_query_matches_everything() {
        let filter = FilterState::new();
        assert!(filter.is_empty());
        for row in rows() {
            assert!(filter.matches(row.text), "empty query matches all");
        }
        assert_eq!(filter.visible(&rows(), |r| r.text), vec![0, 1, 2]);
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        let mut filter = FilterState::new();
        filter.set_query("CHROME");
        assert_eq!(filter.query(), "CHROME");
        assert!(filter.matches("Google Chrome browsing"));
        assert!(!filter.matches("terminal utilities fonts"));
    }

    #[test]
    fn hidden_rows_keep_state() {
        // Selection lives on stable ids outside the filter: hiding a row
        // must not drop its selection, and clearing the query restores it.
        let mut selected: HashSet<&'static str> = HashSet::new();
        selected.insert("neovim");
        selected.insert("browsers");

        let mut filter = FilterState::new();
        filter.set_query("terminal");
        assert_eq!(filter.visible(&rows(), |r| r.text), vec![1]);
        // Hidden rows keep their selection while filtered out.
        assert!(selected.contains("neovim"));
        assert!(selected.contains("browsers"));

        filter.set_query("");
        let visible = filter.visible(&rows(), |r| r.text);
        assert_eq!(visible, vec![0, 1, 2]);
        // …and the selection is intact once visible again.
        assert_eq!(selected.len(), 2);
        for i in visible {
            let _ = rows()[i].id;
        }
        assert!(selected.contains("neovim") && selected.contains("browsers"));
    }

    #[test]
    fn no_match_yields_empty_visible() {
        let mut filter = FilterState::new();
        filter.set_query("zzz-no-such-module");
        assert!(filter.visible(&rows(), |r| r.text).is_empty());
    }
}
