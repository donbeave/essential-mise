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
"$brew" outdated --formula "$formula" --json > "$evidence/outdated.json" || true
"$brew" info --formula "$formula" --json > "$evidence/formula-info.json"
if "$brew" outdated --formula "$formula" | /usr/bin/grep -E '^ampcode[[:space:]]' > "$evidence/outdated.txt"; then
  printf 'homebrew_outdated\n' > "$evidence/outdated-proof.txt"
else
  latest="$($brew livecheck "$formula" | /usr/bin/sed -n 's/.*==>[[:space:]]*//p')"
  formula_latest="$(/opt/homebrew/bin/python3 - "$evidence/formula-info.json" <<'PY'
import json
import sys
with open(sys.argv[1]) as source:
    data = json.load(source)
print(data[0]['versions']['stable'])
PY
)"
  printf '%s\n' "$latest" > "$evidence/livecheck.txt"
  printf '%s\n' "$formula_latest" > "$evidence/formula-version.txt"
  [[ -n "$latest" && -n "$formula_latest" && "$latest" != "$current" && "$formula_latest" != "$current" ]] || {
    printf 'BLOCKED: Amp CLI is not outdated according to Homebrew, livecheck, and current formula metadata\n' > "$evidence/report.txt"
    exit 2
  }
  printf 'livecheck_and_formula_metadata\n' > "$evidence/outdated-proof.txt"
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
