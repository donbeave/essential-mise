#!/usr/bin/python3
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import time

script = pathlib.Path.home() / '.config/mise/bin/claude-statusline.sh'
root = pathlib.Path('/tmp/essential-mise-claude-statusline')
evidence = pathlib.Path('/tmp/essential-mise-evidence/claude-statusline')
shutil.rmtree(root, ignore_errors=True)
evidence.mkdir(parents=True, exist_ok=True)
home = root / 'home'
agent_definition = home / '.claude/agents/ca-explorer.md'
agent_definition.parent.mkdir(parents=True)
agent_definition.write_text('---\neffort: low\n---\n')
transcript = root / 'session.jsonl'
transcript.touch()
subagents = root / 'session/subagents'
subagents.mkdir(parents=True)
assert script.stat().st_mode & stat.S_IXUSR


def assistant_entry():
    return json.dumps({
        'type': 'assistant',
        'message': {
            'model': 'claude-sonnet-5-5',
            'usage': {
                'input_tokens': 90000,
                'cache_read_input_tokens': 1000,
                'cache_creation_input_tokens': 0,
                'output_tokens': 9999,
            },
        },
    }, separators=(',', ':')) + '\n'


def agent(name, finished=False, interrupted=False, idle=False):
    (subagents / f'agent-{name}.meta.json').write_text(json.dumps({
        'agentType': f'ca-{name}',
        'description': 'synthetic test agent',
        'finished': finished,
    }))
    log = subagents / f'agent-{name}.jsonl'
    log.write_text('[Request interrupted by user\n' if interrupted else assistant_entry())
    if idle:
        modified = time.time() - 601
        os.utime(log, (modified, modified))


agent('explorer')
agent('finished', finished=True)
agent('interrupted', interrupted=True)
agent('idle', idle=True)
expected = (
    '~/Projects/ChainArgos/redesign-review │ Opus 5.5 medium │ 35% (345k/1.0M)\n'
    '↳ explorer │ sonnet low │ 91k'
)
(evidence / 'expected.txt').write_text(expected)
for percentage, color in ((35, 32), (50, 33), (80, 31)):
    payload = {
        'workspace': {'current_dir': str(home / 'Projects/ChainArgos/redesign-review')},
        'model': {'display_name': 'Opus 5.5'},
        'effort': {'level': 'medium'},
        'context_window': {
            'used_percentage': percentage,
            'context_window_size': 1000000,
            'current_usage': {
                'input_tokens': 100000,
                'cache_read_input_tokens': 200000,
                'cache_creation_input_tokens': 45000,
                'output_tokens': 123456,
            },
        },
        'transcript_path': str(transcript),
    }
    (evidence / f'payload-{percentage}.json').write_text(json.dumps(payload, indent=2) + '\n')
    with (evidence / f'payload-{percentage}.json').open() as source:
        result = subprocess.run(
            ['/opt/homebrew/bin/bash', str(script)],
            stdin=source,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={**os.environ, 'HOME': str(home), 'BASH_COMPAT': '3.2'},
            check=True,
        )
    raw = result.stdout.decode()
    (evidence / f'output-{percentage}.txt').write_text(raw)
    plain = re.sub(r'\x1b\[[0-9;]*m', '', raw)
    assert f'\x1b[{color}m{percentage}%' in raw
    assert 'finished' not in plain and 'interrupted' not in plain and 'idle' not in plain
    if percentage == 35:
        assert plain == expected
    (evidence / f'output-{percentage}.exit').write_text('0\n')
(evidence / 'actual.txt').write_text(re.sub(r'\x1b\[[0-9;]*m', '', (evidence / 'output-35.txt').read_text()))
report = (
    f'script_sha256={hashlib.sha256(script.read_bytes()).hexdigest()}\n'
    'sample_output=PASS\nrunning_subagent=PASS\nfinished_interrupted_idle_filtering=PASS\n'
    'green_35=PASS\nyellow_50=PASS\nred_80=PASS\ninput_token_formula=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
