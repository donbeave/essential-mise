# Claude Code status line

`ai-cli` installs an executable status-line script and an alias-safe zsh wrapper. Claude reruns the script every five seconds to show the current directory, model and effort, input-token context usage, and running subagents. Finished, interrupted, and idle subagents are hidden. Output tokens are not included in the absolute token figure.

## Enable it

Apply `ai-cli`, then open a fresh zsh session:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-cli bootstrap
/bin/zsh -lic 'typeset -f claude'
```

The wrapper preserves an existing alias and forwards alias-supplied and explicit arguments. It locates the script through `MISE_CONFIG_DIR` or `XDG_CONFIG_HOME`, including paths containing whitespace and shell/JSON metacharacters. `BASH_COMPAT=3.2` is set for Homebrew Bash compatibility.

Claude Code is installed through the real-Homebrew `claude-code@latest` exception; see [Homebrew exceptions](homebrew-exceptions.md).

## Bypass it temporarily

Prefix a launch with `command` to bypass the zsh function and its status-line `--settings` without changing any files:

```bash
command claude
```

This affects only that invocation. The wrapper remains active for later launches.

## Display and troubleshooting

A sanitized sample is:

```text
~/Projects/ChainArgos/redesign-review │ Opus 5.5 medium │ 35% (345k/1.0M)
↳ explorer │ sonnet low │ 91k
```

If the status line is missing:

1. Confirm you are using zsh; the wrapper is not installed for Fish. Run `typeset -f claude` to inspect the active function.
2. Check the script and its dependencies with `test -x "$config_dir/bin/claude-statusline.sh"`, `command -v jq`, and `command -v bash`.
3. Reapply `ai-cli` if the function or script is missing. If you intentionally used `command claude`, launch plain `claude` instead.
4. Complete Claude authentication and check whether the problem persists in a new zsh session. Package installation and synthetic rendering do not establish authenticated interactive display.

Reusable synthetic rendering and wrapper checks are documented in [Disposable Tart/Cua verification](../scripts/vm/README.md). Guest output is collected outside the repository.
