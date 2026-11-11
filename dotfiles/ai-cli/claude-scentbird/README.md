# Scentbird Claude Code profiles

Subagents and user instructions for the two Scentbird Claude Code profiles, `~/.claude-scentbird` and `~/.claude-scentbird-ai` (the `claude-scentbird*` wrappers in `config.ai-cli.toml`). The `scentbird` module (`config.scentbird.toml`) deploys them; select it with `-E scentbird`.

This set is separate from the `tailrocks-*` agents in `dotfiles/ai-cli/claude/agents`, which deploy only to `~/.claude`. Do not copy agents between the two directories or reference one set from the other.

## Naming

Every agent is named `scentbird-<role>`. The `name:` field and the file name must match. Give a new agent the same prefix. The prefix keeps these agents from colliding with plugin agents, with a project's `.claude/agents/` (which takes precedence over user agents), and with the `tailrocks-*` set.

## Public and private parts

This repository is public, so the agents here are generic: role, method, model tier, and tool limits. Everything specific to the employer's codebases (repository map, toolchain facts, conventions, and one `### scentbird-<agent>` section per agent) lives in `scentbird-context.md` in the private `donbeave/essential-mise-scentbird` repository. The module clones it to `~/.local/share/essential-mise-scentbird`, copies the file into both profiles, and `CLAUDE.md` imports it with `@scentbird-context.md`. Never add company-internal names, paths, or processes to the files here.

## Deployment

| Source | Target in both profiles | Mode |
|---|---|---|
| `CLAUDE.md` | `CLAUDE.md` | copy |
| `agents/` | `agents/` | copy |
| private `scentbird-context.md` | `scentbird-context.md` | copy |
| `settings.json` | `settings.json` | merge: owns only `env.CLAUDE_CODE_SUBAGENT_MODEL` |

Copy mode never prunes. When you rename or delete an agent, remove the old file from both `agents/` targets by hand.

## Model tiers

Most agents run on Sonnet 5.5 or Haiku 5.5. Four run on Opus 5.5 and are reached only through the triggers in `CLAUDE.md`; `scentbird-java-architect` is the only Opus xhigh agent. Built-in agents without a pinned model run on Sonnet through `CLAUDE_CODE_SUBAGENT_MODEL`.
