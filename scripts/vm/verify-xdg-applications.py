#!/usr/bin/python3
import os
import pathlib
import shutil
import subprocess
import sys

config_dir = pathlib.Path.home() / '.config/mise'
mise = pathlib.Path.home() / '.local/bin/mise'
ghostty = pathlib.Path('/Applications/Ghostty.app/Contents/MacOS/ghostty')
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
xdg = root / 'custom xdg space'
home.mkdir(parents=True)
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
}
base_env.pop('MISE_ENV', None)
base_env.pop('GH_CONFIG_DIR', None)


def run(name, arguments, env=None, timeout=60):
    arguments = [str(argument) for argument in arguments]
    try:
        result = subprocess.run(
            arguments,
            env=env or base_env,
            capture_output=True,
            text=True,
            timeout=timeout,
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
    if result.returncode != 0:
        raise AssertionError(f'{name} exited {result.returncode}: {result.stderr}')
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


apply('initial-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
apply('second-apply', 'terminal,development,ai-cli', 'packages,dotfiles')
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

fish_version = run('fish-mise', ['fish', '-ic', 'mise --version'], timeout=30)
assert fish_version.strip().startswith('2026.10.23 '), fish_version
fish_completions = run('fish-amp-completions', ['fish', '-ic', 'complete --do-complete="amp "'], timeout=30)
assert fish_completions.strip(), 'Amp produced no Fish completions'
fish_text = (xdg / 'fish/config.fish').read_text()
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
remaining_fish = (xdg / 'fish/config.fish').read_text()
assert '# synthetic user setting' in remaining_fish
assert 'set -g SYNTHETIC_FISH_VALUE preserve' in remaining_fish
assert 'mise:activate' not in remaining_fish and 'mise:amp' not in remaining_fish
assert 'custom_key: preserve' in (xdg / 'gh/config.yml').read_text()

report = (
    'custom_xdg_gh_consumption=PASS\nall_gh_scalars_alias_schema=PASS\n'
    'gh_config_dir_override=PASS\ncustom_xdg_ghostty_consumption=PASS\n'
    'ghostty_theme_resolution=PASS\ncustom_xdg_fish_activation=PASS\n'
    'custom_xdg_fish_amp_completion=PASS\nrepeat_convergence=PASS\n'
    'unrelated_content_preservation=PASS\nnative_ghostty_unapply=PASS\n'
    'fish_assisted_unapply=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
