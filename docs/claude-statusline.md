# Claude Code status line

`ai-cli` installs `bin/claude-statusline.sh` as an executable and adds an alias-safe zsh function. The script requires Bash, `jq`, and macOS `stat -f`.

Claude reruns the command every five seconds. The display includes the current directory, model and effort, input-token context usage, and running subagents. Finished, interrupted, and idle subagents are hidden. The absolute token figure includes input plus cache creation/read tokens; output tokens are intentionally excluded.

## zsh wrapper

```zsh
function claude {
  local statusline="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/claude-statusline.sh"
  local statusline_command settings
  printf -v statusline_command '%q' "$statusline"
  settings="$(/opt/homebrew/bin/jq -cn --arg command "$statusline_command" '{statusLine:{type:"command",command:$command,padding:0,refreshInterval:5}}')" || return
  BASH_COMPAT=3.2 command claude --settings "$settings" "$@"
}
```

The function preserves an existing alias and forwards alias-supplied and explicit arguments. Zsh `%q` shell-quotes the adopted script path and `jq --arg` JSON-encodes it, so custom XDG paths containing whitespace or shell/JSON metacharacters remain intact. `BASH_COMPAT=3.2` preserves the script's literal `~` display with Homebrew Bash 5.3.

Claude Code itself is installed through the real-Homebrew `claude-code@latest` exception; see [Homebrew exceptions](homebrew-exceptions.md). Package installation and synthetic rendering do not establish authenticated interactive behavior. Complete account setup and verify the live status line manually.

A sanitized sample is:

```text
~/Projects/ChainArgos/redesign-review │ Opus 5.5 medium │ 35% (345k/1.0M)
↳ explorer │ sonnet low │ 91k
```

Reusable synthetic rendering and wrapper checks are documented in [Disposable Tart/Cua verification](../scripts/vm/README.md). Guest output is collected outside the repository.
