#!/bin/bash
# Installs or updates pstack through each AI CLI's own plugin or skill mechanism.
# Safe to repeat: every step reads the tool's current state and applies only the difference.
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
  "$bin/claude" plugin marketplace add "$repository"
  "$bin/claude" plugin marketplace update "$marketplace"
  "$bin/claude" plugin install "$plugin"
  "$bin/claude" plugin update "$plugin"
}

install_codex() {
  local installed
  "$bin/codex" plugin marketplace add "$repository"
  installed="$("$bin/codex" plugin list --marketplace "$marketplace" --json)"
  if "$bin/jq" -e '.installed | any(.name == "pstack")' <<<"$installed" >/dev/null; then
    "$bin/codex" plugin marketplace upgrade "$marketplace"
  else
    "$bin/codex" plugin add "$plugin"
  fi
}

install_omp() {
  local installed
  installed="$("$bin/omp" plugin list --json)"
  if "$bin/jq" -e '.npm | any(.name == "pstack")' <<<"$installed" >/dev/null; then
    "$bin/omp" plugin upgrade pstack
  else
    "$bin/omp" plugin install "git:github.com/$repository"
  fi
}

install_opencode() {
  local skills="${XDG_CONFIG_HOME:-$HOME/.config}/opencode/skills" source target link
  require_clone
  mkdir -p "$skills"
  for source in "$skills_source"/*/; do
    source="${source%/}"
    target="$skills/$(basename "$source")"
    if [[ -L "$target" && "$(readlink "$target")" == "$source" ]]; then
      continue
    elif [[ -e "$target" || -L "$target" ]]; then
      printf 'pstack: leaving existing %s in place\n' "$target" >&2
    else
      ln -s "$source" "$target"
    fi
  done
  for link in "$skills"/*; do
    if [[ -L "$link" && "$(readlink "$link")" == "$skills_source"/* && ! -e "$link" ]]; then
      rm "$link"
    fi
  done
}

install_muse() {
  require_clone
  if "$bin/muse" plugins inspect pstack >/dev/null 2>&1; then
    "$bin/muse" plugins update pstack
  else
    "$bin/muse" plugins install "$clone/plugins/pstack"
  fi
  "$bin/muse" plugins approve pstack
}

tools=("$@")
[[ ${#tools[@]} -gt 0 ]] || tools=(claude codex omp opencode muse)
for tool in "${tools[@]}"; do
  case "$tool" in
    claude | codex | omp | opencode | muse) ;;
    *)
      printf 'pstack: unknown tool %s\n' "$tool" >&2
      exit 2
      ;;
  esac
  printf '==> pstack: %s\n' "$tool"
  "install_$tool"
done
