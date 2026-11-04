#!/bin/bash
set -euo pipefail

bin=/opt/homebrew/bin
marketplace=pstack-claude
clone="$HOME/.local/share/$marketplace"
skills_source="$clone/plugins/pstack/skills"

require_clone() {
  [[ -d "$skills_source" ]] || {
    printf 'pstack: %s is missing; apply bootstrap repos first\n' "$clone" >&2
    exit 1
  }
}

install_claude() {
  printf 'PENDING %s/Claude: concurrent native configuration writes can lose changes; existing installations were not changed\n' "$marketplace" >&2
}

install_codex() {
  printf 'PENDING %s/Codex: concurrent native configuration writes can lose changes; existing installations were not changed\n' "$marketplace" >&2
}

install_omp() {
  printf 'PENDING pstack/OMP: native integration awaits upstream PR 396 and a release; existing installations were not changed\n' >&2
}

install_opencode() {
  printf 'PENDING pstack/OpenCode: upstream PR 397 and a release are required; existing installations and legacy links were not changed\n' >&2
}

install_muse() {
  require_clone
  "$bin/muse" plugins install "$clone/plugins/pstack"
}

if [[ "${1:-}" == --single ]]; then
  shift
  "install_$1"
  exit $?
fi

tools=("$@")
[[ ${#tools[@]} -gt 0 ]] || tools=(claude codex omp opencode muse)
for tool in "${tools[@]}"; do
  case "$tool" in
    claude | codex | omp | opencode | muse) ;;
    *) printf '%s: unknown tool %s\n' "$marketplace" "$tool" >&2; exit 2 ;;
  esac
done
status=0
for tool in "${tools[@]}"; do
  printf '==> %s: %s\n' "$marketplace" "$tool"
  if /bin/bash "$0" --single "$tool"; then
    :
  else
    printf '%s: %s failed\n' "$marketplace" "$tool" >&2
    status=1
  fi
done
exit "$status"
