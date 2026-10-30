# Claude Code status line

`bin/claude-statusline.sh` is the unchanged status-line script, installed as an executable. It requires Bash, `jq`, and macOS `stat -f`.

This is a narrowly user-approved status-line exception, not an implementation of revision 7's old excluded status-line graph. The new wrapper uses `refreshInterval = 5`, so Claude re-runs the command every five seconds and can remove a subagent row after its ten-minute idle threshold even when the main session is idle. The absolute token figure is input-token usage (input plus cache creation/read), which matches Claude Code's documented `used_percentage` formula; it intentionally does not include output tokens and is not presented as an output-inclusive total.

## zsh wrapper

The `ai-cli` module adds this native block to `~/.zshrc`:

```zsh
claude() {
  local statusline="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/claude-statusline.sh"
  local statusline_command settings
  printf -v statusline_command '%q' "$statusline"
  settings="$(/opt/homebrew/bin/jq -cn --arg command "$statusline_command" '{statusLine:{type:"command",command:$command,padding:0,refreshInterval:5}}')" || return
  BASH_COMPAT=3.2 command claude --settings "$settings" "$@"
}
```

Every argument is forwarded through `command claude`, so existing flags and subcommands are not replaced. The absolute script path follows the adopted config directory, including custom `MISE_CONFIG_DIR` or `XDG_CONFIG_HOME` values. Zsh `%q` shell-quotes the executable path and `jq --arg` JSON-encodes the complete command, so whitespace and shell/JSON metacharacters are preserved. `BASH_COMPAT=3.2` is required because Homebrew Bash 5.3 otherwise expands the script's replacement tilde to the account's real home instead of leaving `~`; the script itself was not changed to work around that behavior.

## Package owner

Claude Code is installed through real Homebrew with `/opt/homebrew/bin/brew install --cask claude-code@latest`. The verified version is `2.1.312 (Claude Code)`. The older `claude-code` cask's missing `bin/claude` target is documented in [Homebrew exceptions](homebrew-exceptions.md).

## Verification

The sample payload exercised `workspace.current_dir`, `model.display_name`, `effort.level`, `context_window.used_percentage`, `context_window.context_window_size`, `context_window.current_usage`, and `transcript_path`. Synthetic subagents covered running, finished, interrupted, and idle states. Sanitized output was exactly:

```text
~/Projects/ChainArgos/redesign-review │ Opus 5.5 medium │ 35% (345k/1.0M)
↳ explorer │ sonnet low │ 91k
```

The test passed with one running subagent, finished/interrupted/idle agents hidden, and green/yellow/red context thresholds at 35%, 50%, and 80%. Replay it inside a registered guest and collect `/tmp/essential-mise-evidence/claude-statusline` before restarting that guest:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-claude-statusline.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-claude-statusline.py"
```

Fresh login zsh checks from both `/tmp` and `/Users/admin` returned the wrapper above with the adopted absolute script path. An interactive launch of Claude Code 2.1.312 included that exact `--settings` JSON in its process title. Its debug log also reported `model=claude-sonnet-5-5` from the synthetic project settings, confirming that `--settings` merges with, rather than discards, project settings. No real account was used; post-login rendering was not claimed because the test stopped at the login screen.

The existing `claude-code@latest` cask was reused safely, and repeated preview/apply runs converged with no second owner. The original conflicting `claude-code` declaration was removed.
