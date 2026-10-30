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
alternate=''
for directory in "$($brew --cellar "$formula")"/*; do
  [[ -d "$directory" ]] || continue
  version="${directory##*/}"
  if [[ "$version" != "$current" ]]; then
    alternate="$version"
    break
  fi
done
[[ -n "$alternate" ]] || {
  printf 'BLOCKED: no previously installed Amp CLI version is available for the outdated-package fixture\n' > "$evidence/report.txt"
  exit 2
}
printf 'outdated_version=%s\n' "$alternate" >> "$evidence/versions.env"
restore() {
  local result=$?
  "$brew" switch "$formula" "$current" >/dev/null 2>&1 || true
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/report.txt"; fi
}
trap restore EXIT
"$brew" switch "$formula" "$alternate"
"$brew" outdated --formula | /usr/bin/grep -E '^ampcode[[:space:]]' > "$evidence/outdated.txt"
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
