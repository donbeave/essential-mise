#!/usr/bin/python3
import json
import os
import pathlib
import subprocess
import time

vm_name = os.environ['VM_NAME']
caller = pathlib.Path(__file__).resolve().with_name('cua-call.sh')
session = 'ui-smoke'
evidence = '/tmp/essential-mise-evidence/ui-smoke'


def call(tool, arguments):
    arguments = dict(arguments)
    arguments['session'] = session
    result = subprocess.run(
        [str(caller), tool, json.dumps(arguments)],
        text=True,
        capture_output=True,
        timeout=45,
    )
    if result.returncode:
        raise RuntimeError(f'{tool} failed: {result.stdout}{result.stderr}')
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f'{tool} returned non-JSON output: {result.stdout}') from error
    if payload.get('status') == 'refused':
        raise RuntimeError(f'{tool} was refused: {payload}')
    return payload


call('get_desktop_state', {'screenshot_out_file': f'{evidence}/desktop-before.png'})
launched = call('launch_app', {'bundle_id': 'com.apple.calculator'})
pid = launched['pid']
window = None
for _ in range(20):
    windows = call('list_windows', {'pid': pid})['windows']
    candidates = [item for item in windows if item.get('layer') == 0 and item.get('on_current_space')]
    if candidates:
        window = candidates[0]
        break
    time.sleep(0.5)
if window is None:
    raise RuntimeError('Calculator did not expose a usable window')
window_id = window['window_id']
state = call('get_window_state', {
    'pid': pid,
    'window_id': window_id,
    'tree_format': 'elements',
    'screenshot_out_file': f'{evidence}/calculator-initial.png',
})
buttons = {
    element.get('label'): element['element_token']
    for element in state.get('elements', [])
    if element.get('role') == 'AXButton' and element.get('label')
}
for label in ('All Clear', '6', 'Multiply', '7', 'Equals'):
    if label not in buttons:
        raise RuntimeError(f'Calculator button missing from accessibility tree: {label}')
    call('click', {'element_token': buttons[label]})
result = call('get_window_state', {
    'pid': pid,
    'window_id': window_id,
    'query': '42',
    'tree_format': 'markdown',
    'screenshot_out_file': f'{evidence}/calculator-42.png',
})
if 'AXStaticText = "42"' not in result.get('tree_markdown', ''):
    raise RuntimeError(f'Calculator result was not verified: {result}')
print(f'PASS: Calculator 6 x 7 = 42 verified through guest accessibility on {vm_name} (pid {pid}, window {window_id})')
print(f'Guest evidence: {evidence}/calculator-initial.png and {evidence}/calculator-42.png')
