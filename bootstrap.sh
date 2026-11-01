#!/bin/bash
set -euo pipefail

REPO='donbeave/essential-mise'
EXPECTED_ORIGIN='github.com/donbeave/essential-mise'
MISE_VERSION='v2026.10.23'
MISE_BIN="$HOME/.local/bin/mise"
BREW_BIN='/opt/homebrew/bin/brew'
tmp_script=''
work_dir=''

say() { printf '\n==> %s\n' "$*"; }
fail() { printf '\nERROR: %s\n' "$*" >&2; exit 1; }
cleanup() {
  if [[ -n "$tmp_script" ]]; then
    /bin/rm -f "$tmp_script"
  fi
  if [[ -n "$work_dir" ]]; then
    /bin/rm -rf "$work_dir"
  fi
}
trap cleanup EXIT

run_official_installer() {
  local url="$1" interpreter="$2" first_line=''
  tmp_script="$(/usr/bin/mktemp "${TMPDIR:-/tmp}/essential-mise.XXXXXX")"
  /usr/bin/curl -fsSL "$url" -o "$tmp_script" || fail "Failed to download $url"
  [[ -s "$tmp_script" ]] || fail "Downloaded installer is empty: $url"
  IFS= read -r first_line < "$tmp_script" || true
  [[ "$first_line" == '#!'* ]] || fail "Downloaded installer is not a shell script: $url"
  "$interpreter" "$tmp_script" || fail "Installer failed: $url"
  /bin/rm -f "$tmp_script"
  tmp_script=''
}

version_at_least() {
  local current="$1" minimum="$2" index
  local -a current_parts minimum_parts
  IFS=. read -r -a current_parts <<< "$current"
  IFS=. read -r -a minimum_parts <<< "$minimum"
  for index in 0 1 2; do
    [[ "${current_parts[$index]:-0}" =~ ^[0-9]+$ && "${minimum_parts[$index]:-0}" =~ ^[0-9]+$ ]] || return 1
    (( 10#${current_parts[$index]:-0} > 10#${minimum_parts[$index]:-0} )) && return 0
    (( 10#${current_parts[$index]:-0} < 10#${minimum_parts[$index]:-0} )) && return 1
  done
  return 0
}

[[ "$(/usr/bin/uname -s)" == 'Darwin' ]] || fail 'This script runs only on macOS.'
[[ "$(/usr/bin/uname -m)" == 'arm64' ]] || fail 'Use a native Apple Silicon Terminal; Intel and Rosetta are not supported.'
[[ "$(/usr/bin/id -u)" != '0' ]] || fail 'Run as your normal macOS user; installers request elevated access only when necessary.'
[[ -n "${HOME:-}" ]] || fail 'HOME is not set.'

product_version="$(/usr/bin/sw_vers -productVersion)"
IFS=. read -r macos_major macos_minor _ <<< "$product_version"
[[ "$macos_major" =~ ^[0-9]+$ && "$macos_minor" =~ ^[0-9]+$ ]] || fail "Cannot determine macOS version: $product_version"
(( macos_major > 27 || (macos_major == 27 && macos_minor >= 2) )) || \
  fail 'This setup requires macOS 27.2 or newer. Update macOS in System Settings, then run the same command again.'

say '1/7 - Apple Command Line Tools (Git)'
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || ! /usr/bin/git --version >/dev/null 2>&1; then
  say 'macOS will open a Command Line Tools installation dialog.'
  /usr/bin/xcode-select --install || true
  if [[ ! -r /dev/tty ]]; then
    fail 'Finish the Apple installation dialog in Terminal, then rerun this command.'
  fi
  printf '\nFinish the Apple installation dialog, then press Return to continue... ' >/dev/tty
  IFS= read -r _ </dev/tty
fi
/usr/bin/xcode-select -p >/dev/null 2>&1 || fail 'Command Line Tools are not ready. Finish installation, then rerun.'
/usr/bin/git --version >/dev/null 2>&1 || fail 'Git is not ready. Finish Command Line Tools installation, then rerun.'

say '2/7 - Homebrew'
if [[ ! -x "$BREW_BIN" ]]; then
  run_official_installer 'https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh' /bin/bash
fi
[[ -x "$BREW_BIN" ]] || fail "Homebrew installation did not produce $BREW_BIN"
brew_shellenv=''
if ! brew_shellenv="$("$BREW_BIN" shellenv)"; then
  fail 'brew shellenv failed.'
fi
eval "$brew_shellenv"
command -v brew >/dev/null 2>&1 || fail 'brew is not available after shellenv initialization.'
[[ "$("$BREW_BIN" --prefix)" == '/opt/homebrew' ]] || fail 'Homebrew prefix is not /opt/homebrew.'
brew_shellenv_line='eval "$(/opt/homebrew/bin/brew shellenv)"'
if [[ ! -f "$HOME/.zprofile" ]] || ! /usr/bin/grep -Fqx "$brew_shellenv_line" "$HOME/.zprofile"; then
  printf '\n# Homebrew (essential-mise stage 0)\n%s\n' "$brew_shellenv_line" >> "$HOME/.zprofile"
fi

say "3/7 - mise (minimum $MISE_VERSION)"
installed_mise_version=''
if [[ -x "$MISE_BIN" ]]; then
  installed_mise_version="$("$MISE_BIN" --version | /usr/bin/awk '{print $1}')"
fi
if [[ -z "$installed_mise_version" ]] || ! version_at_least "$installed_mise_version" "${MISE_VERSION#v}"; then
  export MISE_VERSION
  run_official_installer 'https://mise.run' /bin/sh
fi
[[ -x "$MISE_BIN" ]] || fail "mise installer did not produce $MISE_BIN"
export PATH="$HOME/.local/bin:$PATH"
installed_mise_version="$("$MISE_BIN" --version | /usr/bin/awk '{print $1}')"
version_at_least "$installed_mise_version" "${MISE_VERSION#v}" || \
  fail "Expected mise ${MISE_VERSION#v} or newer, found $installed_mise_version"
printf 'Using mise %s; no downgrade needed.\n' "$installed_mise_version"

export MISE_CONFIG_DIR="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
[[ "$MISE_CONFIG_DIR" == /* ]] || fail "MISE_CONFIG_DIR must be absolute: $MISE_CONFIG_DIR"
[[ "$MISE_CONFIG_DIR" != '/' && "$MISE_CONFIG_DIR" != "$HOME" ]] || fail "Unsafe MISE_CONFIG_DIR: $MISE_CONFIG_DIR"
[[ "$MISE_CONFIG_DIR" != *:* ]] || fail "MISE_CONFIG_DIR cannot contain a colon because MISE_CEILING_PATHS is colon-separated: $MISE_CONFIG_DIR"
config_path=''
IFS='/' read -r -a config_components <<< "${MISE_CONFIG_DIR#/}"
for component in "${config_components[@]}"; do
  [[ -n "$component" && "$component" != '.' && "$component" != '..' ]] || \
    fail "MISE_CONFIG_DIR contains an empty or relative path component: $MISE_CONFIG_DIR"
  config_path+="/$component"
  [[ ! -L "$config_path" ]] || fail "MISE_CONFIG_DIR cannot traverse a symlink: $config_path"
done
[[ "$config_path" == "$MISE_CONFIG_DIR" ]] || fail "MISE_CONFIG_DIR is not normalized: $MISE_CONFIG_DIR"
config_parent="$MISE_CONFIG_DIR"
while [[ ! -d "$config_parent" ]]; do
  config_parent="${config_parent%/*}"
  [[ -n "$config_parent" ]] || config_parent='/'
done
physical_parent="$(cd -P "$config_parent" && pwd -P)" || fail 'Cannot resolve the configuration parent.'
[[ "$physical_parent" == "$config_parent" ]] || fail "MISE_CONFIG_DIR is not filesystem-canonical: $MISE_CONFIG_DIR"
unset MISE_ENV MISE_GLOBAL_CONFIG_FILE

validate_config_checkout() {
  local inside bare top physical_config physical_top
  [[ -d "$MISE_CONFIG_DIR" ]] || fail "Configuration directory was not created: $MISE_CONFIG_DIR"
  inside="$(/usr/bin/git -C "$MISE_CONFIG_DIR" rev-parse --is-inside-work-tree 2>/dev/null)" || \
    fail 'Existing config is not a Git worktree.'
  [[ "$inside" == 'true' ]] || fail 'Existing config is not a Git worktree.'
  bare="$(/usr/bin/git -C "$MISE_CONFIG_DIR" rev-parse --is-bare-repository)" || fail 'Cannot determine Git repository type.'
  [[ "$bare" == 'false' ]] || fail 'Existing config cannot be a bare repository.'
  top="$(/usr/bin/git -C "$MISE_CONFIG_DIR" rev-parse --show-toplevel)" || fail 'Cannot determine the configuration worktree root.'
  physical_config="$(cd -P "$MISE_CONFIG_DIR" && pwd -P)" || fail 'Cannot resolve the configuration directory.'
  physical_top="$(cd -P "$top" && pwd -P)" || fail 'Cannot resolve the configuration worktree root.'
  [[ "$physical_top" == "$physical_config" ]] || fail "Config path must be the actual worktree root, not $top"
}

first_adoption=0
if [[ -e "$MISE_CONFIG_DIR" ]] && { [[ ! -d "$MISE_CONFIG_DIR" ]] || [[ -n "$(/bin/ls -A "$MISE_CONFIG_DIR")" ]]; }; then
  inside="$(/usr/bin/git -C "$MISE_CONFIG_DIR" rev-parse --is-inside-work-tree 2>/dev/null || true)"
  bare="$(/usr/bin/git -C "$MISE_CONFIG_DIR" rev-parse --is-bare-repository 2>/dev/null || true)"
  if [[ "$inside" == 'true' || "$bare" == 'true' ]]; then
    say '4/7 - Existing configuration detected; skipping adoption'
  else
    say '4/7 - Removing existing non-Git configuration'
    /bin/rm -rf "$MISE_CONFIG_DIR" || fail "Cannot remove existing configuration: $MISE_CONFIG_DIR"
  fi
fi
if [[ ! -e "$MISE_CONFIG_DIR" ]] || { [[ -d "$MISE_CONFIG_DIR" ]] && [[ -z "$(/bin/ls -A "$MISE_CONFIG_DIR")" ]]; }; then
  say '4/7 - First-run adoption of public configuration'
  work_dir="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/essential-mise-adopt.XXXXXX")"
  [[ "$work_dir" != *:* ]] || fail "Temporary adoption directory cannot contain a colon because MISE_CEILING_PATHS is colon-separated: $work_dir"
  cd "$work_dir"
  MISE_CEILING_PATHS="$work_dir:$MISE_CONFIG_DIR" "$MISE_BIN" bootstrap --adopt "$REPO"
  first_adoption=1
fi

validate_config_checkout
if [[ "$first_adoption" -eq 0 ]]; then
  origin="$(/usr/bin/git -C "$MISE_CONFIG_DIR" remote get-url origin 2>/dev/null || true)"
  case "$origin" in
    'https://github.com/donbeave/essential-mise'|'https://github.com/donbeave/essential-mise.git'|'git@github.com:donbeave/essential-mise'|'git@github.com:donbeave/essential-mise.git') ;;
    *) fail "Existing config has an unexpected or missing origin ($origin); refusing to overwrite or pull." ;;
  esac
fi

say '5/7 - Foundation (mas CLI and zsh activation)'
export MISE_CEILING_PATHS="$MISE_CONFIG_DIR"
cd "$MISE_CONFIG_DIR"
"$MISE_BIN" -E foundation bootstrap

say '6/7 - Optional categories'
environment_names=(
  terminal development security js development-rust development-java
  development-apps development-containers development-editors development-apple
  kubernetes infrastructure utilities messengers browsers privacy productivity
  office design media network admin ai-tools ai-cli ai-desktop system-preferences
)
environment_descriptions=(
  'Ghostty, shells, prompt, fonts, and terminal utilities'
  'GitHub CLI and Git LFS'
  '1Password apps, CLI, and Safari extension'
  'Node.js and Bun'
  'Rust, LLVM tools, and mr-boxington'
  'GraalVM, Gradle, Maven, and VisualVM'
  'JetBrains Toolbox, Yaak, TablePlus, DevHub, and Patterns'
  'OrbStack container runtime'
  'Sublime Text, VS Code, Zed, and developer font'
  'Xcode from the Mac App Store'
  'kubectl, kubectx, and Helm'
  'OpenTofu, Ansible, Linode CLI, and gomplate'
  'CleanShot, Bartender, Homebrew App, and Keka'
  'Telegram, WhatsApp, Slack, WeChat, and Zoom'
  'Chrome, Chrome Dev, and Firefox Developer Edition'
  'Tor Browser and Little Snitch'
  'Notion, Grammarly, Speechify, Superwhisper, Raindrop.io, and Reeder'
  'Word, Excel, PDF Expert, Delimited, and Create Booklet 2'
  'Sketch and Pixelmator Pro'
  'VLC, Transmission, HandBrake, Plex, Blackmagic RAW, and Shazam'
  'Surge, Cloudflare WARP, Tunnelblick, and WhyFi'
  'Transmit, Apple Remote Desktop, Disk Aid, iStatistica Pro, and Apple Configurator'
  'Search, parsing, shell, and coding-agent support utilities'
  'Claude Code, Codex, Kimi Code, OpenCode, and other terminal agents'
  'Desktop AI applications and coding clients'
  'Dock, keyboard, Finder, locale, privacy, and storage settings'
)
selection=''
if [[ -t 0 ]]; then
  for index in "${!environment_names[@]}"; do
    printf '%2d) %s - %s\n' "$((index + 1))" "${environment_names[$index]}" "${environment_descriptions[$index]}"
  done
  printf '\nEnter numbers separated by spaces or commas, or press Return to skip: '
  IFS= read -r selection || fail 'Cannot read category selection.'
else
  printf 'No interactive terminal available; skipping optional categories.\n'
fi

selected_environments=''
if [[ -n "$selection" ]]; then
  IFS=' ' read -r -a choices <<< "${selection//,/ }"
  for choice in "${choices[@]}"; do
    [[ "$choice" =~ ^[0-9]+$ ]] || fail "Invalid category number: $choice"
    (( choice >= 1 && choice <= ${#environment_names[@]} )) || fail "Category number is out of range: $choice"
    environment="${environment_names[$((choice - 1))]}"
    case ",$selected_environments," in
      *",$environment,"*) ;;
      *) selected_environments+="${selected_environments:+,}$environment" ;;
    esac
  done
fi

if [[ -n "$selected_environments" ]]; then
  say "Installing selected categories: $selected_environments"
  "$MISE_BIN" -E "$selected_environments" bootstrap || fail 'Selected category installation failed.'
else
  printf 'No optional categories selected.\n'
fi

say '7/7 - Verify a new login and interactive zsh'
/bin/zsh -lic 'command -v git >/dev/null && command -v brew >/dev/null && command -v mise >/dev/null && command -v mas >/dev/null && mise --version && mas --version && mise -E foundation doctor' || \
  fail 'New-shell verification failed. Open a fresh Terminal, inspect mise doctor, and rerun this command.'

say 'Finished. Open a fresh Terminal window.'
if [[ -n "$selected_environments" ]]; then
  printf 'Selected categories were installed once; no optional environment was permanently selected.\n'
fi
