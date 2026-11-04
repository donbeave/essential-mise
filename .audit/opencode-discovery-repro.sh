#!/bin/bash
set -euo pipefail

opencode=${1:?Pass the absolute path to OpenCode v1.19.5}
test_root=$(mktemp -d "${TMPDIR:-/tmp}/opencode-plugin-discovery.XXXXXX")
trap 'rm -rf "$test_root"' EXIT
export HOME="$test_root/home"
export XDG_CONFIG_HOME="$test_root/config"
export XDG_DATA_HOME="$test_root/data"
export XDG_CACHE_HOME="$test_root/cache"
export XDG_STATE_HOME="$test_root/state"
export PROBE_ROOT="$test_root"
mkdir -p "$HOME" "$XDG_CONFIG_HOME/opencode/plugins" "$test_root/package/skills/probe"
cat > "$test_root/package/skills/probe/SKILL.md" <<'EOF'
---
name: probe
description: A test skill bundled with a plugin.
---
The plugin skill is available.
EOF
cat > "$XDG_CONFIG_HOME/opencode/plugins/probe.js" <<'EOF'
import { writeFileSync } from "node:fs";
export default async function () {
  return {
    config: async (config) => {
      writeFileSync(`${process.env.PROBE_ROOT}/plugin-ran`, "yes");
      config.skills ??= {};
      config.skills.paths ??= [];
      config.skills.paths.push(`${process.env.PROBE_ROOT}/package/skills`);
    },
  };
}
EOF
"$opencode" debug skill > "$test_root/plugin-result.json"
python3 - "$test_root/plugin-result.json" <<'PY'
import json, sys
skills = json.load(open(sys.argv[1]))
print('plugin-registered probe visible:', any(s['name'] == 'probe' for s in skills))
PY
if [[ -e "$test_root/plugin-ran" ]]; then
  printf 'plugin config hook ran: yes\n'
else
  printf 'plugin config hook ran: no\n'
fi
python3 - "$XDG_CONFIG_HOME/opencode/opencode.json" "$test_root/package/skills" <<'PY'
import json, sys
with open(sys.argv[1], 'w') as f:
    json.dump({'skills': {'paths': [sys.argv[2]]}}, f)
PY
"$opencode" debug skill > "$test_root/control-result.json"
python3 - "$test_root/control-result.json" <<'PY'
import json, sys
skills = json.load(open(sys.argv[1]))
assert any(s['name'] == 'probe' for s in skills), 'static path control failed'
print('static path control: probe visible')
PY
