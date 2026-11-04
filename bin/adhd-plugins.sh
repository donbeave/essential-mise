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

install_codex() (
  local installed
  local -a path_parts
  if [[ -z "${HOME:-}" || "$HOME" != /* ]]; then
    printf "%s: refusing unset or relative HOME; use an absolute path\n" "$marketplace" >&2
    return 1
  fi
  if [[ -n "${XDG_CONFIG_HOME:-}" && "$XDG_CONFIG_HOME" != /* ]]; then
    printf "%s: refusing relative XDG_CONFIG_HOME; use an absolute path\n" "$marketplace" >&2
    return 1
  fi
  for git_config_var in GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM; do
    git_config_value="${!git_config_var:-}"
    if [[ -n "$git_config_value" && "$git_config_value" != /* ]]; then
      printf "%s: refusing relative %s; use an absolute path\n" "$marketplace" "$git_config_var" >&2
      return 1
    fi
  done
  for git_repo_var in GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CONFIG GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS GIT_CEILING_DIRECTORIES GIT_GRAFT_FILE GIT_SHALLOW_FILE GIT_NAMESPACE GIT_SSH GIT_SSH_COMMAND GIT_EXEC_PATH GIT_TEMPLATE_DIR; do
    if [[ -n "${!git_repo_var:-}" ]]; then
      printf "%s: refusing %s during Codex plugin operations\n" "$marketplace" "$git_repo_var" >&2
      return 1
    fi
  done
  if [[ -z "${PATH:-}" || "$PATH" == :* || "$PATH" == *: || "$PATH" == *::* || "$PATH" == *$'\n'* ]]; then
    printf "%s: refusing empty or newline-containing PATH during Codex plugin operations\n" "$marketplace" >&2
    return 1
  fi
  local path_part
  IFS=: read -r -a path_parts <<<"$PATH"
  for path_part in "${path_parts[@]}"; do
    if [[ "$path_part" != /* ]]; then
      printf "%s: refusing relative PATH entry during Codex plugin operations\n" "$marketplace" >&2
      return 1
    fi
  done
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
  "$bin/codex" plugin marketplace add "$repository" --ref main
  installed="$("$bin/codex" plugin list --marketplace "$marketplace" --json)"
  "$bin/jq" -e 'type == "object" and (.installed | type == "array" and all(.[]; type == "object" and (.name | type == "string" and length > 0) and (.marketplaceName | type == "string") and (.installed | type == "boolean" and . == true) and (.enabled | type == "boolean") and (.source | type == "object")))' <<<"$installed" >/dev/null || return
  if "$bin/jq" -e --arg name "${plugin%@*}" '.installed | any(.name != $name)' <<<"$installed" >/dev/null; then
    printf '%s: refusing to upgrade unrelated Codex marketplace plugins\n' "$marketplace" >&2
    return 1
  fi
  if "$bin/jq" -e '.installed | any(.name == "i-have-adhd")' <<<"$installed" >/dev/null; then
    "$bin/codex" plugin marketplace upgrade "$marketplace"
  else
    "$bin/codex" plugin add "$plugin"
  fi
)

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
