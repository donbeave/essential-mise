#!/usr/bin/python3
"""Reject Codex project configuration before user-scoped plugin operations."""
import json
import os
import select
import subprocess
import sys
import time


process = None
try:
    process = subprocess.Popen(['/opt/homebrew/bin/codex', 'app-server'], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + 20
    pending = b''
    os.set_blocking(process.stdin.fileno(), False)
    def send(payload):
        data = memoryview(payload)
        while data:
            timeout = deadline - time.monotonic()
            if timeout <= 0 or not select.select([], [process.stdin], [], timeout)[1]:
                raise RuntimeError('API write timed out')
            data = data[os.write(process.stdin.fileno(), data):]
    def request(request_id, method, params):
        global pending
        send((json.dumps({'id': request_id, 'method': method, 'params': params}) + '\n').encode())
        while True:
            while b'\n' in pending:
                line, pending = pending.split(b'\n', 1)
                item = json.loads(line)
                if item.get('id') == request_id:
                    if 'error' in item or 'result' not in item:
                        raise RuntimeError('API request failed')
                    return item['result']
            timeout = deadline - time.monotonic()
            if timeout <= 0 or not select.select([process.stdout], [], [], timeout)[0]:
                raise RuntimeError('API request timed out')
            data = os.read(process.stdout.fileno(), 65536)
            if not data:
                raise RuntimeError('API closed')
            pending += data
            if len(pending) > 8 * 1024 * 1024:
                raise RuntimeError('API response too large')
    request(1, 'initialize', {'clientInfo': {'name': 'essential-mise', 'version': '1'}})
    send(b'{"method":"initialized"}\n')
    response = request(2, 'config/read', {'includeLayers': True, 'cwd': os.getcwd()})
    layers = response['layers']
    if not isinstance(layers, list):
        raise RuntimeError('invalid layers')
    for layer in layers:
        if not isinstance(layer, dict) or not isinstance(layer.get('name'), dict):
            raise RuntimeError('invalid layer')
        kind = layer['name'].get('type')
        if not isinstance(kind, str):
            raise RuntimeError('invalid layer kind')
        if kind not in {'mdm', 'system', 'enterpriseManaged', 'user', 'project', 'sessionFlags', 'legacyManagedConfigTomlFromFile', 'legacyManagedConfigTomlFromMdm'}:
            raise RuntimeError('unknown layer kind')
        if kind == 'project':
            print('project')
            break
    else:
        print('clear')

except Exception:
    print('Codex project configuration could not be inspected', file=sys.stderr)
    sys.exit(1)
finally:
    if process is not None:
        try:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=3)
        except ProcessLookupError:
            pass
        except subprocess.TimeoutExpired:
            try:
                process.kill()
                process.wait(timeout=3)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                pass
