#!/opt/homebrew/bin/python3
import collections
import hashlib
import pathlib
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
ENVIRONMENTS = frozenset({
    'admin',
    'ai-cli',
    'ai-desktop',
    'ai-tools',
    'browsers',
    'design',
    'development',
    'development-apple',
    'development-apps',
    'development-containers',
    'development-editors',
    'development-java',
    'development-rust',
    'foundation',
    'infrastructure',
    'js',
    'kubernetes',
    'media',
    'messengers',
    'network',
    'office',
    'privacy',
    'productivity',
    'security',
    'system-preferences',
    'terminal',
    'utilities',
})
EXPECTED_COUNTS = {
    'environments': 27,
    'packages': 130,
    'unique_packages': 118,
    'brew': 47,
    'brew-cask': 49,
    'mas': 21,
    'macos-app': 1,
    'tools': 9,
    'dotfiles': 19,
    'settings': 1,
    'preferences': 67,
    'repositories': 2,
    'login_items': 2,
    'executables': 2,
}
EXPECTED_TOOLS = {
    ('js', 'node'): '26.9.0',
    ('js', 'bun'): '1.4.4',
    ('development-rust', 'rust'): {
        'version': '1.99.1',
        'mr_boxington': True,
        'components': ['clippy', 'rustfmt', 'rust-analyzer', 'rust-src', 'llvm-tools'],
    },
    ('development-rust', 'mr-boxington'): {'version': '1.24.1'},
    ('development-rust', 'aqua:nextest-rs/nextest/cargo-nextest'): '0.9.150',
    ('development-rust', 'aqua:taiki-e/cargo-llvm-cov'): '0.9.1',
    ('development-rust', 'aqua:EmbarkStudios/cargo-deny'): '0.20.2',
    ('development-rust', 'github:Canop/bacon'): {'version': '3.26.0', 'bin_path': 'aarch64-apple-darwin'},
    ('development-java', 'java'): 'oracle-graalvm-25.0.4.1.1',
}
EXPECTED_REPOSITORIES = {
    ('terminal', '~/.oh-my-zsh'): {
        'url': 'https://github.com/ohmyzsh/ohmyzsh.git',
        'ref': '0b50afdbfd8cbd0e03be68ecec4e1b5d3f55ef0a',
    },
    ('terminal', '~/.oh-my-zsh/custom/plugins/zsh-autosuggestions'): {
        'url': 'https://github.com/zsh-users/zsh-autosuggestions.git',
        'ref': '85919cd1ffa7d2d5412f6d3fe437ebdbeeec4fc5',
    },
}
EXPECTED_VARIANTS = {
    ('terminal', '~/.config/ghostty/config'): [
        {'default': True, 'target': '{{ xdg_config_home }}/ghostty/config'},
    ],
    ('terminal', '~/.config/ghostty/themes/generic-terminal'): [
        {'default': True, 'target': '{{ xdg_config_home }}/ghostty/themes/generic-terminal'},
    ],
    ('ai-cli', '~/.config/opencode/AGENTS.md'): [
        {'default': True, 'target': '{{ xdg_config_home }}/opencode/AGENTS.md'},
    ],
}
EXPECTED_NESTED_PREFERENCES = {
    ('system-preferences', 'NSGlobalDomain', 'AppleFirstWeekday', ('gregorian',)): 2,
    ('system-preferences', 'NSGlobalDomain', 'AppleICUDateFormatStrings', ('1',)): 'y-MM-dd',
}
EXPECTED_LOGIN_ITEMS = {
    ('utilities', '/Applications/CleanShot X.app'),
    ('utilities', '/Applications/Bartender 7.app'),
}
EXPECTED_EXECUTABLES = {'fish-config.py', 'claude-statusline.sh'}
STATUSLINE_SHA256 = 'e0268e5e61bb07456bdbbcda5975c7189f5e89168b8cd3df6821cadc2c05917d'
DOCK_KEYS = {
    'autohide', 'orientation', 'tilesize', 'magnification', 'show_recents', 'mru_spaces',
}
KEYBOARD_KEYS = {'key_repeat', 'initial_key_repeat', 'press_and_hold'}
BREW_INSTALL = re.compile(r'/opt/homebrew/bin/brew install(?: --cask)? [^\s;]+')
REAL_HOMEBREW = {
    'ai-cli': {
        'guards': {
            '/opt/homebrew/bin/brew install --cask claude-code@latest':
                '/opt/homebrew/bin/brew list --cask claude-code@latest',
            '/opt/homebrew/bin/brew install ampcode/tap/ampcode':
                '/opt/homebrew/bin/brew list --formula ampcode',
            '/opt/homebrew/bin/brew install can1357/tap/omp':
                '/opt/homebrew/bin/brew list --formula omp',
        },
        'health': (
            '/opt/homebrew/bin/claude --version',
            '/opt/homebrew/bin/amp --version',
            '/opt/homebrew/bin/omp --version',
        ),
    },
    'ai-tools': {
        'guards': {
            '/opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp':
                '/opt/homebrew/bin/brew list --formula fff-mcp',
        },
        'health': ('/opt/homebrew/bin/fff-mcp --healthcheck',),
    },
    'ai-desktop': {
        'guards': {
            '/opt/homebrew/bin/brew install --cask ampcode/tap/amp@beta':
                '/opt/homebrew/bin/brew list --cask amp@beta',
        },
        'health': (
            'test -x /Applications/Amp.app/Contents/MacOS/Amp',
            '/usr/bin/codesign --verify --deep --strict /Applications/Amp.app',
        ),
    },
    'network': {
        'guards': {
            '/opt/homebrew/bin/brew install --cask surge':
                '/opt/homebrew/bin/brew list --cask surge',
        },
        'health': (
            'test -x /opt/homebrew/bin/surge-cli',
            'test -x /opt/homebrew/sbin/surge-dhcpd',
            '/usr/bin/codesign --verify --deep --strict /Applications/Surge.app',
        ),
    },
}


def same_value(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(actual, list):
        return len(actual) == len(expected) and all(
            same_value(actual_item, expected_item)
            for actual_item, expected_item in zip(actual, expected)
        )
    if isinstance(actual, dict):
        return actual.keys() == expected.keys() and all(
            same_value(actual[key], expected[key]) for key in actual
        )
    return actual == expected


def as_table(value, label, errors):
    if not isinstance(value, dict):
        errors.append(f'{label}: expected a TOML table')
        return {}
    return value


def load_configs(root, counts, errors):
    paths = sorted(
        path for path in root.glob('config*.toml')
        if path.is_file() and not path.name.endswith('.local.toml')
    )
    actual_files = {path.name for path in paths}
    expected_files = {'config.toml'} | {f'config.{name}.toml' for name in ENVIRONMENTS}
    for name in sorted(expected_files - actual_files):
        errors.append(f'config: missing {name}')
    for name in sorted(actual_files - expected_files):
        errors.append(f'config: unexpected {name}')

    actual_environments = {
        path.name[len('config.'):-len('.toml')]
        for path in paths
        if path.name.startswith('config.') and path.name != 'config.toml'
    }
    counts['environments'] = len(actual_environments)
    for name in sorted(ENVIRONMENTS - actual_environments):
        errors.append(f'environments: missing {name}')
    for name in sorted(actual_environments - ENVIRONMENTS):
        errors.append(f'environments: unexpected {name}')

    configs = {}
    for path in paths:
        if path.name not in expected_files:
            continue
        owner = 'root' if path.name == 'config.toml' else path.name[len('config.'):-len('.toml')]
        try:
            data = tomllib.loads(path.read_text())
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            errors.append(f'{path.name}: cannot parse TOML: {error}')
            continue
        if owner == 'root':
            if not same_value(data, {'min_version': '2026.10.23'}):
                errors.append(
                    f'config.toml: expected only min_version = "2026.10.23", got {data!r}'
                )
        else:
            configs[owner] = data
    return configs


def validate_dotfile(root, owner, target, specification, errors):
    if not isinstance(specification, dict):
        errors.append(f'dotfile {owner} {target}: expected a declaration table')
        return
    keys = set(specification)
    if 'source' in specification:
        source = specification['source']
        if not isinstance(source, str) or not source:
            errors.append(f'dotfile {owner} {target}: source must be a non-empty string')
        else:
            source_path = pathlib.PurePosixPath(source)
            owned_prefix = pathlib.PurePosixPath('dotfiles') / owner
            try:
                source_path.relative_to(owned_prefix)
                owned = not source_path.is_absolute() and '..' not in source_path.parts
            except ValueError:
                owned = False
            if not owned:
                errors.append(
                    f'dotfile {owner} {target}: source {source!r} is outside dotfiles/{owner}/'
                )
            else:
                disk_path = root.joinpath(*source_path.parts)
                if disk_path.is_dir():
                    members = [path for path in disk_path.rglob('*') if not path.is_dir()]
                    if not members:
                        errors.append(f'dotfile {owner} {target}: empty source directory {source}')
                else:
                    members = [disk_path] if disk_path.is_file() else []
                    if not members:
                        errors.append(f'dotfile {owner} {target}: missing source {source}')
                owned_root = (root / 'dotfiles' / owner).resolve()
                for member in members:
                    try:
                        member.resolve().relative_to(owned_root)
                    except ValueError:
                        errors.append(
                            f'dotfile {owner} {target}: source {source} escapes dotfiles/{owner}/'
                        )
        if specification.get('mode') == 'copy':
            allowed = {'source', 'mode', 'variants'}
            shape = 'copy'
        elif specification.get('merge') is True:
            allowed = {'source', 'merge', 'format', 'variants'}
            shape = 'merge'
        else:
            errors.append(f'dotfile {owner} {target}: expected copy or merge=true shape')
            return
        if not keys <= allowed:
            errors.append(
                f'dotfile {owner} {target}: invalid {shape} keys {sorted(keys - allowed)}'
            )
        if 'format' in specification and (
            not isinstance(specification['format'], str) or not specification['format']
        ):
            errors.append(f'dotfile {owner} {target}: format must be a non-empty string')
    elif 'line' in specification:
        if keys != {'line'} or not isinstance(specification['line'], str) or not specification['line'].strip():
            errors.append(f'dotfile {owner} {target}: line shape must contain only a non-empty line')
    elif 'block' in specification:
        if keys != {'block'} or not isinstance(specification['block'], str) or not specification['block'].strip():
            errors.append(f'dotfile {owner} {target}: block shape must contain only a non-empty block')
    else:
        errors.append(f'dotfile {owner} {target}: expected source, line, or block shape')


def flatten_settings(owner, values, prefix='settings'):
    for key, value in values.items():
        path = f'{prefix}.{key}'
        if isinstance(value, dict):
            yield from flatten_settings(owner, value, path)
        else:
            yield owner, path, value


def collect_preferences(owner, macos, counts, nested, errors):
    defaults = as_table(macos.get('defaults', {}), f'{owner} bootstrap.macos.defaults', errors)
    for domain, values in defaults.items():
        values = as_table(values, f'{owner} defaults {domain}', errors)
        for key in values:
            counts['preferences'] += 1
            if not isinstance(key, str) or not key:
                errors.append(f'{owner} defaults {domain}: preference key must be non-empty')

    dock = as_table(macos.get('dock', {}), f'{owner} bootstrap.macos.dock', errors)
    for key in dock:
        counts['preferences'] += 1
        if key not in DOCK_KEYS:
            errors.append(f'{owner} dock: unexpected preference {key}')

    keyboard = as_table(macos.get('keyboard', {}), f'{owner} bootstrap.macos.keyboard', errors)
    for key in keyboard:
        counts['preferences'] += 1
        if key not in KEYBOARD_KEYS:
            errors.append(f'{owner} keyboard: unexpected preference {key}')

    entries = macos.get('defaults_entries', [])
    if not isinstance(entries, list):
        errors.append(f'{owner} bootstrap.macos.defaults_entries: expected an array')
        return
    for index, entry in enumerate(entries):
        label = f'{owner} defaults_entries[{index}]'
        entry = as_table(entry, label, errors)
        if not entry:
            continue
        counts['preferences'] += 1
        if set(entry) != {'domain', 'key', 'path', 'value'}:
            errors.append(f'{label}: expected exactly domain, key, path, and value')
            continue
        domain, key, path = entry['domain'], entry['key'], entry['path']
        if not all(isinstance(value, str) and value for value in (domain, key)):
            errors.append(f'{label}: domain and key must be non-empty strings')
            continue
        if not isinstance(path, list) or not all(isinstance(part, str) and part for part in path):
            errors.append(f'{label}: path must be an array of non-empty strings')
            continue
        nested[(owner, domain, key, tuple(path))] = entry['value']


def hook_commands(value, label, errors):
    if isinstance(value, str):
        return [value]
    if isinstance(value, list) and all(isinstance(command, str) for command in value):
        return value
    errors.append(f'{label}: run must be a string or an array of strings')
    return []


def shell_tokens(command):
    lexer = shlex.shlex(command, posix=True, punctuation_chars='();<>|&\n')
    lexer.whitespace_split = True
    lexer.whitespace = ' \t\r'
    tokens = []
    for token in lexer:
        if '\n' in token and all(character in '();<>|&\n' for character in token):
            tokens.extend(part for part in re.split(r'(\n)', token) if part)
        else:
            tokens.append(token)
    return tokens


def parse_shell(command):
    tokens = shell_tokens(command)

    def parse_list(index, stops):
        commands = []
        while index < len(tokens):
            token = tokens[index]
            if token in {';', '\n'}:
                index += 1
                continue
            if token in stops:
                break
            if token == 'if':
                branches, index = parse_if(index)
                commands.append(('if', branches))
                continue
            start = index
            while index < len(tokens) and tokens[index] not in {';', '\n'}:
                index += 1
            commands.append(('command', tokens[start:index]))
        return commands, index

    def parse_if(index):
        branches = []
        while True:
            condition, index = parse_list(index + 1, {'then'})
            if index == len(tokens):
                raise ValueError('missing then')
            body, index = parse_list(index + 1, {'elif', 'else', 'fi'})
            if index == len(tokens):
                raise ValueError('missing fi')
            branches.append((condition, body))
            if tokens[index] == 'elif':
                continue
            if tokens[index] == 'else':
                body, index = parse_list(index + 1, {'fi'})
                if index == len(tokens):
                    raise ValueError('missing fi')
                branches.append((None, body))
            return branches, index + 1

    commands, index = parse_list(0, {'then', 'elif', 'else', 'fi'})
    if index != len(tokens):
        raise ValueError('unbalanced shell conditional')
    return commands


def contains_guarded_install(commands, condition, install):
    for kind, value in commands:
        if kind != 'if':
            continue
        for branch_condition, body in value:
            if branch_condition == [('command', condition)] and ('command', install) in body:
                return True
            if contains_guarded_install(body, condition, install):
                return True
    return False


def has_presence_guard(commands, install, guard):
    condition = shell_tokens(f'! {guard} >/dev/null 2>&1')
    install = shell_tokens(install)
    for command in commands:
        try:
            parsed = parse_shell(command)
        except ValueError:
            continue
        if contains_guarded_install(parsed, condition, install):
            return True
    return False


def validate_homebrew(hooks, errors):
    actual_installs = collections.Counter()
    for (owner, hook_name), hook in hooks.items():
        for command in hook['commands']:
            for match in BREW_INSTALL.finditer(command):
                actual_installs[(owner, hook_name, match.group(0))] += 1

    expected_installs = collections.Counter({
        (owner, 'post-packages', install): 1
        for owner, specification in REAL_HOMEBREW.items()
        for install in specification['guards']
    })
    for (owner, hook_name, install), count in sorted((actual_installs - expected_installs).items()):
        errors.append(f'Homebrew: unexpected {owner} {hook_name} command {install!r} (x{count})')
    for (owner, hook_name, install), count in sorted((expected_installs - actual_installs).items()):
        errors.append(f'Homebrew: missing {owner} {hook_name} command {install!r} (x{count})')

    for owner, specification in REAL_HOMEBREW.items():
        hook = hooks.get((owner, 'post-packages'))
        if hook is None:
            errors.append(f'{owner} post-packages: missing real-Homebrew hook')
            continue
        description = hook['description']
        if not isinstance(description, str) or not all(
            phrase in description.lower()
            for phrase in ('presence-only', 'upgrades are separate')
        ):
            errors.append(
                f'{owner} post-packages: description must state presence-only and upgrades are separate'
            )
        lines = {
            line.strip()
            for command in hook['commands']
            for line in command.splitlines()
        }
        for health_check in specification['health']:
            if health_check not in lines:
                errors.append(f'{owner} post-packages: missing health check {health_check!r}')
        for install, guard in specification['guards'].items():
            if (owner, 'post-packages', install) not in actual_installs:
                continue
            if not has_presence_guard(hook['commands'], install, guard):
                errors.append(f'{owner}: {install}: missing active presence guard ({guard})')


def compare_exact(label, actual, expected, errors):
    for key in sorted(set(actual) - set(expected)):
        errors.append(f'{label}: unexpected {key!r}')
    for key in sorted(set(expected) - set(actual)):
        errors.append(f'{label}: missing {key!r}')
    for key in sorted(set(actual) & set(expected)):
        if not same_value(actual[key], expected[key]):
            errors.append(f'{label} {key!r}: got {actual[key]!r}, expected {expected[key]!r}')


def validate(root):
    counts = dict.fromkeys(EXPECTED_COUNTS, 0)
    errors = []
    for name in ('miserc.local.toml', 'config.workspaces.local.toml'):
        tracked = subprocess.run(
            ['git', '-C', str(root), 'ls-files', '--error-unmatch', '--', name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if tracked.returncode == 0:
            errors.append(f'committed local configuration is forbidden: {name}')

    configs = load_configs(root, counts, errors)
    packages_by_owner = {}
    package_identities = set()
    actual_tools = {}
    actual_variants = {}
    actual_settings = []
    actual_nested_preferences = {}
    actual_repositories = {}
    actual_login_items = []
    hooks = {}

    for owner, data in sorted(configs.items()):
        bootstrap = as_table(data.get('bootstrap', {}), f'{owner} bootstrap', errors)
        packages = as_table(bootstrap.get('packages', {}), f'{owner} bootstrap.packages', errors)
        packages_by_owner[owner] = set(packages)
        for name, value in packages.items():
            counts['packages'] += 1
            kind, separator, identity = name.partition(':')
            if not separator or not identity or kind not in {'brew', 'brew-cask', 'mas', 'macos-app'}:
                errors.append(f'{owner} package {name!r}: invalid native package identity')
                continue
            package_identities.add(name)
            if kind == 'macos-app':
                required = {'version', 'url', 'sha256', 'artifact', 'os'}
                if not isinstance(value, dict) or set(value) != required or not all(
                    isinstance(value[key], str) and value[key] for key in required
                ):
                    errors.append(
                        f'{owner} package {name}: expected non-empty version/url/sha256/artifact/os fields'
                    )
            elif not isinstance(value, str) or not value:
                errors.append(f'{owner} package {name}: expected a non-empty version string')

        tools = as_table(data.get('tools', {}), f'{owner} tools', errors)
        for name, value in tools.items():
            counts['tools'] += 1
            actual_tools[(owner, name)] = value

        dotfiles = as_table(data.get('dotfiles', {}), f'{owner} dotfiles', errors)
        for target, specification in dotfiles.items():
            counts['dotfiles'] += 1
            validate_dotfile(root, owner, target, specification, errors)
            if isinstance(specification, dict) and 'variants' in specification:
                actual_variants[(owner, target)] = specification['variants']

        settings = as_table(data.get('settings', {}), f'{owner} settings', errors)
        for setting_owner, path, value in flatten_settings(owner, settings):
            counts['settings'] += 1
            actual_settings.append((setting_owner, path))
            if not isinstance(value, str) or not value:
                errors.append(f'setting {setting_owner} {path}: expected a non-empty command string')

        macos = as_table(bootstrap.get('macos', {}), f'{owner} bootstrap.macos', errors)
        collect_preferences(owner, macos, counts, actual_nested_preferences, errors)

        repositories = as_table(bootstrap.get('repos', {}), f'{owner} bootstrap.repos', errors)
        for destination, value in repositories.items():
            counts['repositories'] += 1
            actual_repositories[(owner, destination)] = value

        login_items = macos.get('login_items', {})
        if login_items:
            login_items = as_table(
                login_items, f'{owner} bootstrap.macos.login_items', errors
            )
            if set(login_items) != {'apps', 'exclusive'}:
                errors.append(f'{owner} login items: expected only apps and exclusive')
            if login_items.get('exclusive') is not False:
                errors.append(f'{owner} login items: exclusive must be false')
            apps = login_items.get('apps', [])
            if not isinstance(apps, list) or not all(isinstance(path, str) for path in apps):
                errors.append(f'{owner} login items: apps must be an array of paths')
            else:
                for path in apps:
                    counts['login_items'] += 1
                    actual_login_items.append((owner, path))

        raw_hooks = as_table(bootstrap.get('hooks', {}), f'{owner} bootstrap.hooks', errors)
        for hook_name, value in raw_hooks.items():
            label = f'{owner} {hook_name}'
            hook = as_table(value, label, errors)
            hooks[(owner, hook_name)] = {
                'description': hook.get('description', ''),
                'commands': hook_commands(hook.get('run', []), label, errors),
            }

    unique_kinds = collections.Counter(name.split(':', 1)[0] for name in package_identities)
    counts['unique_packages'] = len(package_identities)
    for kind in ('brew', 'brew-cask', 'mas', 'macos-app'):
        counts[kind] = unique_kinds[kind]

    if 'brew:mas' not in packages_by_owner.get('foundation', set()):
        errors.append('foundation: missing default brew:mas')
    for owner in ('terminal', 'development-editors'):
        if 'brew-cask:font-jetbrains-mono-nerd-font' not in packages_by_owner.get(owner, set()):
            errors.append(f'{owner}: missing shared prerequisite brew-cask:font-jetbrains-mono-nerd-font')
    for owner in ('ai-tools', 'ai-cli'):
        if 'brew:jq' not in packages_by_owner.get(owner, set()):
            errors.append(f'{owner}: missing shared prerequisite brew:jq')
    for owner, packages in sorted(packages_by_owner.items()):
        if any(name.startswith('mas:') for name in packages) and 'brew:mas' not in packages:
            errors.append(f'{owner}: missing shared prerequisite brew:mas for MAS packages')

    compare_exact('runtime', actual_tools, EXPECTED_TOOLS, errors)
    compare_exact('repository', actual_repositories, EXPECTED_REPOSITORIES, errors)
    compare_exact('dotfile variant', actual_variants, EXPECTED_VARIANTS, errors)
    compare_exact(
        'nested preference', actual_nested_preferences, EXPECTED_NESTED_PREFERENCES, errors
    )
    compare_exact(
        'nested setting',
        dict.fromkeys(actual_settings, True),
        {('development-rust', 'settings.github.credential_command'): True},
        errors,
    )

    for item in sorted(set(actual_login_items) - EXPECTED_LOGIN_ITEMS):
        errors.append(f'login items: unexpected {item[0]} {item[1]}')
    for item in sorted(EXPECTED_LOGIN_ITEMS - set(actual_login_items)):
        errors.append(f'login items: missing {item[0]} {item[1]}')

    validate_homebrew(hooks, errors)

    bin_directory = root / 'bin'
    try:
        actual_executables = {
            path.name for path in bin_directory.iterdir() if path.is_file() or path.is_symlink()
        }
    except OSError as error:
        actual_executables = set()
        errors.append(f'bin: cannot inspect executables: {error}')
    counts['executables'] = len(actual_executables)
    for name in sorted(actual_executables - EXPECTED_EXECUTABLES):
        errors.append(f'bin: unexpected executable {name}')
    for name in sorted(EXPECTED_EXECUTABLES - actual_executables):
        errors.append(f'bin: missing executable {name}')
    for name in sorted(EXPECTED_EXECUTABLES & actual_executables):
        path = bin_directory / name
        try:
            if not path.is_file() or path.stat().st_mode & 0o111 == 0:
                errors.append(f'bin/{name}: expected an executable regular file')
        except OSError as error:
            errors.append(f'bin/{name}: cannot inspect: {error}')
    statusline = bin_directory / 'claude-statusline.sh'
    if statusline.is_file():
        actual_hash = hashlib.sha256(statusline.read_bytes()).hexdigest()
        if actual_hash != STATUSLINE_SHA256:
            errors.append(
                f'bin/claude-statusline.sh: SHA256 {actual_hash}, expected {STATUSLINE_SHA256}'
            )

    for name, expected in EXPECTED_COUNTS.items():
        if counts[name] != expected:
            errors.append(f'count {name}: got {counts[name]}, expected {expected}')
    return counts, errors


def copy_inputs(source, destination):
    destination.mkdir()
    for name in ['config.toml'] + [f'config.{owner}.toml' for owner in sorted(ENVIRONMENTS)]:
        shutil.copy2(source / name, destination / name)
    for name in ('bin', 'dotfiles'):
        shutil.copytree(source / name, destination / name)


def replace_once(path, old, new):
    text = path.read_text()
    if text.count(old) != 1:
        raise RuntimeError(f'mutation anchor occurs {text.count(old)} times in {path.name}')
    path.write_text(text.replace(old, new, 1))


def self_test(root):
    mutations = [
        (
            'remove_foundation_mas',
            'config.foundation.toml',
            '"brew:mas" = "latest"\n',
            '',
            'foundation: missing default brew:mas',
        ),
        (
            'remove_ai_cli_jq',
            'config.ai-cli.toml',
            '"brew:jq" = "latest"\n',
            '',
            'ai-cli: missing shared prerequisite brew:jq',
        ),
        (
            'add_unknown_login_item',
            'config.utilities.toml',
            'apps = ["/Applications/CleanShot X.app", "/Applications/Bartender 7.app"]',
            'apps = ["/Applications/CleanShot X.app", "/Applications/Bartender 7.app", "/Applications/Unknown.app"]',
            'login items: unexpected utilities /Applications/Unknown.app',
        ),
        (
            'hardcode_opencode_variant',
            'config.ai-cli.toml',
            '{{ xdg_config_home }}/opencode/AGENTS.md',
            '~/.config/opencode/AGENTS.md',
            "dotfile variant ('ai-cli', '~/.config/opencode/AGENTS.md')",
        ),
        (
            'remove_ghostty_variant',
            'config.terminal.toml',
            ', variants = [{ default = true, target = "{{ xdg_config_home }}/ghostty/config" }]',
            '',
            "dotfile variant: missing ('terminal', '~/.config/ghostty/config')",
        ),
        (
            'change_runtime_pin',
            'config.js.toml',
            'node = "26.9.0"',
            'node = "26.9.1"',
            "runtime ('js', 'node')",
        ),
        (
            'remove_presence_guard',
            'config.ai-tools.toml',
            'if ! /opt/homebrew/bin/brew list --formula fff-mcp >/dev/null 2>&1; then /opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp; fi',
            '/opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp',
            'ai-tools: /opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp: missing active presence guard',
        ),
        (
            'install_after_presence_guard',
            'config.ai-tools.toml',
            'if ! /opt/homebrew/bin/brew list --formula fff-mcp >/dev/null 2>&1; then /opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp; fi',
            'if ! /opt/homebrew/bin/brew list --formula fff-mcp >/dev/null 2>&1; then\\n  :\\nfi\\n/opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp\\nif true; then\\n  :\\nfi',
            'ai-tools: /opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp: missing active presence guard',
        ),
    ]
    passed = True
    try:
        with tempfile.TemporaryDirectory(prefix='verify-current-inventory-') as temporary:
            temporary_root = pathlib.Path(temporary)
            baseline = temporary_root / 'baseline'
            copy_inputs(root, baseline)
            (baseline / 'miserc.local.toml').write_text('env = ["foundation"]\n')
            (baseline / 'config.workspaces.local.toml').write_text('[bootstrap.repos]\n')
            _, baseline_errors = validate(baseline)
            if baseline_errors:
                print('baseline=FAIL', file=sys.stderr)
                for error in baseline_errors:
                    print(f'  {error}', file=sys.stderr)
                return 1
            print('baseline=PASS')

            for name, filename, old, new, expected_error in mutations:
                case_root = temporary_root / name
                copy_inputs(root, case_root)
                replace_once(case_root / filename, old, new)
                _, errors = validate(case_root)
                if not errors:
                    print(f'mutation_{name}=NOT_REJECTED')
                    passed = False
                elif not any(expected_error in error for error in errors):
                    print(f'mutation_{name}=REJECTED_FOR_WRONG_REASON')
                    for error in errors:
                        print(f'  {error}', file=sys.stderr)
                    passed = False
                else:
                    print(f'mutation_{name}=REJECTED')
    except (OSError, RuntimeError) as error:
        print(f'self-test setup failed: {error}', file=sys.stderr)
        return 1
    if not passed:
        print('self_test=FAIL', file=sys.stderr)
        return 1
    print('self_test=PASS')
    return 0


def main():
    arguments = sys.argv[1:]
    if arguments == ['--self-test']:
        return self_test(ROOT)
    if arguments:
        print(f'usage: {pathlib.Path(sys.argv[0]).name} [--self-test]', file=sys.stderr)
        return 1
    counts, errors = validate(ROOT)
    for name in EXPECTED_COUNTS:
        print(f'{name}={counts[name]}')
    if errors:
        print('current_inventory=FAIL', file=sys.stderr)
        for error in sorted(set(errors)):
            print(error, file=sys.stderr)
        return 1
    print('current_inventory=PASS')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
