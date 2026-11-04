#!/bin/bash
# Installs or updates pstack through each supported AI CLI's plugin mechanism.
# Repeat runs preserve disabled and unrelated installations where the agent supports it.
# Usage: pstack-plugins.sh [claude|codex|omp|opencode|muse ...]   (default: all)
set -euo pipefail

bin=/opt/homebrew/bin
repository=michael-denyer/pstack-claude
marketplace=pstack-claude
plugin="pstack@$marketplace"
clone="$HOME/.local/share/$marketplace"
skills_source="$clone/plugins/pstack/skills"

require_clone() {
  [[ -d "$skills_source" ]] || {
    printf 'pstack: %s is missing; apply bootstrap repos first\n' "$clone" >&2
    exit 1
  }
}

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

install_codex() (
  local installed
  if [[ -z "${HOME:-}" || "$HOME" != /* ]]; then
    printf "%s: refusing unset or relative HOME; use an absolute path\n" "$marketplace" >&2
    return 1
  fi
  if [[ -n "${CODEX_HOME:-}" && "$CODEX_HOME" != /* ]]; then
    printf "%s: refusing relative CODEX_HOME; use an absolute path\n" "$marketplace" >&2
    return 1
  fi
  if [[ -n "${CODEX_SQLITE_HOME:-}" && "$CODEX_SQLITE_HOME" != /* ]]; then
    printf "%s: refusing relative CODEX_SQLITE_HOME; use an absolute path\n" "$marketplace" >&2
    return 1
  fi
  if [[ -e /.codex || -L /.codex ]]; then
    printf "%s: refusing Codex operation with root project configuration\n" "$marketplace" >&2
    return 1
  fi
  cd / || return
  installed="$("$bin/codex" plugin list --marketplace "$marketplace" --json)" || return
  "$bin/jq" -e 'type == "object" and (.installed | type == "array" and all(.[]; type == "object" and (.name | type == "string" and length > 0) and (.marketplaceName | type == "string") and (.installed | type == "boolean" and . == true) and (.enabled | type == "boolean") and (.source | type == "object")))' <<<"$installed" >/dev/null || return
  if "$bin/jq" -e --arg name "${plugin%@*}" '.installed | any(.name != $name)' <<<"$installed" >/dev/null; then
    printf '%s: refusing to change marketplace with unrelated Codex plugins\n' "$marketplace" >&2
    return 1
  fi
  "$bin/codex" plugin marketplace add "$repository"
  installed="$("$bin/codex" plugin list --marketplace "$marketplace" --json)"
  "$bin/jq" -e 'type == "object" and (.installed | type == "array" and all(.[]; type == "object" and (.name | type == "string" and length > 0) and (.marketplaceName | type == "string") and (.installed | type == "boolean" and . == true) and (.enabled | type == "boolean") and (.source | type == "object")))' <<<"$installed" >/dev/null || return
  if "$bin/jq" -e --arg name "${plugin%@*}" '.installed | any(.name != $name)' <<<"$installed" >/dev/null; then
    printf '%s: refusing to upgrade unrelated Codex marketplace plugins\n' "$marketplace" >&2
    return 1
  fi
  if "$bin/jq" -e '.installed | any(.name == "pstack")' <<<"$installed" >/dev/null; then
    "$bin/codex" plugin marketplace upgrade "$marketplace"
  else
    "$bin/codex" plugin add "$plugin"
  fi
)

install_omp() {
  printf 'PENDING pstack/OMP: native integration awaits upstream PR 396 and a release; existing installations were not changed\n' >&2
}

install_opencode() {
  printf 'PENDING pstack/OpenCode: no verified native plugin; existing installations and legacy links were not changed\n' >&2
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
