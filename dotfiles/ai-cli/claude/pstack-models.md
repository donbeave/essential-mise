# pstack model configuration

Per-role model overrides for pstack skills. Each pstack SKILL.md names its defaults in a Models section; the values here override those defaults. Delete a line to fall back to the skill default. A value of `inherit-parent` or `auto` runs that role on the parent session's model (the `Agent` call omits `model`); an alias entry in a panel list still counts toward that panel's fan-out. A model may carry a reasoning effort, as in `opus @xhigh` (levels: low, medium, high, xhigh, max); the role's subagent then runs at that level, each entry of a panel list on its own. `default effort` sets the level for a value without one; `session` uses the runtime's platform mapping to start from the parent session's effort. `session hook: off` stops the Claude Code SessionStart hook from injecting the poteto-mode mandate; any other value, or no line, leaves it on.

Policy (see ~/.claude/CLAUDE.md, "Model and effort"): Sonnet by default, Haiku for read-and-collect roles, one Opus seat in panels for cross-model diversity, Fable never by default.

feature, refactoring: sonnet @high
bug-fix: sonnet @high
perf-issue: sonnet @high
hillclimb: sonnet @high
judgment and prose: sonnet @high
strongest judgment: opus @high
how explorer: haiku @medium
how explainer: sonnet @medium
why investigators: haiku @medium
why synthesizer: sonnet @high
reflect tooling: haiku @medium
reflect judgment, divergent, synthesizer: sonnet @high
arena runners: sonnet @high, opus @medium, haiku @medium
arena cross-judge pool: sonnet @high, opus @medium
swarm workers: sonnet @high
architect runners: sonnet @high, opus @medium
interrogate reviewers: sonnet @high, opus @medium, haiku @medium

default effort: high
session hook: on
