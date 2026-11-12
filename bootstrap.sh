#!/bin/bash
set -euo pipefail

REPO='donbeave/essential-mise'
EMC_VERSION='v0.2.0'
EMC_SHA256='aeab5f1a99f9143d7fc525890c52935888ce2d2ef2c10343fe9019216338fc63'
EMC_URL="https://github.com/${REPO}/releases/download/${EMC_VERSION}/essential-mise-console-aarch64-apple-darwin.tar.gz"

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
