# Draft pull requests by default

`development` makes every new GitHub pull request a draft. This applies to people and to coding agents that forget `--draft`. The GitHub CLI has no config key for this, and a `gh` alias cannot replace the built-in `pr create`. So the setup has two parts.

| Part | Where | What it does |
|---|---|---|
| `gh prd` | gh alias, set by the `post-packages` hook | Runs `gh pr create --draft` |
| `gh` zsh function | `~/.zshenv` block `mise:gh-draft` | Adds `--draft` to `gh pr create` and `gh pr new` |

The function is in `~/.zshenv` because zsh reads that file for every shell, interactive or not, so agent shells get it too. Bash, fish and direct calls to `/opt/homebrew/bin/gh` do not.

The function leaves the command as given when it already has `-d`, `--draft`, `--draft=<value>`, `-w` or `--web`. The CLI does not open a draft from `--web`. Set `GH_PR_CREATE_DRAFT=0` to open a ready pull request:

```bash
GH_PR_CREATE_DRAFT=0 gh pr create --fill
```

Use `gh pr ready <number>` when the pull request is ready for review.

Apply `development`, then open a fresh shell and check both parts:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E development dot apply
/opt/homebrew/bin/gh alias list | grep '^prd:'
/bin/zsh -c 'whence -w gh'
```

`whence -w gh` prints `gh: function`.
