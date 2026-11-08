#!/bin/bash
set -euo pipefail

REPO='donbeave/essential-mise'
EMC_VERSION='v0.1.2'
EMC_SHA256='cc653eac8a2e193a154a6feaa2585e26119c13f05f992c6cb337838d77d42a41'
EMC_URL="https://github.com/${REPO}/releases/download/${EMC_VERSION}/essential-mise-console-aarch64-apple-darwin.tar.gz"

# Module catalogue parsed by essential-mise-console (spec §0: 29 environments).
# shellcheck disable=SC2034
environment_names=(
  terminal github security js development-rust development-java
  development-db development-containers development-editors development-vscode development-apple
  kubernetes infrastructure utilities messengers browsers development-browsers privacy
  productivity office office-docs design media network admin ai-tools ai-cli ai-desktop system-preferences
)
# shellcheck disable=SC2034
environment_descriptions=(
  'Ghostty, shells, prompt, fonts, and terminal utilities'
  'GitHub CLI and Git LFS'
  '1Password apps, CLI, and Safari extension'
  'Node.js and Bun'
  'Rust, LLVM tools, mr-boxington, and JetBrains Toolbox'
  'GraalVM, Gradle, Maven, VisualVM, and JetBrains Toolbox'
  'Yaak and TablePlus'
  'OrbStack container runtime'
  'Sublime Text and Zed'
  'Visual Studio Code'
  'Xcode from the Mac App Store'
  'kubectl, kubectx, and Helm'
  'OpenTofu, Ansible, Linode CLI, and gomplate'
  'CleanShot, Bartender, Homebrew App, and Keka'
  'Telegram, WhatsApp, Slack, WeChat, and Zoom'
  'Google Chrome'
  'Chrome Dev and Firefox Developer Edition'
  'Tor Browser and Little Snitch'
  'Notion, Grammarly, Speechify, Superwhisper, and Reeder'
  'Word and Excel'
  'PDF Expert, Delimited, Create Booklet 2, and OpenMark'
  'Sketch and Pixelmator Pro'
  'VLC, Transmission, HandBrake, Plex, Blackmagic RAW, and Shazam'
  'Surge, Cloudflare WARP, Tunnelblick, and WhyFi'
  'Transmit, Apple Remote Desktop, Disk Aid, iStatistica Pro, and Apple Configurator'
  'Search, parsing, shell, and coding-agent support utilities'
  'Claude Code, Codex, Kimi Code, OpenCode, and other terminal agents'
  'Desktop AI applications and coding clients'
  'Dock, keyboard, Finder, locale, privacy, and storage settings'
)

tmp_archive=''

say() { printf '==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
cleanup() {
  if [[ -n "$tmp_archive" ]]; then
    /bin/rm -f "$tmp_archive"
  fi
}
trap cleanup EXIT

printf 'essential-mise bootstrap (console %s)\n' "$EMC_VERSION"

check_platform() {
  [[ "$(/usr/bin/uname -s)" == 'Darwin' ]] || return 1
  [[ "$(/usr/bin/uname -m)" == 'arm64' ]] || return 1
  [[ "$(/usr/bin/id -u)" != '0' ]] || return 1
  [[ -n "${HOME:-}" ]] || return 1
  local product_version macos_major macos_minor
  product_version="$(/usr/bin/sw_vers -productVersion 2>/dev/null)" || return 1
  IFS=. read -r macos_major macos_minor _ <<< "$product_version"
  [[ "$macos_major" =~ ^[0-9]+$ && "$macos_minor" =~ ^[0-9]+$ ]] || return 1
  (( macos_major > 27 || (macos_major == 27 && macos_minor >= 2) )) || return 1
}

if ! check_platform; then
  fail 'This setup runs only on Apple Silicon macOS 27.2+.'
fi
say '[1/3] Checking platform (Apple Silicon, macOS 27.2+)... ok'

cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/essential-mise"
/bin/mkdir -p "$cache_dir"
cached_archive="${cache_dir}/essential-mise-console-${EMC_VERSION}-aarch64-apple-darwin.tar.gz"
cached_bin="${cache_dir}/essential-mise-console-${EMC_VERSION}"

verify_sha256() {
  local file="$1" actual=''
  actual="$(/usr/bin/shasum -a 256 "$file" | /usr/bin/awk '{print $1}')"
  if [[ "$actual" != "$EMC_SHA256" ]]; then
    fail "Checksum mismatch for $file (expected $EMC_SHA256, got $actual). Refusing to run."
  fi
}

if [[ -f "$cached_archive" && -x "$cached_bin" ]]; then
  verify_sha256 "$cached_archive"
  /usr/bin/tar -xzf "$cached_archive" -C "$cache_dir" essential-mise-console
  /bin/mv -f "${cache_dir}/essential-mise-console" "$cached_bin"
  /bin/chmod 0755 "$cached_bin"
  say "[2/3] Using cached console ${EMC_VERSION} ... checksum ok"
else
  say "[2/3] Downloading console ${EMC_VERSION} ..."
  tmp_archive="$(/usr/bin/mktemp "${TMPDIR:-/tmp}/essential-mise-console.XXXXXX")"
  if ! /usr/bin/curl -fL -# "$EMC_URL" -o "$tmp_archive" || [[ ! -s "$tmp_archive" ]]; then
    fail "Failed to download ${EMC_URL}."
  fi
  verify_sha256 "$tmp_archive"
  printf '    checksum ok (sha256:%s…)\n' "${EMC_SHA256:0:7}"
  /bin/mv -f "$tmp_archive" "$cached_archive"
  tmp_archive=''
  /usr/bin/tar -xzf "$cached_archive" -C "$cache_dir" essential-mise-console
  /bin/mv -f "${cache_dir}/essential-mise-console" "$cached_bin"
  /bin/chmod 0755 "$cached_bin"
fi

say "[3/3] Starting console ${EMC_VERSION} ..."
exec "$cached_bin" setup "$@"
