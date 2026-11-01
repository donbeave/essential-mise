# Engineering policy

- Judge work by correctness, consistency, and project fit. Do not defer a known-wrong state because of cost, effort, or claims that it is marginal.
- Before fixing a bug, identify why the architecture allowed it and whether the same structure permits related bugs. Prefer fixes that remove the enabling condition.
- When uncertain, inspect, test, and measure before acting.
- Decompose independent work and use parallel research, implementation, review, and verification when the host supports them.
- When committing, verify the effective Git committer identity is Alexey Zhokhov <alexey@zhokhov.com>. Use git commit -s so Git adds exactly one Signed-off-by trailer; never add or duplicate it manually.

# Subagents

Definitions live in `~/.claude/agents/`. This section is for the main session. Roster agents skip this file (`omitClaudeMd: true`) and get what they need from `repo-context`. No subagent can spawn another (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=1`), and the built-in Explore and Plan agents are denied: use `locator` and `architect`.

The agents are generic. None of them knows a specific project. Each one describes only its capabilities, and the brief you send decides where it works. Every agent except `locator` preloads the `repo-context` skill (`~/.claude/skills/repo-context/SKILL.md`), which makes it:
1. resolve the repo and scope from the brief, and stop with `Needs from caller:` if they are missing;
2. read that repo's `AGENTS.md` and `CLAUDE.md` explicitly, plus the matching note in `~/.claude/project-notes/` (indexed by git remote);
3. discover gate commands from the repo itself;
4. follow the shared scope, verification and commit rules, and report under fixed headings.

Project-specific knowledge belongs in the repo's own `AGENTS.md`, or in a project note when it cannot go in the repo. It never goes in an agent file. To onboard a new repo, add a note and a row to `~/.claude/project-notes/README.md`.

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
| Find files, symbols, call sites, config (instead of built-in Explore) | `locator` |
| Any library/tool/version/API question; choosing or bumping a dependency | `docs-researcher` |
| Design before non-trivial work: new modules/crates/packages, contracts, cross-repo changes, migrations | `architect` |
| One highest-stakes decision per phase: after `architect` and an Opus escalation failed, or when the repo's plan names a Fable call | `fable-architect` |
| Rust backend/systems/library implementation | `rust-engineer` |
| Rust CLI (clap) and TUI (ratatui/crossterm, PTY) | `rust-cli-tui-engineer` |
| Agent sandboxes: containers, isolation, mounts, credentials, capsule, role images | `agent-sandbox-engineer` |
| OpenShell API/behavior, jackin openshell-v2 tasks, blocker reports | `openshell-specialist` |
| CI/CD, GitHub Actions, Velnor generator/runners, mise, hk, releases, images | `cicd-architect` |
| TypeScript/React implementation, design-system components | `frontend-engineer` |
| TanStack Start/Router/Query routing and data-loading design or upgrades | `tanstack-start-specialist` |
| Map drafts to design-system components; visual review from screenshots | `design-director` |
| Flows, route/deep-link map, state matrix, spec critique | `ux-specialist` |
| Fully specified mechanical edits: lint fixes, codemods, headers, chosen pin bumps | `mechanic` |
| Bug or CI failure with unknown cause, flaky test, regression | `debugger` |
| Performance budgets, stress scenarios, profiling | `perf-engineer` |
| Run gates and report PASS/FAIL | `verifier` |
| Review a diff, branch or PR for correctness | `code-reviewer` |
| Sandbox, unsafe/FFI, parsers, archives, credentials, CI trust, new dependencies | `security-reviewer` (in addition to `code-reviewer`) |
| Approve a test plan before writing tests; red/green proof before keeping a test | `test-reviewer` |
| WCAG 2.2 audit of built UI | `a11y-auditor` |
| Drive the built app and compare with drafts/specs | `qa-engineer` |
| Any `.mdx` or docs-site page; READMEs; decision records | `docs-writer` |

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
| Haiku `medium` | `locator` |
| Haiku `high` | `verifier`, `mechanic` |
| Sonnet `medium` | `docs-researcher`, `docs-writer` |
| Sonnet `high` | every other agent: architects, implementers, reviewers, specialists, design, QA |
| Fable `high` | `fable-architect` only |

Session defaults in `~/.claude/settings.json`:
- Main model Sonnet 5.5. Effort is saved per model under `modelSettings` (Sonnet `high`, Opus `medium`); a top-level `effortLevel` is ignored by Opus 5.5 and newer models.
- `CLAUDE_CODE_SUBAGENT_MODEL` is Sonnet, so `general-purpose` and plugin agents without a model run on Sonnet instead of inheriting an Opus main session. Forks still use the main model.

### When the main session should be Opus

Switch the main session to Opus (`/model opus`, or `claude --model opus`) only to orchestrate work that Sonnet cannot manage well:
- multi-phase, multi-repo programs coordinating many subagents (the ChainArgos redesign `goal.md`, the jackin OpenShell v2 rewrite);
- architecture with many interacting constraints or recorded decisions to reconcile;
- a bug or design problem Sonnet has already failed twice.
Subagents keep their Sonnet/Haiku defaults under an Opus orchestrator. `opusplan` (Opus while planning, Sonnet while executing) is a good middle ground for a single hard feature. Return to Sonnet when the hard part is done.

### Per-call overrides (Agent tool `model` and `effort`)

A per-call `model` keeps the agent's frontmatter effort, so always pass `effort` with it.
- `model: haiku, effort: high` to `frontend-engineer` or `rust-engineer` for trivial transforms, generated tables and asset pipelines.
- `model: sonnet, effort: high` to `mechanic` or `verifier` when the task needs judgment beyond running a pattern or a command.
- `model: opus, effort: high` only on escalation (below), with a reason.
- `xhigh` and `max` only when the user asks.

## Escalation

Anthropic measured a policy of running cheap and re-running only failures at `high` on a stronger setting: about 97% solved, about 40% cheaper than running everything on the expensive setting. Follow it:

1. When a result fails its gate or review, run the same agent again with the failure evidence, at the same settings. An agent that stopped because the brief was ambiguous or needed a judgment call has not failed a round: fix the brief, or send it straight to the Sonnet agent for that work.
2. After two failed rounds on one finding, move one step up: Haiku → `model: sonnet, effort: high`; Sonnet → `model: opus, effort: high`.
3. When Opus has also failed, or two Opus-level designs conflict, use the phase's single `fable-architect` call, with a written reason. Otherwise stop and ask the user.

## Delegation rules

- **Pipeline for a feature or unit:** `locator`/`docs-researcher` → `architect` → `test-reviewer` (plan) → implementer(s) → `verifier` → `test-reviewer` (red/green, for new tests) → `code-reviewer` (+ `security-reviewer` when its triggers apply) → fix round → `verifier` → `docs-writer` for affected docs.
- **Pipeline for a UI unit:** `locator` → `design-director` spec → `ux-specialist` critique → `frontend-engineer`(s) → `verifier` → `design-director` review ∥ `a11y-auditor` ∥ `qa-engineer` → `code-reviewer` → fix round → `verifier` → `docs-writer`.
- Diffs touching sandbox isolation, credentials or CI trust also get a cross-model check with pstack `interrogate`, run from the main session.
- One design critique round per unit. One `code-reviewer` per batch, on `<batch start>..HEAD`. After the fix round, check the fixes against the findings yourself; do not re-review.
- At most one `fable-architect` call per phase, and only on the escalation path or when the repo's plan names a Fable call for that phase.
- Only `docs-writer` edits `.mdx` files.
- Never write a test without a plan that `test-reviewer` approved. Never keep a test that `test-reviewer` has not seen go red, then green. `test-reviewer` needs the fix committed: it proves red/green in a temporary worktree of that SHA.
- Run independent subagents in parallel, in one message, with at most 20 at once. Parallel writers must never share a file: give each one its own worktree or slot, or a disjoint set of files.
- Brief each agent by the brief contract above. An agent knows only its brief, the repo's files and its project note.
- Pass a subagent's diff, never its conclusion, to a reviewer. Treat agent reports as claims, and verify the key facts before acting on them.
- If the user denies a tool call or subagent, do not retry it unchanged.
- Subagents do not commit or push unless their brief says so. The main session commits and pushes verified work.
- `settings.json` denies force-push, `git reset --hard`, `git clean -f`, `git stash drop/clear` and `--no-verify` in every mode. When one of these is genuinely needed, ask the user to run it.

## pstack

pstack (`pstack@pstack-claude`) stays on as the main session's working style: its SessionStart hook routes multi-file, design and unknown-cause work through `/poteto-mode`. These rules adapt it to the roster above:

- **Delegates**: when a poteto-mode playbook step says to spawn `pstack:poteto-agent`, spawn the matching roster agent from the routing table instead. Use `poteto-agent` only when no roster agent fits. Roster agents preload the pstack principles for their role (`skills:` in each file) plus `repo-context`, which folds in pstack's prove-it-works, deslop and unslop rules; they can invoke other pstack skills by name.
- **Routed skills** (`how`, `why`, `swarm`, `arena`, `architect`, `interrogate`, `reflect`, `thermos`) set their own subagent types; leave them, and their models come from the pstack model sheet below. These skills fan out, so run them only from the main session.
- **Steering**: state the goal and constraints, and name a principle only to override a default (for example "apply attack the premise"). Ask agents which decision each named principle changed.
- **Fresh by default**: give a fix round, follow-up or retry to a fresh agent, with consolidated scope (the original brief, every later directive, the prior report and branch). Resume an agent only when the work needs state that is costly to move: uncommitted changes in its checkout, or a running process. Stop an abandoned agent and confirm it stopped; check `git status` against its claims.
- **Own the result**: review each agent's diff and write your own summary; never pass a report through. A required independent review that cannot run stays `BLOCKED: independent review`; your own review, tests or CI do not substitute.
- **Second opinions** come from the same brief on a different model: `arena` for design, `interrogate` for adversarial review. Use them for decisions that are expensive to reverse.
- **Model sheet**: pstack's own defaults put most roles on Opus. `~/.claude/pstack-models.md` overrides them to match the model policy above: Sonnet for work, Haiku for read-and-collect roles, one Opus seat in review panels for diversity, no Fable. It is loaded here:

@~/.claude/pstack-models.md

## Legacy `ca-*` names

`platform/redesign/goal.md` names `ca-*` agents. Map each name to its current agent and keep the overrides that `goal.md` gives:

| `ca-*` name | Agent |
|---|---|
| `ca-explorer` | `locator` |
| `ca-docs-researcher` | `docs-researcher` |
| `ca-fable-architect` | `fable-architect` |
| `ca-design-director` | `design-director` |
| `ca-ux-specialist` | `ux-specialist` |
| `ca-frontend-engineer` | `frontend-engineer` |
| `ca-verifier` | `verifier` |
| `ca-a11y-auditor` | `a11y-auditor` |
| `ca-qa-engineer` | `qa-engineer` |
| `ca-code-reviewer` | `code-reviewer` |
| `ca-perf-engineer` | `perf-engineer` |
| `ca-docs-writer` | `docs-writer` |
