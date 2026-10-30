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

[[ "$(/usr/bin/uname -s)" == 'Darwin' ]] || fail 'This script runs only on macOS.'
[[ "$(/usr/bin/uname -m)" == 'arm64' ]] || fail 'Use a native Apple Silicon Terminal; Intel and Rosetta are not supported.'
[[ "$(/usr/bin/id -u)" != '0' ]] || fail 'Run as your normal macOS user; installers request elevated access only when necessary.'
[[ -n "${HOME:-}" ]] || fail 'HOME is not set.'

product_version="$(/usr/bin/sw_vers -productVersion)"
IFS=. read -r macos_major macos_minor _ <<< "$product_version"
[[ "$macos_major" =~ ^[0-9]+$ && "$macos_minor" =~ ^[0-9]+$ ]] || fail "Cannot determine macOS version: $product_version"
(( macos_major > 27 || (macos_major == 27 && macos_minor >= 2) )) || \
  fail 'This setup requires macOS 27.2 or newer. Update macOS in System Settings, then run the same command again.'

say '1/6 - Apple Command Line Tools (Git)'
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

say '2/6 - Homebrew'
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

say "3/6 - mise ($MISE_VERSION)"
export MISE_VERSION
run_official_installer 'https://mise.run' /bin/sh
[[ -x "$MISE_BIN" ]] || fail "mise installer did not produce $MISE_BIN"
export PATH="$HOME/.local/bin:$PATH"
installed_mise_version="$("$MISE_BIN" --version | /usr/bin/awk '{print $1}')"
[[ "$installed_mise_version" == "${MISE_VERSION#v}" ]] || \
  fail "Expected mise ${MISE_VERSION#v}, found $installed_mise_version"

export MISE_CONFIG_DIR="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
[[ "$MISE_CONFIG_DIR" == /* ]] || fail "MISE_CONFIG_DIR must be absolute: $MISE_CONFIG_DIR"
[[ "$MISE_CONFIG_DIR" != '/' && "$MISE_CONFIG_DIR" != "$HOME" ]] || fail "Unsafe MISE_CONFIG_DIR: $MISE_CONFIG_DIR"
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
if [[ ! -e "$MISE_CONFIG_DIR" ]] || { [[ -d "$MISE_CONFIG_DIR" ]] && [[ -z "$(/bin/ls -A "$MISE_CONFIG_DIR")" ]]; }; then
  say '4/6 - First-run adoption of public configuration'
  work_dir="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/essential-mise-adopt.XXXXXX")"
  cd "$work_dir"
  MISE_CEILING_PATHS="$work_dir:$MISE_CONFIG_DIR" "$MISE_BIN" bootstrap --adopt "$REPO"
  first_adoption=1
else
  say '4/6 - Existing configuration detected; skipping adoption'
fi

validate_config_checkout
if [[ "$first_adoption" -eq 0 ]]; then
  origin="$(/usr/bin/git -C "$MISE_CONFIG_DIR" remote get-url origin 2>/dev/null || true)"
  case "$origin" in
    'https://github.com/donbeave/essential-mise'|'https://github.com/donbeave/essential-mise.git'|'git@github.com:donbeave/essential-mise'|'git@github.com:donbeave/essential-mise.git') ;;
    *) fail "Existing config has an unexpected or missing origin ($origin); refusing to overwrite or pull." ;;
  esac
fi

say '5/6 - Foundation only (zsh activation)'
export MISE_CEILING_PATHS="$MISE_CONFIG_DIR"
cd "$MISE_CONFIG_DIR"
"$MISE_BIN" -E foundation bootstrap

say '6/6 - Verify a new login and interactive zsh'
/bin/zsh -lic 'command -v git >/dev/null && command -v brew >/dev/null && command -v mise >/dev/null && mise --version && mise -E foundation doctor' || \
  fail 'New-shell verification failed. Open a fresh Terminal, inspect mise doctor, and rerun this command.'

say 'Finished. Open a fresh Terminal window.'
printf -v quoted_config_dir '%q' "$MISE_CONFIG_DIR"
printf '%s\n' \
  'Preview the standard workstation:' \
  "  MISE_CEILING_PATHS=$quoted_config_dir mise -C $quoted_config_dir -E foundation,terminal,development,security bootstrap --dry-run" \
  'Apply it after review:' \
  "  MISE_CEILING_PATHS=$quoted_config_dir mise -C $quoted_config_dir -E foundation,terminal,development,security bootstrap" \
  'Preview coding agents and shared tools:' \
  "  MISE_CEILING_PATHS=$quoted_config_dir mise -C $quoted_config_dir -E ai-tools,ai-cli bootstrap --dry-run" \
  'This script did NOT permanently select an optional environment.'
