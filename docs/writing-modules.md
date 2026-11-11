# Writing modules

A module is one file in your config repository named `config.<name>.toml`. It declares what mise bootstrap should manage when `<name>` is selected.

## Steps

1. Create `config.<name>.toml`. Use native mise resources first: packages (`brew`, `brew-cask`, `mas`), dotfiles, repositories, tools, hooks, and macOS defaults.
2. Add `[[modules]]` to `essential-mise.toml` with the same `name` and a one-line `description`. A base module goes in `setup.base_modules` instead.
3. Put files that a `[dotfiles]` entry copies under `dotfiles/<module>/`.
4. Run `essential-mise-console lint --config-dir .`.
5. Preview, then apply in a disposable VM or a sandbox home: `MISE_CEILING_PATHS=$PWD mise -C . -E <name> bootstrap --dry-run`.

## Rules

- Do not name a module `config` or end a module file in `.local.toml`; those are never modules.
- Keep a module self-contained. If it needs another module, document the dependency in its description or put the shared part in a base module.
- Hooks must call scripts by `${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/<script>` so they work from any checkout location.
- Keep identity and secrets out of the repository. Use `*.local.toml` files (ignored by git) for machine-local overrides.
- Prefer native resources over hooks. A hook that wraps Homebrew needs a documented reason, health check, upgrade, and removal path.

The starter's `config.terminal.toml` is a working example.
