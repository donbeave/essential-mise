#!/bin/bash
set -euo pipefail

app=/Applications/Amp.app
binary="$app/Contents/MacOS/Amp"
evidence=/tmp/essential-mise-evidence/amp-startup
mount_point=''
rm -rf "$evidence"
mkdir -p "$evidence"
printf 'IN_PROGRESS\n' > "$evidence/result.txt"
record_result() {
  local result=$?
  if [[ -n "$mount_point" ]]; then hdiutil detach "$mount_point" || true; fi
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/result.txt"; fi
}
trap record_result EXIT
cp "$0" "$evidence/diagnose-amp-startup.sh"
shasum -a 256 "$0" > "$evidence/fixture.sha256"
sw_vers > "$evidence/guest-os.txt"
uname -m > "$evidence/architecture.txt"
if [[ -x "$HOME/.local/bin/mise" ]]; then "$HOME/.local/bin/mise" --version > "$evidence/mise-version.txt"; fi
/opt/homebrew/bin/cua-driver --version > "$evidence/cua-version.txt"
/opt/homebrew/bin/brew list --cask amp@beta > "$evidence/cask-files.txt"
/opt/homebrew/bin/brew list --cask --versions amp@beta > "$evidence/cask-version.txt"
file "$binary" > "$evidence/binary-architecture.txt"
shasum -a 256 "$binary" > "$evidence/installed-binary.sha256"
/usr/bin/codesign --verify --deep --strict "$app" > "$evidence/codesign.log" 2>&1
/usr/bin/codesign -dv --verbose=4 "$app" > "$evidence/codesign-identity.log" 2>&1
/usr/sbin/spctl -a -vv "$app" > "$evidence/gatekeeper.log" 2>&1
/opt/homebrew/bin/brew fetch --cask ampcode/tap/amp@beta > "$evidence/official-fetch.log" 2>&1
cache="$(/opt/homebrew/bin/brew --cache --cask amp@beta)"
test -f "$cache"
hdiutil attach "$cache" -nobrowse -readonly -plist > "$evidence/dmg-attach.plist"
mount_point="$(/usr/bin/python3 -c 'import plistlib, sys; entities = plistlib.load(open(sys.argv[1], "rb"))["system-entities"]; print(next(entity["mount-point"] for entity in entities if "mount-point" in entity))' "$evidence/dmg-attach.plist")"
cmp "$binary" "$mount_point/Amp.app/Contents/MacOS/Amp"
shasum -a 256 "$cache" > "$evidence/official-dmg.sha256"
printf 'installed executable is byte-identical to the official DMG\n' > "$evidence/official-comparison.txt"
hdiutil detach "$mount_point"
mount_point=''
/usr/bin/open -a Amp
sleep 12
pid="$(pgrep -x Amp | head -n 1)"
printf 'pid=%s\n' "$pid" > "$evidence/process.env"
ps -p "$pid" -o pid,ppid,state,%cpu,etime,comm > "$evidence/process.txt"
sample "$pid" 3 1 -file "$evidence/process-sample.txt" > "$evidence/sample-command.log" 2>&1
/opt/homebrew/bin/cua-driver call list_windows '{}' > "$evidence/windows.json"
/usr/bin/python3 -c 'import json, sys; windows = json.load(open(sys.argv[1]))["windows"]; matches = [window for window in windows if window["pid"] == int(sys.argv[2])]; assert not matches, matches' "$evidence/windows.json" "$pid"
log show --last 5m --style compact --predicate 'process == "Amp"' > "$evidence/process-log.txt" 2>&1 || true
printf 'BLOCKED: Amp remains running without a window after launch; inspect process-sample.txt\n' > "$evidence/result.txt"
kill "$pid" || true
/bin/cat "$evidence/result.txt"
