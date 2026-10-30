#!/usr/bin/python3
import pathlib
import re
import sys
import tomllib

root = pathlib.Path(__file__).resolve().parents[2]
matrix = (root / 'docs/migration-decisions.md').read_text()
counts = {
    'packages': 0,
    'tools': 0,
    'dotfiles': 0,
    'settings': 0,
    'preferences': 0,
    'repositories': 0,
    'login_items': 0,
    'executables': 0,
}
missing = []


def require(kind, value):
    if value not in matrix:
        missing.append(f'{kind}: {value}')


def flatten_settings(values, prefix='settings'):
    for key, value in values.items():
        path = f'{prefix}.{key}'
        if isinstance(value, dict):
            flatten_settings(value, path)
        else:
            counts['settings'] += 1
            require('setting', path)


dock_keys = {
    'autohide', 'orientation', 'tilesize', 'magnification', 'show_recents', 'mru_spaces',
}
keyboard_keys = {'key_repeat', 'initial_key_repeat', 'press_and_hold'}

for config in sorted(root.glob('config*.toml')):
    data = tomllib.loads(config.read_text())
    bootstrap = data.get('bootstrap', {})
    packages = bootstrap.get('packages', {})
    for name, value in packages.items():
        counts['packages'] += 1
        require('package', name)
        if isinstance(value, dict):
            require('package version', str(value.get('version', '')))
    for name, value in data.get('tools', {}).items():
        counts['tools'] += 1
        require('tool', name)
        version = value.get('version') if isinstance(value, dict) else value
        require('tool version', str(version))
    for target in data.get('dotfiles', {}):
        counts['dotfiles'] += 1
        require('dotfile', target)
    flatten_settings(data.get('settings', {}))
    for domain, values in bootstrap.get('macos', {}).get('defaults', {}).items():
        for key in values:
            counts['preferences'] += 1
            require('preference', f'{domain} {key}')
    for entry in bootstrap.get('macos', {}).get('defaults_entries', []):
        counts['preferences'] += 1
        path = '.'.join(entry['path'])
        require('nested preference', f'{entry["domain"]} {entry["key"]}.{path}')
    for key in bootstrap.get('macos', {}).get('dock', {}):
        assert key in dock_keys, key
        counts['preferences'] += 1
        mapped = {'show_recents': 'show-recents', 'mru_spaces': 'mru-spaces'}.get(key, key)
        require('Dock preference', f'com.apple.dock {mapped}')
    for key in bootstrap.get('macos', {}).get('keyboard', {}):
        assert key in keyboard_keys, key
        counts['preferences'] += 1
        mapped = {'key_repeat': 'KeyRepeat', 'initial_key_repeat': 'InitialKeyRepeat', 'press_and_hold': 'ApplePressAndHoldEnabled'}[key]
        require('keyboard preference', f'NSGlobalDomain {mapped}')
    for name, value in bootstrap.get('repos', {}).items():
        counts['repositories'] += 1
        require('repository', name)
        if isinstance(value, dict):
            require('repository URL', str(value.get('url', '')))
            require('repository revision', str(value.get('ref', value.get('rev', ''))))
    for item in bootstrap.get('macos', {}).get('login_items', []):
        counts['login_items'] += 1
        require('login item', item['path'] if isinstance(item, dict) else str(item))
    for hook in bootstrap.get('hooks', {}).values():
        commands = hook.get('run', [])
        if isinstance(commands, str):
            commands = [commands]
        for command in commands:
            for package in re.findall(r'brew install(?: --cask)? ([^\s;]+)', command):
                require('Homebrew hook', package)

for executable in sorted((root / 'bin').glob('*')):
    if executable.is_file():
        counts['executables'] += 1
        require('executable', str(executable.relative_to(root)))

for kind, count in counts.items():
    print(f'{kind}={count}')
if missing:
    print('Missing exact migration-matrix records:', file=sys.stderr)
    for item in sorted(set(missing)):
        print(item, file=sys.stderr)
    raise SystemExit(1)
print('current_inventory=PASS')
