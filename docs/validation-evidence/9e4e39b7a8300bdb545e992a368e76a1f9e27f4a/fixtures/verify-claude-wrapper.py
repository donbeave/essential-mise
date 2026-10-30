#!/usr/bin/python3
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess

root = pathlib.Path('/tmp/essential-mise-claude-wrapper')
evidence = pathlib.Path('/tmp/essential-mise-evidence/claude-wrapper')
shutil.rmtree(root, ignore_errors=True)
shutil.rmtree(evidence, ignore_errors=True)
evidence.mkdir(parents=True)
home = root / 'home'
home.mkdir(parents=True)
source = pathlib.Path.home() / '.config/mise/bin/claude-statusline.sh'
config = root / 'xdg space "double" \'single\' $dollar \\slash' / 'mise'
script = config / 'bin/claude-statusline.sh'
script.parent.mkdir(parents=True)
shutil.copy2(source, script)
script.chmod(script.stat().st_mode | stat.S_IXUSR)

zshrc = (pathlib.Path.home() / '.zshrc').read_text()
match = re.search(r'^function claude \{\n(?:  .*\n)+^\}$', zshrc, re.MULTILINE)
assert match, 'Rendered Claude wrapper was not found in ~/.zshrc'
wrapper = match.group(0) + '\n'
(evidence / 'wrapper.zsh').write_text(wrapper)
assert 'printf -v statusline_command' in wrapper
assert 'jq -cn --arg command' in wrapper

capture = root / 'captured.json'
mock_bin = root / 'mock bin'
mock_bin.mkdir()
mock = mock_bin / 'claude'
mock.write_text(f'''#!/usr/bin/python3
import json
import sys
with open({str(capture)!r}, "w") as output:
    json.dump(sys.argv[1:], output)
''')
mock.chmod(0o755)
harness = root / 'harness.zsh'
startup_marker = root / 'startup-reached.txt'
alias_capture = root / 'existing-alias.txt'
harness.write_text(
    "alias claude='claude --permission-mode plan'\n"
    + wrapper
    + f"\nprint -r -- \"$aliases[claude]\" > {str(alias_capture)!r}\n"
    + f"print startup-ok > {str(startup_marker)!r}\n"
    + 'claude --version "argument with spaces"\n'
)
env = {
    **os.environ,
    'HOME': str(home),
    'PATH': f'{mock_bin}:/opt/homebrew/bin:/usr/bin:/bin',
    'XDG_CONFIG_HOME': str(config.parent),
}
env.pop('MISE_CONFIG_DIR', None)
result = subprocess.run(
    ['/bin/zsh', '-f', str(harness)],
    env=env,
    capture_output=True,
    text=True,
)
(evidence / 'wrapper.stdout').write_text(result.stdout)
(evidence / 'wrapper.stderr').write_text(result.stderr)
(evidence / 'wrapper.exit').write_text(f'{result.returncode}\n')
result.check_returncode()
assert startup_marker.read_text().strip() == 'startup-ok'
assert alias_capture.read_text().strip() == 'claude --permission-mode plan'
(evidence / 'existing-alias.txt').write_text(alias_capture.read_text())
arguments = json.loads(capture.read_text())
(evidence / 'arguments.json').write_text(json.dumps(arguments, indent=2) + '\n')
assert arguments[-4:] == ['--permission-mode', 'plan', '--version', 'argument with spaces']
assert arguments[0] == '--settings' and len(arguments) == 6
settings = json.loads(arguments[1])
assert settings['statusLine']['type'] == 'command'
assert settings['statusLine']['padding'] == 0
assert settings['statusLine']['refreshInterval'] == 5
(evidence / 'settings.json').write_text(json.dumps(settings, indent=2) + '\n')

payload = {
    'workspace': {'current_dir': str(home)},
    'model': {'display_name': 'Synthetic'},
    'effort': {'level': 'low'},
    'context_window': {
        'used_percentage': 35,
        'context_window_size': 1000,
        'current_usage': {'input_tokens': 350},
    },
    'transcript_path': str(root / 'none.jsonl'),
}
rendered = subprocess.run(
    settings['statusLine']['command'],
    shell=True,
    executable='/bin/sh',
    env={**env, 'BASH_COMPAT': '3.2'},
    input=json.dumps(payload),
    capture_output=True,
    text=True,
)
(evidence / 'rendered.stdout').write_text(rendered.stdout)
(evidence / 'rendered.stderr').write_text(rendered.stderr)
(evidence / 'rendered.exit').write_text(f'{rendered.returncode}\n')
rendered.check_returncode()
plain = re.sub(r'\x1b\[[0-9;]*m', '', rendered.stdout)
assert plain == '~ │ Synthetic low │ 35% (350/1k)', plain
report = (
    f'script_sha256={hashlib.sha256(script.read_bytes()).hexdigest()}\n'
    'special_character_xdg_path=PASS\njson_encoding=PASS\nshell_command_quoting=PASS\n'
    'existing_alias_coexistence=PASS\nlater_startup_reached=PASS\n'
    'argument_forwarding=PASS\nrendered_command=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
