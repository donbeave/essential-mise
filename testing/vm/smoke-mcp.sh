#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
source "$script_dir/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
requests="$(mktemp /tmp/essential-mise-mcp-requests.XXXXXX)"
responses="/tmp/essential-mise-mcp-$VM_NAME.jsonl"
trap '/bin/rm -f "$requests"' EXIT
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"essential-mise-vm-smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_apps","arguments":{}}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_desktop_state","arguments":{}}}' \
  > "$requests"
"$script_dir/connect-mcp.sh" < "$requests" > "$responses"
/usr/bin/python3 - "$responses" "/tmp/essential-mise-mcp-$VM_NAME-desktop" <<'PY'
import base64
import json
import pathlib
import sys

response_path = pathlib.Path(sys.argv[1])
responses = [json.loads(line) for line in response_path.read_text().splitlines() if line.strip()]
by_id = {response.get('id'): response for response in responses if 'id' in response}
for request_id in (1, 2, 3, 4):
    response = by_id.get(request_id)
    if not response or 'error' in response or 'result' not in response:
        raise SystemExit(f'MCP request {request_id} failed: {response}')
server = by_id[1]['result'].get('serverInfo', {})
tools = by_id[2]['result'].get('tools', [])
if not tools:
    raise SystemExit('MCP server returned no tools')
for request_id in (3, 4):
    if by_id[request_id]['result'].get('isError'):
        raise SystemExit(f'MCP tool request {request_id} failed: {by_id[request_id]}')
images = [block for block in by_id[4]['result'].get('content', []) if block.get('type') == 'image']
if not images:
    raise SystemExit('Guest desktop response contained no image')
image = images[0]
suffix = '.png' if image.get('mimeType') == 'image/png' else '.jpg'
image_path = pathlib.Path(sys.argv[2] + suffix)
image_path.write_bytes(base64.b64decode(image['data'], validate=True))
print(f'MCP server: {server.get("name", "unknown")} {server.get("version", "unknown")}')
print(f'MCP tools available: {len(tools)}')
print(f'Guest list_apps and get_desktop_state succeeded over SSH: {response_path}')
print(f'Guest desktop image: {image_path}')
PY
