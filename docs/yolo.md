# Yolo aliases

`ai-cli` adds zsh commands for terminal coding agents. Arguments are passed through, for example `grok-yolo -c`.

The commands are in one `~/.zshrc` block, `mise:yolo`. Fish does not get them. Apply `ai-cli`, then open a fresh zsh session:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-cli dot apply
/bin/zsh -lic 'alias grok-yolo'
```

| Alias | Expands to | What that flag does |
|---|---|---|
| `claude-yolo` | Claude bypass mode with the extra dangerous removal bypass | Loads user settings without permission deny and ask rules, excludes project and local settings, and retains the [status line](claude-statusline.md). Managed settings and plugins can still restrict actions |
| `codex-yolo` | `codex --yolo` | Alias of `--dangerously-bypass-approvals-and-sandbox`. Skips approvals and turns the sandbox off (`danger-full-access`) |
| `cursor-yolo` | `cursor-agent --yolo` | Alias of `--force`. Allows commands unless a rule denies them. The command is `cursor-agent`; on this setup `agent` is Grok |
| `agy-yolo` | `agy --dangerously-skip-permissions` | Antigravity CLI. Auto-approves tool permission requests |
| `kimi-yolo` | `kimi --auto` | Never Ask: actions run without prompts, and Plan mode still waits. Kimi's own `--yolo` is Ask When Needed and still prompts for risky actions, so the alias does not use it |
| `opencode-yolo` | `opencode --auto` | Auto-approves permissions that are not explicitly denied |
| `grok-yolo` | `grok --yolo` | Alias of `--always-approve` and of `--permission-mode bypassPermissions`. Auto-approves tool executions |
| `amp-yolo` | `amp --dangerously-allow-all` | Disables command confirmation prompts |
| `omp-yolo` | `omp --approval-mode=yolo` | Sets this session's approval mode to `yolo` |
| `muse-yolo` | `muse --yolo` | Muse Code. Disables approval and sandbox for the workspace run |

## Provider matrix (`<agent>-<provider>[-<account>]`)

One alias per account per agent; every entry also has a `-yolo` twin.

| Alias | Account | Isolation |
|---|---|---|
| `claude-anthropic`, `claude-anthropic2` | Anthropic, alexey@chainargos.com / alexey2@chainargos.com | `~/.claude-chainargos`, `~/.claude-chainargos2` (shared with the legacy `claude-chainargos` / `claude-chainargos2` names) |
| `claude-kimi`, `claude-zai`, `claude-minimax`, `claude-mimo` | Kimi / Z.AI / MiniMax / MiMo, alexey@chainargos.com | `~/.claude-kimi`, `~/.claude-zai`, `~/.claude-minimax`, `~/.claude-mimo` |
| `codex-openai` | OpenAI, alexey@chainargos.com | `~/.codex` (explicit twin of bare `codex`) |
| `codex-openai2` | OpenAI, alexey2@chainargos.com | `~/.codex-openai2` |
| `codex-openai-zhokhov` | OpenAI, alexey@zhokhov.com | `~/.codex-openai-zhokhov` |
| `codex-kimi`, `codex-zai`, `codex-minimax`, `codex-mimo` | Kimi / Z.AI / MiniMax / MiMo, alexey@chainargos.com | `~/.codex-kimi`, `~/.codex-zai`, `~/.codex-minimax`, `~/.codex-mimo` |
| `omp-kimi`, `omp-zai`, `omp-minimax`, `omp-mimo` | Kimi / Z.AI / MiniMax / MiMo, alexey@chainargos.com | `omp --profile <name>` (created on first use) |
| `kimi-chainargos` | Kimi, alexey@chainargos.com | Bare `kimi` |
| `grok-chainargos` | Grok, alexey@chainargos.com | Bare `grok` |
| `opencode-chainargos` | OpenCode Go, alexey@chainargos.com | Bare `opencode` |
| `muse-chainargos` | Muse, alexey@chainargos.com | Bare `muse` |
| `agy-gmail` | Gemini/Antigravity, donbeave@gmail.com | Bare `agy` |

MiniMax Code and MiMo Code standalone CLIs are not installed, so MiniMax/MiMo are
covered only through the `claude-*`, `codex-*`, and `omp-*` provider aliases.

`codex --yolo` is the wide mode: it removes the sandbox as well as the prompts. `--not-so-yolo` is a different flag (`--approve-for-me`) and is not what `codex-yolo` runs.

`claude-yolo` allows destructive removals without the critical-path prompt. Use it in an isolated environment. It reads settings from `CLAUDE_CONFIG_DIR` when set, or `~/.claude` otherwise. Plain `claude` retains its normal settings and permission rules.

`claude-scentbird-yolo` and `claude-scentbird-ai-yolo` use the same YOLO function with their respective account directories. All arguments are forwarded to Claude.

`claude-chainargos` and `claude-chainargos2` use `~/.claude-chainargos` and `~/.claude-chainargos2`, respectively. Their `-yolo` variants use the same directories and the shared YOLO function. New accounts start with empty settings until Claude creates a settings file.
