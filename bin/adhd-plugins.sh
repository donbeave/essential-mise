#!/bin/bash
set -euo pipefail

bin=/opt/homebrew/bin
marketplace=i-have-adhd
clone="$HOME/.local/share/i-have-adhd"

install_claude() {
  printf 'PENDING %s/Claude: concurrent native configuration writes can lose changes; existing installations were not changed\n' "$marketplace" >&2
}

install_codex() {
  printf 'PENDING %s/Codex: concurrent native configuration writes can lose changes; existing installations were not changed\n' "$marketplace" >&2
}

install_omp() {
  printf 'PENDING i-have-adhd/OMP: shared cache can affect other projects; existing installations were not changed\n' >&2
}

install_opencode() {
  printf 'PENDING i-have-adhd/OpenCode: native plugin installation requires manual review; existing installations were not changed\n' >&2
}

install_muse() {
  [[ -f "$clone/.claude-plugin/plugin.json" ]] || {
    printf 'i-have-adhd: %s is missing; apply bootstrap repos first\n' "$clone" >&2
    return 1
  }
  "$bin/muse" plugins install "$clone"
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
