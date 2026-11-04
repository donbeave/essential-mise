#!/bin/bash
# Remove pre-namespacing agent files that essential-mise no longer installs.
# Managed set is the 19 tailrocks-*.md files copied from
# dotfiles/ai-cli/claude/agents (config.ai-cli.toml); mode=copy never prunes,
# so these legacy names would otherwise linger forever in both agent dirs.
set -euo pipefail

home="${HOME:?}"
legacy=(
  a11y-auditor.md
  agent-sandbox-engineer.md
  architect.md
  cicd-architect.md
  code-reviewer.md
  debugger.md
  design-director.md
  docs-researcher.md
  docs-writer.md
  fable-architect.md
  frontend-engineer.md
  locator.md
  mechanic.md
  openshell-specialist.md
  perf-engineer.md
  qa-engineer.md
  rust-cli-tui-engineer.md
  rust-engineer.md
  security-reviewer.md
  tanstack-start-specialist.md
  test-reviewer.md
  ux-specialist.md
  verifier.md
)
dirs=("$home/.claude/agents" "$home/.grok/agents")

usage() { printf 'usage: %s [apply|status]\n' "$(basename "$0")" >&2; exit 2; }
verb="${1:-apply}"
[[ "$verb" == 'apply' || "$verb" == 'status' ]] || usage

remaining=0
for dir in "${dirs[@]}"; do
  [[ -d "$dir" ]] || continue
  for name in "${legacy[@]}"; do
    target="$dir/$name"
    [[ -e "$target" || -L "$target" ]] || continue
    if [[ "$verb" == 'apply' ]]; then
      /bin/rm -f "$target"
      printf 'removed %s\n' "$target"
    else
      printf 'stale %s\n' "$target"
      remaining=1
    fi
  done
done
if [[ "$verb" == 'status' ]]; then
  [[ "$remaining" -eq 0 ]] && printf 'no legacy agent files\n'
  exit "$remaining"
fi
printf 'legacy agent files pruned\n'
