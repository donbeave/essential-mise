#!/bin/bash
set -euo pipefail

brew=/opt/homebrew/bin/brew
formula=ampcode
evidence=/tmp/essential-mise-evidence/hook-lifecycle
rm -rf "$evidence"
mkdir -p "$evidence"
printf 'IN_PROGRESS\n' > "$evidence/report.txt"
linked="$($brew --prefix "$formula")"
current="$(readlink "$linked")"
current="${current##*/}"
printf 'original_version=%s\n' "$current" > "$evidence/versions.env"
printf 'tested_version=%s\n' "$current" >> "$evidence/versions.env"
record_failure() {
  local result=$?
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/report.txt"; fi
}
trap record_failure EXIT
if ! "$brew" outdated --formula "$formula" | /usr/bin/grep -E '^ampcode[[:space:]]' > "$evidence/outdated.txt"; then
  latest="$($brew livecheck "$formula" | /usr/bin/sed -n 's/.*==>[[:space:]]*//p')"
  printf '%s\n' "$latest" > "$evidence/livecheck.txt"
  [[ -n "$latest" && "$latest" != "$current" ]] || {
    printf 'BLOCKED: Amp CLI is not outdated according to Homebrew or livecheck\n' > "$evidence/report.txt"
    exit 2
  }
fi
"$brew" list --versions --formula > "$evidence/packages-before.txt"
readlink "$linked" > "$evidence/link-before.txt"
cd "$HOME/.config/mise"
"$HOME/.local/bin/mise" -E ai-cli bootstrap > "$evidence/apply.log" 2>&1
"$brew" list --versions --formula > "$evidence/packages-after.txt"
cmp "$evidence/packages-before.txt" "$evidence/packages-after.txt"
readlink "$linked" > "$evidence/link-after.txt"
cmp "$evidence/link-before.txt" "$evidence/link-after.txt"
/opt/homebrew/bin/amp --version > "$evidence/health.txt" 2>&1
printf 'outdated_package_preserved=PASS\nno_dependency_changes=PASS\nhealth_check=PASS\n' > "$evidence/report.txt"
trap - EXIT
/bin/cat "$evidence/report.txt"
