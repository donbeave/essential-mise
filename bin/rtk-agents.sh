#!/bin/bash
set -euo pipefail

bin=/opt/homebrew/bin
rtk="$bin/rtk"
home="${HOME:?}"
muse_plugin="${XDG_DATA_HOME:-$home/.local/share}/essential-mise/rtk-muse-plugin"

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
  local installed recorded cache
  mkdir -p "$muse_plugin/.muse-plugin"
  write_if_changed "$muse_plugin/.muse-plugin/plugin.json" <<JSON || true
{
  "schemaVersion": 1,
  "name": "rtk",
  "displayName": "RTK",
  "version": "1.0.0",
  "description": "Rewrites shell commands through rtk to save tokens",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "hooks": [
      {
        "id": "rewrite",
        "event": "PreToolUse",
        "command": ["/bin/sh", "-c", "$rtk hook claude | $bin/jq -c 'del(.hookSpecificOutput.permissionDecisionReason)'"],
        "timeoutMs": 5000
      }
    ]
  }
}
JSON
  if installed="$("$bin/muse" plugins inspect rtk --json 2>/dev/null)"; then
    recorded="$("$bin/jq" -r '.record.source.path' <<<"$installed")"
    cache="$("$bin/jq" -r '.record.cache_path' <<<"$installed")"
    if [[ "$(cd "$recorded" 2>/dev/null && pwd -P)" != "$(cd "$muse_plugin" && pwd -P)" ]]; then
      "$bin/muse" plugins remove rtk
      "$bin/muse" plugins install "$muse_plugin"
    elif ! cmp -s "$cache/.muse-plugin/plugin.json" "$muse_plugin/.muse-plugin/plugin.json"; then
      "$bin/muse" plugins update rtk
    fi
  else
    "$bin/muse" plugins install "$muse_plugin"
  fi
  "$bin/muse" plugins approve rtk
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
  elif [[ "$tool" == muse ]] && ! have jq; then
    printf 'rtk: skipping muse; jq is not installed\n' >&2
    continue
  elif ! have "$(binary_for "$tool")"; then
    printf 'rtk: skipping %s; %s is not installed\n' "$tool" "$(binary_for "$tool")" >&2
    continue
  fi
  printf '==> rtk: %s\n' "$tool"
  "setup_$tool"
done
