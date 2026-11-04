# Engineering policy

- Judge work by correctness, consistency, and project fit. Do not defer a known-wrong state because of cost, effort, or claims that it is marginal.
- Before fixing a bug, identify why the architecture allowed it and whether the same structure permits related bugs. Prefer fixes that remove the enabling condition.
- When uncertain, inspect, test, and measure before acting.
- Decompose independent work and use parallel research, implementation, review, and verification when the host supports them.
- When committing, verify the effective Git committer identity is Alexey Zhokhov <alexey@zhokhov.com>. Use git commit -s so Git adds exactly one Signed-off-by trailer; never add or duplicate it manually.

# Subagents

Definitions live in `~/.claude/agents/tailrocks-*.md`. This section is for the main session. Roster agents skip this file (`omitClaudeMd: true`) and get what they need from `repo-context`. No subagent can spawn another (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=1`), and the built-in Explore and Plan agents are denied: use @"tailrocks-locator (agent)" and @"tailrocks-architect (agent)".

The agents are generic roles. None of them is tied to a project, library, vendor or model, and no agent name contains a model name. The brief you send decides where an agent works. Every agent except @"tailrocks-locator (agent)" preloads the `repo-context` skill (`~/.claude/skills/repo-context/SKILL.md`), which makes it:
1. resolve the repo and scope from the brief, and stop with `Needs from caller:` if they are missing;
2. read that repo's `AGENTS.md` and `CLAUDE.md` explicitly, plus the matching note in `~/.claude/project-notes/` (indexed by git remote);
3. discover gate commands from the repo itself;
4. follow the shared scope, verification and commit rules, and report under fixed headings.

Project-specific knowledge belongs in the repo's own `AGENTS.md`, or in a project note when it cannot go in the repo. Language, library and vendor specifics belong in stack skills (`rust-practices`, `typescript-react-practices`, `tanstack-start`, `openshell`, `github-actions`), which `repo-context` invokes by detecting the stack. None of it goes in an agent file. To onboard a new repo, add a note and a row to `~/.claude/project-notes/README.md`.

Referencing agents: write every subagent reference in Claude Code's mention form, exactly as the @-mention typeahead inserts it and without backticks: @"tailrocks-verifier (agent)". The `(agent)` suffix tells any reader, human or AI agent, that this is a subagent. `CLAUDE.md` keeps this form as text; it is not read as a file import. Use the form in briefs, plans, reports, docs, commit messages and PR descriptions. The bare `name` appears only where a tool needs the identifier: frontmatter `name:`, the Agent tool's `subagent_type`, and code. Express a model choice as a per-call override (@"tailrocks-architect (agent)" with `model: fable, effort: high`), never as part of a name.

### Brief contract

Every brief must state:
- **Repo**: an absolute path, plus any sibling repos the change spans; branch, worktree or slot.
- **Scope**: the files, crates, packages or diff range the agent owns; for reviews, the exact `<base>..HEAD` or PR number.
- **Task**: the goal, inputs to read (spec, design, draft), and what is already decided or ruled out.
- **Gate**: the check that proves the work, or "the repo's fast gate".
- **Return**: what you want back, if more than the standard report.
- **Permissions**: whether it may commit, and separately whether it may push (both default to no; commit permission does not imply push, even when the repo says to push after every commit).

## Routing: which agent for which work

| Work | Agent |
|---|---|
| Find files, symbols, call sites, config (instead of built-in Explore) | @"tailrocks-locator (agent)" |
| Any library/tool/service/version/API question; choosing or bumping a dependency; upstream blocker reports | @"tailrocks-docs-researcher (agent)" |
| Design before non-trivial work: modules/crates/packages, contracts, routing and data loading, cross-repo changes, migrations; one phase-level decision (decision mode) | @"tailrocks-architect (agent)" |
| Backend, systems, library, CLI and TUI implementation | @"tailrocks-systems-engineer (agent)" |
| Web UI implementation: components, routes, data loading, design-system packages | @"tailrocks-frontend-engineer (agent)" |
| Sandboxes and isolation: containers, namespaces, mounts, egress, credentials, images | @"tailrocks-sandbox-engineer (agent)" |
| CI/CD, workflow generators, runners, toolchain pinning, hooks, releases, images | @"tailrocks-cicd-engineer (agent)" |
| Map drafts to design-system components; visual review from screenshots | @"tailrocks-design-director (agent)" |
| Flows, route/deep-link map, state matrix, spec critique | @"tailrocks-ux-specialist (agent)" |
| Fully specified mechanical edits: lint fixes, codemods, headers, chosen pin bumps | @"tailrocks-mechanic (agent)" |
| Bug or CI failure with unknown cause, flaky test, regression | @"tailrocks-debugger (agent)" |
| Performance budgets, stress scenarios, profiling | @"tailrocks-perf-engineer (agent)" |
| Run gates and report PASS/FAIL | @"tailrocks-verifier (agent)" |
| Review a diff, branch or PR for correctness | @"tailrocks-code-reviewer (agent)" |
| Sandbox, unsafe/FFI, parsers, archives, credentials, CI trust, new dependencies | @"tailrocks-security-reviewer (agent)" (in addition to @"tailrocks-code-reviewer (agent)") |
| Approve a test plan before writing tests; red/green proof before keeping a test | @"tailrocks-test-reviewer (agent)" |
| WCAG 2.2 audit of built UI | @"tailrocks-a11y-auditor (agent)" |
| Drive the built app and compare with drafts/specs | @"tailrocks-qa-engineer (agent)" |
| Any `.mdx` or docs-site page; READMEs; decision records | @"tailrocks-docs-writer (agent)" |

Do it yourself, without a subagent, for a one-file change with an obvious check, a question you can answer from two or three reads, or work that needs frequent back-and-forth with the user.

## Model and effort

Sonnet is the default. Haiku does cheap read-and-run work. Opus 5.5 is the orchestrator for hard work and an escalation step. It is never a roster default; it appears only as the one Opus seat in pstack review panels (see the model sheet). Fable 5.1 is the rarest last resort.

Evidence (checked 2026-11-01):
- SWE-bench Pro subset, Anthropic's cost-optimization page: Sonnet 5.5 `high` solves 87.6% at $0.11 per solved task, the cheapest coding setting measured. Opus 5.5 `medium` solves 92.8% at $0.22. Fable 5.1 `high` solves 92.3% at $1.19.
- Same page: Sonnet at `medium` loses about 7 points on SWE-bench Pro and 15 on Anthropic's internal agentic benchmark, so coding and review agents run Sonnet `high`, never `medium`.
- Same page: Haiku 5.5 subagents cut cost about 32% for about −4 points. The effort docs say Haiku `low` skips searches and stops early, and recommend `high` for longer agent tasks and strict instruction following.
- Artificial Analysis (independent, at `max`): Sonnet spent so many more tokens that its total ($7,981) approached Opus's ($8,910). So escalate to Opus instead of raising Sonnet past `high`.
- Haiku 5.5 costs 5× more per token above 100K prompt tokens, so Haiku agents compact at 100K (`autoCompactWindow`).

Defaults set in agent frontmatter:

| Model + effort | Agents |
|---|---|
| Haiku `medium` | @"tailrocks-locator (agent)" |
| Haiku `high` | @"tailrocks-verifier (agent)", @"tailrocks-mechanic (agent)" |
| Sonnet `medium` | @"tailrocks-docs-researcher (agent)", @"tailrocks-docs-writer (agent)" |
| Sonnet `high` | every other agent |

Fable is never a frontmatter default; it is reached only per call (Escalation, step 3).

Session defaults in `~/.claude/settings.json`:
- Main model and effort are whatever `/model` and `/effort` saved (`model` and per-model `modelSettings` in `settings.json`). A top-level `effortLevel` in the user settings file does not apply to Opus 5.5 or later models; one in project, local or managed settings applies to every model (Claude Code model-config docs, checked 2026-11-03).
- `CLAUDE_CODE_SUBAGENT_MODEL` is Sonnet, so `general-purpose` and plugin agents without a model run on Sonnet instead of inheriting an Opus main session. Forks still use the main model.

### When the main session should be Opus

Switch the main session to Opus (`/model opus`, or `claude --model opus`) only to orchestrate work that Sonnet cannot manage well:
- multi-phase, multi-repo programs coordinating many subagents;
- architecture with many interacting constraints or recorded decisions to reconcile;
- a bug or design problem Sonnet has already failed twice.
Subagents keep their Sonnet/Haiku defaults under an Opus orchestrator. `opusplan` (Opus while planning, Sonnet while executing) is a good middle ground for a single hard feature. Return to Sonnet when the hard part is done.

### Per-call overrides (Agent tool `model` and `effort`)

A per-call `model` keeps the agent's frontmatter effort, so always pass `effort` with it.
- `model: haiku, effort: high` to @"tailrocks-frontend-engineer (agent)" or @"tailrocks-systems-engineer (agent)" for trivial transforms, generated tables and asset pipelines.
- `model: sonnet, effort: high` to @"tailrocks-mechanic (agent)" or @"tailrocks-verifier (agent)" when the task needs judgment beyond running a pattern or a command.
- `model: opus, effort: high` only on escalation (below), with a reason.
- `xhigh` and `max` only when the user asks.

## Escalation

Anthropic's cost-optimization guidance (checked 2026-11-03): running everything at a low setting and re-running only the failures at a higher one kept the pass rate of running everything at the higher setting, for a little over half the cost. Follow it:

1. When a result fails its gate or review, run the same agent again with the failure evidence, at the same settings. An agent that stopped because the brief was ambiguous or needed a judgment call has not failed a round: fix the brief, or send it to the roster agent for that work at its default model.
2. After two failed rounds on one finding, move one step up: Haiku → `model: sonnet, effort: high`; Sonnet → `model: opus, effort: high`.
3. When Opus has also failed, or two Opus-level designs conflict, make the phase's single Fable call: @"tailrocks-architect (agent)" in decision mode with `model: fable, effort: high`, and a written reason in the brief. Otherwise stop and ask the user.

## Delegation rules

- **Pipeline for a feature or unit:** @"tailrocks-locator (agent)"/@"tailrocks-docs-researcher (agent)" → @"tailrocks-architect (agent)" → @"tailrocks-test-reviewer (agent)" (plan) → implementer(s) → @"tailrocks-verifier (agent)" → @"tailrocks-test-reviewer (agent)" (red/green, for new tests) → @"tailrocks-code-reviewer (agent)" (+ @"tailrocks-security-reviewer (agent)" when its triggers apply) → fix round → @"tailrocks-verifier (agent)" → @"tailrocks-docs-writer (agent)" for affected docs.
- **Pipeline for a UI unit:** @"tailrocks-locator (agent)" → @"tailrocks-design-director (agent)" spec → @"tailrocks-ux-specialist (agent)" critique → @"tailrocks-frontend-engineer (agent)"(s) → @"tailrocks-verifier (agent)" → @"tailrocks-design-director (agent)" review ∥ @"tailrocks-a11y-auditor (agent)" ∥ @"tailrocks-qa-engineer (agent)" → @"tailrocks-code-reviewer (agent)" → fix round → @"tailrocks-verifier (agent)" → @"tailrocks-docs-writer (agent)".
- Diffs touching sandbox isolation, credentials or CI trust also get a cross-model check from the main session: use pstack `interrogate` when available, or an independent cross-model review otherwise. If neither can run, mark the check BLOCKED.
- One design critique round per unit. One @"tailrocks-code-reviewer (agent)" per batch, on `<batch start>..HEAD`. After the fix round, check the fixes against the findings yourself; do not re-review.
- At most one Fable call per phase, and only on the escalation path or when the repo's plan names a Fable call for that phase.
- Only @"tailrocks-docs-writer (agent)" edits `.mdx` files.
- Never write a test without a plan that @"tailrocks-test-reviewer (agent)" approved. Never keep a test that @"tailrocks-test-reviewer (agent)" has not seen go red, then green. It needs the fix committed: it proves red/green in a temporary worktree of that SHA.
- Run independent subagents in parallel, in one message, with at most 20 at once. Parallel writers must never share a file: give each one its own worktree or slot, or a disjoint set of files.
- Brief each agent by the brief contract above. An agent knows only its brief, the repo's files and its project note.
- Pass a subagent's diff, never its conclusion, to a reviewer. Treat agent reports as claims, and verify the key facts before acting on them.
- If the user denies a tool call or subagent, do not retry it unchanged.
- Subagents do not commit or push unless their brief says so. The main session commits and pushes verified work.
- `settings.json` denies force-push, `git reset --hard`, `git clean -f`, `git stash drop/clear` and `--no-verify` in every mode. When one of these is genuinely needed, ask the user to run it.

## pstack

When pstack (`pstack@pstack-claude`) is installed and enabled, use it as the main session's working style. Its SessionStart hook routes multi-file, design and unknown-cause work through `/poteto-mode` when the hook is enabled. These rules apply to the roster above; pstack-specific instructions apply when pstack is available:

- **Delegates**: when a poteto-mode playbook step says to spawn `pstack:poteto-agent`, spawn the matching roster agent from the routing table instead. Use `poteto-agent` only when no roster agent fits. Each roster agent lists its role's pstack principles for preload when available, plus `repo-context`, which folds in pstack's prove-it-works, deslop and unslop rules. When pstack is available, agents can invoke its other skills by name.
- **Routed skills** (`how`, `why`, `swarm`, `arena`, `architect`, `interrogate`, `reflect`, `thermos`) set their own subagent types; leave them, and their models come from the pstack model sheet below. These skills fan out, so run them only from the main session.
- **Steering**: state the goal and constraints, and name a principle only to override a default (for example "apply attack the premise"). Ask agents which decision each named principle changed.
- **Fresh by default**: give a fix round, follow-up or retry to a fresh agent, with consolidated scope (the original brief, every later directive, the prior report and branch). Resume an agent only when the work needs state that is costly to move: uncommitted changes in its checkout, or a running process. Stop an abandoned agent and confirm it stopped; check `git status` against its claims.
- **Own the result**: review each agent's diff and write your own summary; never pass a report through. A required independent review that cannot run stays `BLOCKED: independent review`; your own review, tests or CI do not substitute.
- **Second opinions** come from the same brief on a different model: `arena` for design, `interrogate` for adversarial review. Use them for decisions that are expensive to reverse.
- **Model sheet**: pstack's own defaults put most roles on Opus. `~/.claude/pstack-models.md` overrides them to match the model policy above: Sonnet for work, Haiku for read-and-collect roles, one Opus seat in review panels for diversity, no Fable. It is loaded here:

@~/.claude/pstack-models.md

@RTK.md
