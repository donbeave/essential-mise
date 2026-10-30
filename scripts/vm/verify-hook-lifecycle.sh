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
alternate="$current"
for directory in "$($brew --cellar "$formula")"/*; do
  [[ -d "$directory" ]] || continue
  version="${directory##*/}"
  if [[ "$version" != "$current" ]]; then
    alternate="$version"
    break
  fi
done
printf 'tested_version=%s\n' "$alternate" >> "$evidence/versions.env"
restore() {
  local result=$?
  "$brew" switch "$formula" "$current" >/dev/null 2>&1 || true
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/report.txt"; fi
}
trap restore EXIT
if [[ "$alternate" != "$current" ]]; then "$brew" switch "$formula" "$alternate"; fi
if ! "$brew" outdated --formula "$formula" | /usr/bin/grep -E '^ampcode[[:space:]]' > "$evidence/outdated.txt"; then
  latest="$($brew livecheck "$formula" | /usr/bin/sed -n 's/.*==>[[:space:]]*//p')"
  printf '%s\n' "$latest" > "$evidence/livecheck.txt"
  [[ -n "$latest" && "$latest" != "$alternate" ]] || {
    printf 'BLOCKED: Amp CLI is not outdated according to Homebrew or livecheck\n' > "$evidence/report.txt"
    exit 2
  }
fi
"$brew" list --versions > "$evidence/packages-before.txt"
readlink "$linked" > "$evidence/link-before.txt"
cd "$HOME/.config/mise"
"$HOME/.local/bin/mise" -E ai-cli bootstrap > "$evidence/apply.log" 2>&1
"$brew" list --versions > "$evidence/packages-after.txt"
cmp "$evidence/packages-before.txt" "$evidence/packages-after.txt"
readlink "$linked" > "$evidence/link-after.txt"
cmp "$evidence/link-before.txt" "$evidence/link-after.txt"
/opt/homebrew/bin/amp --version > "$evidence/health.txt" 2>&1
printf 'outdated_package_preserved=PASS\nno_dependency_changes=PASS\nhealth_check=PASS\n' > "$evidence/report.txt"
"$brew" switch "$formula" "$current"
trap - EXIT
/bin/cat "$evidence/report.txt"
