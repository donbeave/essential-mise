# Scentbird profile

@scentbird-context.md

The import above loads the private Scentbird context (repository map, toolchain facts, conventions, and one section per subagent). It is deployed from a private repository by the `scentbird` mise module. If it is missing, rely on each repository's own `CLAUDE.md`/`AGENTS.md` and say that the Scentbird context is not installed.

## Safety (all agents)

Never act on a shared or live environment: no `kubectl apply/delete/rollout`, Argo sync or rollback, pipeline or manual-job triggers, artifact publishing, or writes to non-local databases. Never commit or push unless the user explicitly asked. Propose the command instead.

# Subagent routing

Every subagent in this profile is named `scentbird-*`. Give any new one the same prefix. These agents are separate from the `tailrocks-*` set in `~/.claude/agents`: do not mix the two sets or reference one from the other.

Delegate to the cheapest agent that can do the job. Default to Sonnet and Haiku agents; Opus agents are the exception.

- **Haiku first** for lookups, running commands, and reading output: `scentbird-code-locator`, `scentbird-log-triager`, `scentbird-test-runner`, `scentbird-pr-writer`, `scentbird-dependency-auditor`, `scentbird-conventions-checker`.
- **Sonnet** for all implementation, design within a service, review, debugging, and operations: `scentbird-java-developer`, `scentbird-senior-java-engineer`, `scentbird-rust-engineer`, `scentbird-root-cause-investigator`, `scentbird-performance-engineer`, `scentbird-graalvm-native-engineer`, `scentbird-code-reviewer`, `scentbird-test-engineer`, `scentbird-migration-engineer`, `scentbird-gradle-build-engineer`, `scentbird-library-release-engineer`, `scentbird-database-engineer`, `scentbird-ci-cd-engineer`, `scentbird-sre-engineer`, `scentbird-docs-writer`, `scentbird-java-consultant`, `scentbird-build-fixer`.
- **Opus only by trigger:**
  - `scentbird-escalation-advisor` (low): a Sonnet or Haiku agent failed twice, or two approaches conflict.
  - `scentbird-api-compat-guardian` (medium): a diff *removes, renames, or changes the type or cardinality* of a proto, GraphQL, or event field, changes a public shared-library or Gradle-plugin API, or contains a destructive migration. Additive changes stay with `scentbird-code-reviewer`.
  - `scentbird-security-auditor` (high): auth, payment, or PII changes, or the user asks for a security audit.
  - `scentbird-java-architect` (xhigh): **very rare.** Only when the user explicitly asks for architecture work, or for a new cross-service boundary or data split. Never for routine features, specs, language ports, bugs, or reviews.
- Do not pass `model: "opus"` on an Agent call to upgrade a Sonnet or Haiku agent; escalate through `scentbird-escalation-advisor` instead.
- Built-in agents without a pinned model (general-purpose, Plan) run on Sonnet through `CLAUDE_CODE_SUBAGENT_MODEL`.
