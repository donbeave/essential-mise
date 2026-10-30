#!/usr/bin/python3
import os
import pathlib
import shutil
import subprocess
import sys

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
    return run(name, [mise, '-C', config_dir, '-E', environments, 'bootstrap', '--only', parts, '--yes'], env)


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


def verify_opencode(prefix, env, target):
    assert target.is_file(), target
    assert target.read_bytes() == (config_dir / 'dotfiles/ai-cli/GLOBAL_AGENTS.md').read_bytes()
    paths = run(f'{prefix}-paths', [opencode, 'debug', 'paths'], env, timeout=30, cwd=opencode_work)
    assert str(target.parent) in paths, paths
    run(f'{prefix}-config', [opencode, 'debug', 'config'], env, timeout=30, cwd=opencode_work)
    agent = run(f'{prefix}-agent', [opencode, 'debug', 'agent', 'build'], env, timeout=30, cwd=opencode_work)
    assert '# Engineering policy' in agent, agent


def verify_opencode_absent(prefix, env):
    agent = run(f'{prefix}-agent', [opencode, 'debug', 'agent', 'build'], env, timeout=30, cwd=opencode_work)
    assert '# Engineering policy' not in agent, agent


apply('initial-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
apply('second-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
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
assert f'{home}/.local/share/mise/installs/bun/1.4.4/' in expected_bun, expected_bun
fish_activation = run('fish-bun-activation', [fish, '-ic', 'command -v bun; and bun --version'], fish_env, timeout=30)
assert expected_bun in fish_activation, fish_activation
assert '1.4.4' in fish_activation, fish_activation

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
    run('fish-activation-neutralized', [fish, '-ic', 'command -v bun'], fish_env, timeout=30, expected=1)
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
verify_opencode_absent('opencode-custom-absent', base_env)
run('native-default-ai-cli-unapply', [mise, '-C', config_dir, 'bootstrap', 'unapply', 'ai-cli', '--yes'], default_env)
assert not default_opencode.exists()
verify_opencode_absent('opencode-default-absent', default_env)

report = (
    'custom_xdg_gh_consumption=PASS\nall_gh_scalars_alias_schema=PASS\n'
    'gh_config_dir_override=PASS\ncustom_xdg_ghostty_consumption=PASS\n'
    'ghostty_theme_resolution=PASS\ncustom_xdg_opencode_instructions=PASS\n'
    'default_xdg_opencode_instructions=PASS\nopencode_unapply=PASS\n'
    'custom_xdg_fish_runtime_activation=PASS\nfish_activation_negative_control=PASS\n'
    'custom_xdg_fish_amp_completion=PASS\nfish_amp_negative_control=PASS\n'
    'repeat_convergence=PASS\nunrelated_content_preservation=PASS\n'
    'native_ghostty_unapply=PASS\nfish_assisted_unapply=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
