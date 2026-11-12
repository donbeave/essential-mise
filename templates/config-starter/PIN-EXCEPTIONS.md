# Pin exceptions

Pin every version that the package manager can pin. Entries that cannot be
pinned carry a `# pin-exception: <reason>` comment in the module file and are
listed here. This is a convention of the starter; the engine does not enforce it.

| Entry | Module | Reason | How upgrades are reviewed |
| --- | --- | --- | --- |
| `brew:mas` | `foundation` | Homebrew formulae have no version pins. | `mise bootstrap packages upgrade --dry-run`, then review. |
| `brew:starship` | `terminal` | Homebrew formulae have no version pins. | Same. |
| `brew:bat` | `terminal` | Homebrew formulae have no version pins. | Same. |

Find entries that are not recorded:

```bash
rg '"latest"' config*.toml | rg -v 'pin-exception:'
```
