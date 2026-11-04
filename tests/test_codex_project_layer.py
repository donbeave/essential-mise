#!/usr/bin/env python3
"""Exercise the Codex configuration preflight with a synthetic app-server."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
FAKE_SERVER = r'''#!/usr/bin/python3
import json, os, sys
user = {'name': {'type': 'user', 'file': '/tmp/config.toml', 'profile': None}}
cases = {
    'clear': [user],
    'project': [user, {'name': {'type': 'project'}}],
    'empty': [],
    'missing-user': [{'name': {'type': 'system'}}],
    'duplicate-user': [user, user],
    'malformed-after-project': [user, {'name': {'type': 'project'}}, {}],
    'relative-user': [{'name': {'type': 'user', 'file': 'relative', 'profile': None}}],
    'unknown-layer': [user, {'name': {'type': 'future'}}],
}
for line in sys.stdin:
    message = json.loads(line)
    if 'id' not in message:
        continue
    request_id = message['id']
    if os.environ['CASE'] == 'non-integer-id' and request_id == 2:
        print(json.dumps({'id': 2.0, 'result': {'layers': [user]}}), flush=True)
        break
    result = {} if request_id == 1 else {'layers': cases[os.environ['CASE']]}
    print(json.dumps({'id': request_id, 'result': result}), flush=True)
'''


class CodexProjectLayer(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='codex-layer-test-')
        root = Path(self.temp.name)
        fake = root / 'fake-server'
        fake.write_text(FAKE_SERVER)
        fake.chmod(0o755)
        helper = (ROOT / 'bin/codex-project-layer.py').read_text()
        needle = "'/opt/homebrew/bin/codex', 'app-server'"
        self.assertEqual(helper.count(needle), 1)
        self.helper = root / 'guard.py'
        self.helper.write_text(helper.replace(needle, repr(str(fake)) + ", 'app-server'"))

    def tearDown(self):
        self.temp.cleanup()

    def test_layer_responses_fail_closed(self):
        cases = {
            'clear': (0, 'clear'),
            'project': (0, 'project'),
            'empty': (1, ''),
            'missing-user': (1, ''),
            'duplicate-user': (1, ''),
            'malformed-after-project': (1, ''),
            'relative-user': (1, ''),
            'unknown-layer': (1, ''),
            'non-integer-id': (1, ''),
        }
        for case, expected in cases.items():
            with self.subTest(case=case):
                result = subprocess.run(['/usr/bin/python3', str(self.helper)],
                                        env=dict(os.environ, CASE=case),
                                        text=True, capture_output=True, timeout=30)
                self.assertEqual((result.returncode, result.stdout.strip()), expected)


if __name__ == '__main__':
    unittest.main(verbosity=2)
