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
[[ -n "$brew_shellenv" ]] || fail 'brew shellenv returned no output.'
eval "$brew_shellenv"
command -v brew >/dev/null 2>&1 || fail 'brew is not available after shellenv initialization.'
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
[[ "$MISE_CONFIG_DIR" != *'..'* ]] || fail "MISE_CONFIG_DIR cannot contain parent traversal: $MISE_CONFIG_DIR"
[[ ! -L "$MISE_CONFIG_DIR" ]] || fail "MISE_CONFIG_DIR cannot be a symlink: $MISE_CONFIG_DIR"
unset MISE_ENV MISE_GLOBAL_CONFIG_FILE

if [[ ! -e "$MISE_CONFIG_DIR" ]] || { [[ -d "$MISE_CONFIG_DIR" ]] && [[ -z "$(/bin/ls -A "$MISE_CONFIG_DIR")" ]]; }; then
  say '4/6 - First-run adoption of public configuration'
  work_dir="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/essential-mise-adopt.XXXXXX")"
  cd "$work_dir"
  "$MISE_BIN" bootstrap --adopt "$REPO"
else
  say '4/6 - Existing configuration detected; skipping adoption'
  [[ -d "$MISE_CONFIG_DIR" ]] || fail "Config path is not a directory: $MISE_CONFIG_DIR"
  origin="$(/usr/bin/git -C "$MISE_CONFIG_DIR" remote get-url origin 2>/dev/null || true)"
  case "$origin" in
    'https://github.com/donbeave/essential-mise'|'https://github.com/donbeave/essential-mise.git'|'git@github.com:donbeave/essential-mise'|'git@github.com:donbeave/essential-mise.git') ;;
    *) fail "Existing config has an unexpected or missing origin ($origin); refusing to overwrite or pull." ;;
  esac
fi

[[ -d "$MISE_CONFIG_DIR" ]] || fail "Configuration directory was not created: $MISE_CONFIG_DIR"

say '5/6 - Foundation only (zsh activation)'
cd "$MISE_CONFIG_DIR"
"$MISE_BIN" -E foundation bootstrap

say '6/6 - Verify a new login and interactive zsh'
/bin/zsh -lic 'command -v git >/dev/null && command -v brew >/dev/null && command -v mise >/dev/null && mise --version && mise -E foundation doctor' || \
  fail 'New-shell verification failed. Open a fresh Terminal, inspect mise doctor, and rerun this command.'

say 'Finished. Open a fresh Terminal window.'
printf '%s\n' \
  'Preview the standard workstation:' \
  '  mise -E foundation,terminal,development,security bootstrap --dry-run' \
  'Apply it after review:' \
  '  mise -E foundation,terminal,development,security bootstrap' \
  'Preview coding agents and shared tools:' \
  '  mise -E ai-tools,ai-cli bootstrap --dry-run' \
  'This script did NOT permanently select an optional environment.'
