#!/bin/bash
set -euo pipefail

bin=/opt/homebrew/bin
rtk="$bin/rtk"
home="${HOME:?}"

have() { [[ -x "$bin/$1" ]]; }

write_if_changed() {
  local target="$1" temporary
  temporary="$(mktemp "$target.XXXXXX")"
  cat >"$temporary"
  if [[ -f "$target" ]] && cmp -s "$temporary" "$target"; then
    rm "$temporary"
    return 1
  fi
  mv "$temporary" "$target"
}

setup_claude() {
  local arguments=(init -g --auto-patch --no-trust-filters)
  have opencode && arguments+=(--opencode)
  "$rtk" "${arguments[@]}" </dev/null
}

setup_codex() {
  "$rtk" init -g --codex </dev/null
}

setup_antigravity() {
  "$rtk" init -g --agent antigravity --auto-patch --no-trust-filters </dev/null
}

setup_omp() {
  "$rtk" init -g --agent omp --auto-patch --no-trust-filters </dev/null
}

setup_grok() {
  mkdir -p "$home/.grok/hooks"
  write_if_changed "$home/.grok/hooks/rtk.json" <<'JSON' || true
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "rtk hook claude" }]
      }
    ]
  }
}
JSON
}

setup_muse() {
  printf 'PENDING rtk/Muse: existing plugin ownership and approval cannot be preserved safely; no changes made\n' >&2
}

binary_for() {
  case "$1" in
    claude) echo "claude" ;;
    codex) echo "codex" ;;
    antigravity) echo "agy" ;;
    omp) echo "omp" ;;
    grok) echo "grok" ;;
    muse) echo "muse" ;;
  esac
}

if [[ ! -x "$rtk" ]]; then
  printf 'rtk: %s is missing; select ai-tools to enable RTK for the agents\n' "$rtk" >&2
  exit 0
fi

tools=("$@")
[[ ${#tools[@]} -gt 0 ]] || tools=(claude codex antigravity omp grok muse)
for tool in "${tools[@]}"; do
  case "$tool" in
    claude | codex | antigravity | omp | grok | muse) ;;
    *)
      printf 'rtk: unknown tool %s\n' "$tool" >&2
      exit 2
      ;;
  esac
  if [[ "$tool" == claude ]]; then
    have claude || have opencode || {
      printf 'rtk: skipping claude and opencode; neither is installed\n' >&2
      continue
    }
  elif ! have "$(binary_for "$tool")"; then
    printf 'rtk: skipping %s; %s is not installed\n' "$tool" "$(binary_for "$tool")" >&2
    continue
  fi
  printf '==> rtk: %s\n' "$tool"
  "setup_$tool"
done
