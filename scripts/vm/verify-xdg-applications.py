#!/usr/bin/python3
import http.server
import json
import os
import pathlib
import shutil
import subprocess
import sys
import threading
import time

config_dir = pathlib.Path.home() / '.config/mise'
mise = pathlib.Path.home() / '.local/bin/mise'
ghostty = pathlib.Path('/Applications/Ghostty.app/Contents/MacOS/ghostty')
fish = pathlib.Path('/opt/homebrew/bin/fish')
opencode = pathlib.Path('/opt/homebrew/bin/opencode')
root = pathlib.Path('/tmp/essential-mise-xdg-applications')
evidence = pathlib.Path('/tmp/essential-mise-evidence/xdg-applications')
shutil.rmtree(root, ignore_errors=True)
shutil.rmtree(evidence, ignore_errors=True)
evidence.mkdir(parents=True)
(evidence / 'report.txt').write_text('IN_PROGRESS\n')


def record_failure(exception_type, exception, traceback):
    (evidence / 'report.txt').write_text(f'FAIL\n{exception_type.__name__}: {exception}\n')


sys.excepthook = record_failure
home = root / 'home'
default_home = root / 'default home'
xdg = root / 'custom xdg space'
opencode_work = root / 'opencode-work'
home.mkdir(parents=True)
default_home.mkdir(parents=True)
opencode_work.mkdir()
(xdg / 'gh').mkdir(parents=True)
(xdg / 'fish').mkdir(parents=True)
(xdg / 'gh/config.yml').write_text('''version: 1
git_protocol: ssh
aliases:
  co: old checkout
  useralias: gist list
custom_key: preserve
''')
(xdg / 'fish/config.fish').write_text('# synthetic user setting\nset -g SYNTHETIC_FISH_VALUE preserve\n')
base_env = {
    **os.environ,
    'HOME': str(home),
    'XDG_CONFIG_HOME': str(xdg),
    'MISE_CONFIG_DIR': str(config_dir),
    'PATH': f'{mise.parent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'GIT_CONFIG_NOSYSTEM': '1',
    'OPENCODE_DISABLE_AUTOUPDATE': '1',
    'OPENCODE_DISABLE_MODELS_FETCH': '1',
    'OPENCODE_DISABLE_PROJECT_CONFIG': '1',
    'OPENCODE_DISABLE_CLAUDE_CODE': '1',
    'OPENCODE_DISABLE_CLAUDE_CODE_PROMPT': '1',
    'OPENCODE_DISABLE_CLAUDE_CODE_SKILLS': '1',
}
for variable in ('MISE_ENV', 'GH_CONFIG_DIR', 'OPENCODE_CONFIG', 'OPENCODE_CONFIG_DIR', 'OPENCODE_CONFIG_CONTENT'):
    base_env.pop(variable, None)


def run(name, arguments, env=None, timeout=60, expected=0, cwd=None):
    arguments = [str(argument) for argument in arguments]
    try:
        result = subprocess.run(
            arguments,
            env=env or base_env,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
        )
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout.decode() if isinstance(error.stdout, bytes) else error.stdout or ''
        stderr = error.stderr.decode() if isinstance(error.stderr, bytes) else error.stderr or ''
        (evidence / f'{name}.stdout').write_text(stdout)
        (evidence / f'{name}.stderr').write_text(stderr)
        (evidence / f'{name}.exit').write_text('124\n')
        raise AssertionError(f'{name} timed out after {timeout}s') from error
    (evidence / f'{name}.stdout').write_text(result.stdout)
    (evidence / f'{name}.stderr').write_text(result.stderr)
    (evidence / f'{name}.exit').write_text(f'{result.returncode}\n')
    if result.returncode != expected:
        raise AssertionError(f'{name} exited {result.returncode}, expected {expected}: {result.stderr}')
    return result.stdout


def apply(name, environments, parts, env=None):
    return run(
        name,
        [mise, '-C', config_dir, '-E', environments, 'bootstrap', '--only', parts, '--yes'],
        env,
        timeout=1200,
    )


expected = {
    'git_protocol': 'https',
    'editor': '',
    'prompt': 'enabled',
    'prefer_editor_prompt': 'disabled',
    'pager': '',
    'http_unix_socket': '',
    'browser': '',
    'color_labels': 'disabled',
    'accessible_colors': 'disabled',
    'accessible_prompter': 'disabled',
    'spinner': 'enabled',
    'version': '1',
}


def verify_gh(prefix, env):
    for key, value in expected.items():
        actual = run(f'{prefix}-{key}', ['gh', 'config', 'get', key], env).strip()
        assert actual == value, (key, actual, value)
    aliases = run(f'{prefix}-aliases', ['gh', 'alias', 'list'], env)
    assert 'co:' in aliases and 'pr checkout' in aliases
    assert 'useralias:' in aliases and 'gist list' in aliases


synthetic_requests = []


class SyntheticProvider(http.server.BaseHTTPRequestHandler):
    def send_payload(self, status, body, content_type):
        encoded = body.encode()
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self):
        if self.path == '/v1/models':
            body = json.dumps({
                'object': 'list',
                'data': [{'id': 'synthetic-model', 'object': 'model', 'created': int(time.time()), 'owned_by': 'synthetic'}],
            })
            self.send_payload(200, body, 'application/json')
        else:
            self.send_payload(404, '{}', 'application/json')

    def do_POST(self):
        if self.path != '/v1/chat/completions':
            self.send_payload(404, '{}', 'application/json')
            return
        length = int(self.headers.get('Content-Length', '0'))
        payload = json.loads(self.rfile.read(length))
        synthetic_requests.append({'path': self.path, 'payload': payload})
        created = int(time.time())
        if payload.get('stream'):
            chunks = [
                {
                    'id': 'chatcmpl-synthetic',
                    'object': 'chat.completion.chunk',
                    'created': created,
                    'model': 'synthetic-model',
                    'choices': [{'index': 0, 'delta': {'role': 'assistant', 'content': 'synthetic-ok'}, 'finish_reason': None}],
                },
                {
                    'id': 'chatcmpl-synthetic',
                    'object': 'chat.completion.chunk',
                    'created': created,
                    'model': 'synthetic-model',
                    'choices': [{'index': 0, 'delta': {}, 'finish_reason': 'stop'}],
                    'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2},
                },
            ]
            body = ''.join(f'data: {json.dumps(chunk)}\n\n' for chunk in chunks) + 'data: [DONE]\n\n'
            self.send_payload(200, body, 'text/event-stream')
            return
        body = json.dumps({
            'id': 'chatcmpl-synthetic',
            'object': 'chat.completion',
            'created': created,
            'model': 'synthetic-model',
            'choices': [{'index': 0, 'message': {'role': 'assistant', 'content': 'synthetic-ok'}, 'finish_reason': 'stop'}],
            'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2},
        })
        self.send_payload(200, body, 'application/json')

    def log_message(self, format, *args):
        pass


synthetic_server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), SyntheticProvider)
synthetic_server.daemon_threads = True
synthetic_thread = threading.Thread(target=synthetic_server.serve_forever, daemon=True)
synthetic_thread.start()


def message_texts(message):
    content = message.get('content')
    if isinstance(content, str):
        return [content]
    if isinstance(content, list):
        return [
            part['text'] for part in content
            if isinstance(part, dict) and part.get('type') == 'text' and isinstance(part.get('text'), str)
        ]
    return []


def main_requests(entries, probe):
    expected = {probe, json.dumps(probe)}
    matches = []
    for entry in entries:
        messages = entry['payload'].get('messages')
        if not isinstance(messages, list):
            continue
        for message in messages:
            if not isinstance(message, dict) or message.get('role') != 'user':
                continue
            if expected.intersection(message_texts(message)):
                matches.append(entry)
                break
    return matches


def run_synthetic_prompt(name, env):
    provider = {
        'model': 'synthetic/synthetic-model',
        'small_model': 'synthetic/synthetic-model',
        'provider': {
            'synthetic': {
                'npm': '@ai-sdk/openai-compatible',
                'name': 'Synthetic loopback provider',
                'options': {
                    'baseURL': f'http://127.0.0.1:{synthetic_server.server_port}/v1',
                    'apiKey': 'synthetic-only',
                },
                'models': {'synthetic-model': {'name': 'Synthetic loopback model'}},
            },
        },
    }
    prompt_env = {**env, 'OPENCODE_CONFIG_CONTENT': json.dumps(provider)}
    probe = f'synthetic instruction probe: {name}'
    before = len(synthetic_requests)
    run(
        name,
        [
            opencode,
            'run',
            probe,
            '--model',
            'synthetic/synthetic-model',
            '--title',
            f'essential-mise-{name}',
            '--standalone',
        ],
        prompt_env,
        timeout=180,
        cwd=opencode_work,
    )
    matches = main_requests(synthetic_requests[before:], probe)
    assert matches, synthetic_requests[before:]
    (evidence / f'{name}-main-requests.json').write_text(json.dumps(matches, indent=2) + '\n')
    return [json.dumps(entry['payload']) for entry in matches]


def verify_opencode(prefix, env, target):
    assert target.is_file(), target
    assert target.read_bytes() == (config_dir / 'dotfiles/ai-cli/GLOBAL_AGENTS.md').read_bytes()
    paths = run(f'{prefix}-paths', [opencode, 'debug', 'paths'], env, timeout=30, cwd=opencode_work)
    assert str(target.parent) in paths, paths
    run(f'{prefix}-config', [opencode, 'debug', 'config', '--standalone'], env, timeout=30, cwd=opencode_work)
    requests = run_synthetic_prompt(f'{prefix}-instruction-consumption', env)
    assert all('# Engineering policy' in request for request in requests), requests


def verify_opencode_absent(prefix, env, target):
    assert not target.exists(), target
    requests = run_synthetic_prompt(f'{prefix}-instruction-consumption', env)
    assert all('# Engineering policy' not in request for request in requests), requests


apply('initial-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
apply('second-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
second_apply_output = (evidence / 'second-apply.stdout').read_text() + (evidence / 'second-apply.stderr').read_text()
assert 'Changed alias co' not in second_apply_output
run('install-js', [mise, '-C', config_dir, '-E', 'js', 'bootstrap', '--yes'], timeout=600)
verify_gh('gh', base_env)
gh_text = (xdg / 'gh/config.yml').read_text()
(evidence / 'gh-config.yml').write_text(gh_text)
assert 'custom_key: preserve' in gh_text
assert not (home / '.config/gh/config.yml').exists()

assert (xdg / 'ghostty/config').is_file()
assert (xdg / 'ghostty/themes/generic-terminal').is_file()
assert not (home / '.config/ghostty/config').exists()
show_config = run('ghostty-show-config', [ghostty, '+show-config'], timeout=30)
assert 'font-family' in show_config and 'JetBrainsMono Nerd Font' in show_config
assert 'scrollback-limit' in show_config and '67108864' in show_config
run('ghostty-validate-config', [ghostty, '+validate-config'], timeout=30)

custom_opencode = xdg / 'opencode/AGENTS.md'
verify_opencode('opencode-custom', base_env, custom_opencode)
assert not (home / '.config/opencode/AGENTS.md').exists()
default_env = {**base_env, 'HOME': str(default_home)}
default_env.pop('XDG_CONFIG_HOME', None)
apply('default-home-apply', 'ai-cli', 'dotfiles', default_env)
default_opencode = default_home / '.config/opencode/AGENTS.md'
verify_opencode('opencode-default', default_env, default_opencode)

opencode_override = root / 'opencode override with spaces'
opencode_override.mkdir()
override_target = opencode_override / 'AGENTS.md'
opencode_override_env = {**base_env, 'OPENCODE_CONFIG_DIR': str(opencode_override)}
override_paths = run('opencode-override-paths', [opencode, 'debug', 'paths'], opencode_override_env, timeout=30, cwd=opencode_work)
assert str(opencode_override) in override_paths, override_paths
verify_opencode_absent('opencode-override-unmanaged', opencode_override_env, override_target)
shutil.copy2(config_dir / 'dotfiles/ai-cli/GLOBAL_AGENTS.md', override_target)
verify_opencode('opencode-override', opencode_override_env, override_target)

fish_bin = root / 'fish-bin'
fish_bin.mkdir()
(fish_bin / 'mise').symlink_to(mise)
fish_env = {
    **base_env,
    'MISE_ENV': 'js',
    'PATH': f'{fish_bin}:/usr/bin:/bin:/usr/sbin:/sbin',
}
assert shutil.which('bun', path=fish_env['PATH']) is None
expected_bun = run('expected-bun', [mise, '-C', config_dir, '-E', 'js', 'which', 'bun']).strip()
assert f'{home}/.local/share/mise/installs/bun/' in expected_bun, expected_bun
fish_activation = run('fish-bun-activation', [fish, '-ic', 'command -v bun; and bun --version'], fish_env, timeout=30)
assert expected_bun in fish_activation, fish_activation
assert run('expected-bun-version', [expected_bun, '--version']).strip() in fish_activation, fish_activation

fish_path = xdg / 'fish/config.fish'
original_fish = fish_path.read_text()
neutralized = original_fish.replace(
    '    mise activate fish | source',
    '    # activation neutralized for negative control',
).replace(
    '    mise activate fish --shims | source',
    '    # shims neutralized for negative control',
)
assert neutralized != original_fish
assert neutralized.count('# >>> mise:activate >>>') == 1
try:
    fish_path.write_text(neutralized)
    run('fish-activation-neutralized', [fish, '-ic', 'command -v bun'], fish_env, timeout=30, expected=127)
finally:
    fish_path.write_text(original_fish)
run('fish-activation-restored', ['/usr/bin/python3', config_dir / 'bin/fish-config.py', 'status', 'activate'])

fish_completions = run(
    'fish-amp-completions',
    [fish, '-ic', 'set -g fish_complete_path; complete -c amp'],
    timeout=30,
)
assert fish_completions.strip(), 'Amp managed block produced no Fish completions'
neutralized = original_fish.replace(
    '    command -q amp; and amp completions fish | source',
    '    # Amp completion neutralized for negative control',
)
assert neutralized != original_fish
assert neutralized.count('# >>> mise:amp >>>') == 1
try:
    fish_path.write_text(neutralized)
    without_amp = run(
        'fish-amp-neutralized',
        [fish, '-ic', 'set -g fish_complete_path; complete -c amp'],
        timeout=30,
    )
    assert not without_amp.strip(), without_amp
finally:
    fish_path.write_text(original_fish)
run('fish-amp-restored', ['/usr/bin/python3', config_dir / 'bin/fish-config.py', 'status', 'amp'])

fish_text = fish_path.read_text()
(evidence / 'config.fish').write_text(fish_text)
assert '# synthetic user setting' in fish_text
assert 'set -g SYNTHETIC_FISH_VALUE preserve' in fish_text
assert fish_text.count('# >>> mise:activate >>>') == 1
assert fish_text.count('# >>> mise:amp >>>') == 1
assert not (home / '.config/fish/config.fish').exists()

override = root / 'gh override'
override.mkdir()
(override / 'config.yml').write_text('''version: 1
aliases:
  useralias: gist list
custom_override: preserve
''')
override_env = {**base_env, 'GH_CONFIG_DIR': str(override)}
apply('gh-config-dir-apply', 'development', 'packages', override_env)
verify_gh('gh-override', override_env)
assert 'custom_override: preserve' in (override / 'config.yml').read_text()

run('native-terminal-unapply', [mise, '-C', config_dir, 'bootstrap', 'unapply', 'terminal', '--yes'])
assert not (xdg / 'ghostty/config').exists()
assert not (xdg / 'ghostty/themes/generic-terminal').exists()
for name in ('activate', 'amp'):
    run(f'fish-unapply-{name}', ['/usr/bin/python3', config_dir / 'bin/fish-config.py', 'unapply', name])
remaining_fish = fish_path.read_text()
assert '# synthetic user setting' in remaining_fish
assert 'set -g SYNTHETIC_FISH_VALUE preserve' in remaining_fish
assert 'mise:activate' not in remaining_fish and 'mise:amp' not in remaining_fish
assert 'custom_key: preserve' in (xdg / 'gh/config.yml').read_text()

run('native-ai-cli-unapply', [mise, '-C', config_dir, 'bootstrap', 'unapply', 'ai-cli', '--yes'])
assert not custom_opencode.exists()
verify_opencode_absent('opencode-custom-absent', base_env, custom_opencode)
run('native-default-ai-cli-unapply', [mise, '-C', config_dir, 'bootstrap', 'unapply', 'ai-cli', '--yes'], default_env)
assert not default_opencode.exists()
verify_opencode_absent('opencode-default-absent', default_env, default_opencode)
assert override_target.is_file()
verify_opencode('opencode-override-preserved', opencode_override_env, override_target)
synthetic_server.shutdown()
synthetic_server.server_close()
synthetic_thread.join(timeout=5)

report = (
    'custom_xdg_gh_consumption=PASS\nall_gh_scalars_alias_schema=PASS\n'
    'gh_config_dir_override=PASS\ncustom_xdg_ghostty_consumption=PASS\n'
    'ghostty_theme_resolution=PASS\ncustom_xdg_opencode_instructions=PASS\n'
    'default_xdg_opencode_instructions=PASS\nopencode_config_dir_override=PASS\n'
    'opencode_override_preservation=PASS\nopencode_unapply=PASS\n'
    'custom_xdg_fish_runtime_activation=PASS\nfish_activation_negative_control=PASS\n'
    'custom_xdg_fish_amp_completion=PASS\nfish_amp_negative_control=PASS\n'
    'repeat_convergence=PASS\nunrelated_content_preservation=PASS\n'
    'native_ghostty_unapply=PASS\nfish_assisted_unapply=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
