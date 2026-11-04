# Resume prompt — essential-mise-console spec

Paste into a fresh session to continue this work:

```
Continue the essential-mise-console spec in
~/.config/mise/docs/specs/essential-mise-console/ (README.md, spec.md).

Context: essential-mise is a mise config repo (mise bootstrap modules in
config.<name>.toml). We are specifying a thin Rust Ratatui TUI frontend
over `mise bootstrap`, built on Termrock (junie-tui) as a component
library — same split as tailrocks/terminal-components-baseline
apps/jackin (jackin-console): our crate holds screens + a Backend trait
that shells out to `mise bootstrap plan/status --json` and apply
commands; all install logic stays stock mise; TOML stays the source of
truth. P0 (bash per-package deselect in bootstrap.sh) ships before any
Rust work; P1 starts only after baseline PR #1
(tailrocks/terminal-components-baseline#1) merges and pins its rev.
Decisions locked and open questions are in README.md §Open questions
and spec.md §10.

First: read README.md and spec.md fully, then tell me which open
question you want to resolve and propose the smallest next step. Do not
write code until an open question is resolved. Verify every factual
claim about junie-tui APIs and mise CLI flags against the live checkout
(/tmp/tcb may be stale — re-clone if needed) and `mise help` before
stating it. Keep responses short. Never leave a PR you create unmerged.
```
