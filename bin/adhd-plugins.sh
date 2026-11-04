#!/bin/bash
set -euo pipefail

bin=/opt/homebrew/bin
repository=ayghri/i-have-adhd
marketplace=i-have-adhd
plugin=i-have-adhd@i-have-adhd
clone="$HOME/.local/share/i-have-adhd"

install_claude() {
  local catalogs count installed user_count enabled
  installed="$("$bin/claude" plugin list --json)" || return
  "$bin/jq" -e 'type == "array" and all(.[]; type == "object" and (.id | type == "string") and (.scope | type == "string") and (.enabled | type == "boolean"))' <<<"$installed" >/dev/null || return
  user_count="$("$bin/jq" -r --arg id "$plugin" '[.[] | select(.id == $id and .scope == "user")] | length' <<<"$installed")" || return
  [[ "$user_count" == 0 || "$user_count" == 1 ]] || return 1
  if [[ "$user_count" == 1 ]]; then
    enabled="$("$bin/jq" -r --arg id "$plugin" '.[] | select(.id == $id and .scope == "user") | .enabled' <<<"$installed")" || return
    if [[ "$enabled" == false ]]; then
      printf 'PENDING %s/Claude: user disabled this plugin; leaving it unchanged\n' "$marketplace" >&2
      return 0
    fi
  fi
  catalogs="$("$bin/claude" plugin marketplace list --json)" || return
  "$bin/jq" -e 'type == "array" and all(.[]; type == "object" and (.name | type == "string"))' <<<"$catalogs" >/dev/null || return
  count="$("$bin/jq" -r --arg name "$marketplace" '[.[] | select(.name == $name)] | length' <<<"$catalogs")" || return
  if [[ "$count" == 0 ]]; then
    "$bin/claude" plugin marketplace add "$repository" || return
    catalogs="$("$bin/claude" plugin marketplace list --json)" || return
  fi
  if ! "$bin/jq" -e --arg name "$marketplace" --arg repo "$repository"     '[.[] | select(.name == $name)] | length == 1 and .[0].source == "github" and .[0].repo == $repo and (.[0].ref // "") == ""' <<<"$catalogs" >/dev/null; then
    printf '%s: refusing Claude marketplace with unexpected source\n' "$marketplace" >&2
    return 1
  fi
  "$bin/claude" plugin marketplace update "$marketplace"
  if [[ "$user_count" == 0 ]]; then
    "$bin/claude" plugin install "$plugin" --scope user
  else
    "$bin/claude" plugin update "$plugin" --scope user
  fi
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
