#!/usr/bin/env python3
"""Run in the disposable macOS guest with Homebrew jq installed."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MOCK = r'''#!/usr/bin/python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ['MOCK_LOG'], 'a') as f:
    f.write(json.dumps([name] + args) + '\n')
if os.environ.get('MOCK_FAIL') == name + ' ' + ' '.join(args):
    sys.exit(9)
state = pathlib.Path(os.environ['MOCK_STATE'])
if name == 'claude' and args == ['plugin', 'list', '--json']:
    print(state.joinpath('claude-plugins.json').read_text())
elif name == 'claude' and args == ['plugin', 'marketplace', 'list', '--json']:
    print(state.joinpath('claude.json').read_text())
elif name == 'claude' and args[:3] == ['plugin', 'marketplace', 'add']:
    repo = args[3]
    mp = 'pstack-claude' if repo.startswith('michael-denyer/') else 'i-have-adhd'
    state.joinpath('claude.json').write_text(json.dumps([{'name': mp, 'source':'github', 'repo':repo}]))
elif name == 'claude' and args[:2] == ['plugin', 'install']:
    state.joinpath('claude-plugins.json').write_text(json.dumps([{'id':args[2], 'scope':'user', 'enabled':True}]))
elif name == 'omp' and args == ['plugin', 'list', '--json']:
    print(state.joinpath('omp.json').read_text())
elif name == 'omp' and args == ['plugin', 'marketplace', 'list']:
    print(state.joinpath('omp-marketplaces.txt').read_text())
elif name == 'omp' and args[:3] == ['plugin', 'marketplace', 'add']:
    state.joinpath('omp-marketplaces.txt').write_text('i-have-adhd ' + args[3] + '\n')
elif name == 'codex' and args[:2] == ['plugin', 'list']:
    print(state.joinpath('codex.json').read_text())
'''

class PluginLifecycle(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='plugin-lifecycle-')
        self.root = Path(self.temp.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        mock = self.bin / 'mock'
        mock.write_text(MOCK)
        mock.chmod(0o755)
        for name in ('claude', 'codex', 'omp', 'muse'):
            (self.bin / name).symlink_to(mock)
        (self.bin / 'jq').symlink_to('/opt/homebrew/bin/jq')
        self.state = self.root / 'state'
        self.state.mkdir()
        (self.state / 'claude.json').write_text('[]')
        (self.state / 'claude-plugins.json').write_text('[]')
        (self.state / 'codex.json').write_text('{"installed": []}')
        (self.state / 'omp.json').write_text('{"npm":[],"marketplace":[]}')
        (self.state / 'omp-marketplaces.txt').write_text('No marketplaces configured\n')
        self.home = self.root / 'home'
        (self.home / '.local/share/pstack-claude/plugins/pstack/skills').mkdir(parents=True)
        self.log = self.root / 'calls.jsonl'
        self.env = os.environ.copy()
        self.env.update(HOME=str(self.home), MOCK_LOG=str(self.log), MOCK_STATE=str(self.state))
        self.scripts = {}
        for name in ('pstack', 'adhd'):
            script = self.root / (name + '.sh')
            text = (ROOT / 'bin' / (name + '-plugins.sh')).read_text()
            script.write_text(text.replace('bin=/opt/homebrew/bin', 'bin=' + str(self.bin), 1))
            self.scripts[name] = script

    def tearDown(self):
        self.temp.cleanup()

    def run_script(self, script, *args, fail=None):
        env = self.env.copy()
        if fail:
            env['MOCK_FAIL'] = fail
        return subprocess.run(['/bin/bash', str(self.scripts[script]), *args], env=env,
                              text=True, capture_output=True)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_claude_fresh_and_repeat_do_not_readd(self):
        self.assertEqual(self.run_script('pstack', 'claude').returncode, 0)
        self.assertEqual(self.run_script('pstack', 'claude').returncode, 0)
        self.assertEqual(sum(c[:4] == ['claude','plugin','marketplace','add'] for c in self.calls()), 1)
        self.assertIn(['claude','plugin','update','pstack@pstack-claude','--scope','user'], self.calls())

    def test_claude_collision_is_not_mutated(self):
        (self.state / 'claude.json').write_text(json.dumps([{'name':'pstack-claude','source':'github','repo':'other/repo'}]))
        result = self.run_script('pstack', 'claude')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [['claude','plugin','list','--json'], ['claude','plugin','marketplace','list','--json']])

    def test_claude_disabled_is_not_reenabled_or_updated(self):
        for script, plugin in [('pstack', 'pstack@pstack-claude'), ('adhd', 'i-have-adhd@i-have-adhd')]:
            self.log.unlink(missing_ok=True)
            record = {'id': plugin, 'scope': 'user', 'enabled': False}
            (self.state / 'claude-plugins.json').write_text(json.dumps([record]))
            result = self.run_script(script, 'claude')
            self.assertEqual(result.returncode, 0)
            self.assertIn('user disabled', result.stderr)
            self.assertEqual(self.calls(), [['claude','plugin','list','--json']])
            self.assertEqual(json.loads((self.state / 'claude-plugins.json').read_text()), [record])

    def test_claude_malformed_listing_has_no_mutation(self):
        (self.state / 'claude-plugins.json').write_text('[null]')
        self.assertNotEqual(self.run_script('pstack', 'claude').returncode, 0)
        self.assertEqual(self.calls(), [['claude','plugin','list','--json']])

    def test_codex_malformed_member_never_adds_or_upgrades_plugin(self):
        for malformed in [None, {'name': 3}, {'name': 'pstack'}]:
            self.log.unlink(missing_ok=True)
            (self.state / 'codex.json').write_text(json.dumps({'installed':[malformed]}))
            self.assertNotEqual(self.run_script('pstack', 'codex').returncode, 0)
            self.assertFalse(any(c[:3] in (['codex','plugin','add'], ['codex','plugin','marketplace']) and
                                 ('upgrade' in c or 'pstack@pstack-claude' in c) for c in self.calls()))

    def test_omp_existing_user_or_project_installation_is_left_untouched(self):
        for scope in ('user', 'project'):
            self.log.unlink(missing_ok=True)
            entry = {'id':'i-have-adhd@i-have-adhd','scope':scope,
                     'entries':[{'scope':scope,'installPath':'/synthetic/cache','version':'0.4.1'}]}
            (self.state / 'omp.json').write_text(json.dumps({'npm':[], 'marketplace':[entry]}))
            result = self.run_script('adhd','omp')
            self.assertEqual(result.returncode, 0)
            self.assertIn('PENDING i-have-adhd/OMP', result.stderr)
            self.assertEqual(self.calls(), [])
            self.assertEqual(json.loads((self.state / 'omp.json').read_text()), {'npm':[], 'marketplace':[entry]})

    def test_adhd_omp_fresh_install_is_pending_without_mutation(self):
        result = self.run_script('adhd', 'omp')
        self.assertEqual(result.returncode, 0)
        self.assertIn('PENDING i-have-adhd/OMP', result.stderr)
        self.assertEqual(self.calls(), [])

    def test_pstack_omp_pending_preserves_all_installations(self):
        existing = {'npm':[{'name':'pstack','path':'/tmp/node_modules/pstack'}],
                    'marketplace':[{'id':'pstack@other','scope':'user'}]}
        (self.state / 'omp.json').write_text(json.dumps(existing))
        result = self.run_script('pstack','omp')
        self.assertEqual(result.returncode, 0)
        self.assertIn('PENDING pstack/OMP', result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual(json.loads((self.state / 'omp.json').read_text()), existing)

    def test_pstack_opencode_pending_does_not_remove_links(self):
        skills = self.home / '.config/opencode/skills'
        skills.mkdir(parents=True)
        link = skills / 'existing'
        link.symlink_to(self.home / '.local/share/pstack-claude/plugins/pstack/skills/legacy')
        result = self.run_script('pstack','opencode')
        self.assertEqual(result.returncode, 0)
        self.assertIn('PENDING pstack/OpenCode', result.stderr)
        self.assertTrue(link.is_symlink())
        self.assertEqual(self.calls(), [])

    def test_failure_does_not_skip_later_agent_and_retry(self):
        failure = 'claude plugin marketplace list --json'
        self.assertNotEqual(self.run_script('pstack','claude','muse',fail=failure).returncode, 0)
        self.assertTrue(any(c[:3] == ['muse','plugins','install'] for c in self.calls()))
        self.assertEqual(self.run_script('pstack','claude','muse').returncode, 0)

    def test_failure_stops_current_agent_before_following_operations(self):
        failure = 'claude plugin marketplace update pstack-claude'
        self.assertNotEqual(self.run_script('pstack','claude','muse',fail=failure).returncode, 0)
        self.assertFalse(any(c[:3] == ['claude','plugin','install'] for c in self.calls()))
        self.assertTrue(any(c[:3] == ['muse','plugins','install'] for c in self.calls()))

    def test_unknown_tool_fails_before_any_mutation(self):
        self.assertNotEqual(self.run_script('pstack','claude','unknown').returncode, 0)
        self.assertEqual(self.calls(), [])

    def test_opencode_is_pending_and_existing_state_is_untouched(self):
        plugins = self.home / '.config/opencode/plugins'
        plugins.mkdir(parents=True)
        user_file = plugins / 'my-plugin.js'
        user_file.write_text('user-owned')
        prior_loader = plugins / 'i-have-adhd.js'
        prior_loader.write_text('existing loader')
        result = self.run_script('adhd', 'opencode')
        self.assertEqual(result.returncode, 0)
        self.assertIn('PENDING i-have-adhd/OpenCode', result.stderr)
        self.assertEqual(user_file.read_text(), 'user-owned')
        self.assertEqual(prior_loader.read_text(), 'existing loader')
        self.assertEqual(sorted(p.name for p in plugins.iterdir()), ['i-have-adhd.js', 'my-plugin.js'])
        self.assertEqual(self.calls(), [])

    def test_post_repo_runner_attempts_both_and_preserves_failure(self):
        runner_dir = self.root / 'runner'
        runner_dir.mkdir()
        shutil.copy2(ROOT / 'bin' / 'agent-plugins.sh', runner_dir / 'agent-plugins.sh')
        for name, code in [('pstack', 7), ('adhd', 0)]:
            script = runner_dir / (name + '-plugins.sh')
            script.write_text('#!/bin/bash\nprintf "%s\\n" ' + name + ' >> "$MOCK_LOG"\nexit ' + str(code) + '\n')
        result = subprocess.run(['/bin/bash', str(runner_dir / 'agent-plugins.sh')], env=self.env,
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.log.read_text().splitlines(), ['pstack', 'adhd'])
        self.assertIn('pstack-plugins.sh failed', result.stderr)
        (runner_dir / 'pstack-plugins.sh').write_text('#!/bin/bash\nprintf "pstack\\n" >> "$MOCK_LOG"\n')
        self.assertEqual(subprocess.run(['/bin/bash', str(runner_dir / 'agent-plugins.sh')],
                                        env=self.env, capture_output=True).returncode, 0)
        self.assertEqual(self.log.read_text().splitlines(), ['pstack', 'adhd', 'pstack', 'adhd'])

if __name__ == '__main__':
    unittest.main(verbosity=2)
