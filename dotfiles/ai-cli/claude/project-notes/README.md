# Project notes

Facts about specific repositories that generic subagents need but should not carry in their prompts. Before working in a repo, an agent reads only the note that matches its remote, plus the repo's own AGENTS.md/CLAUDE.md. The repo's own files always win over these notes.

| Remote (`git remote get-url origin`) | Note |
|---|---|
| `jackin-project/jackin`, `jackin-project/jackin-sentinel`, `jackin-project/jackin-agent-smith`, `jackin-project/jackin-the-architect`, `tailrocks/terminal-components-baseline` | `jackin.md` |
| `tailrocks/velnor-new` | `velnor-new.md` |
| `ChainArgos/platform`, `ChainArgos/ui-design-system`, `ChainArgos/documentation-portal` | `chainargos-frontend.md` |

To add a project, write `<name>.md` with these sections and add a row above: What it is · Stack · Gates · Rules that change how agents work · Ownership and generated files · Where decisions live.
