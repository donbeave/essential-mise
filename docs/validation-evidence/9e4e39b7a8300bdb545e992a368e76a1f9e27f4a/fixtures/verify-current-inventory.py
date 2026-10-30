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
matrix_errors = []


def code_spans(cell):
    return re.findall(r'`([^`]+)`', cell)


def exact_code(cell):
    stripped = cell.strip()
    spans = code_spans(stripped)
    if len(spans) == 1 and stripped == f'`{spans[0]}`':
        return spans[0]
    return None


def split_table_row(line):
    cells = []
    current = []
    in_code = False
    escaped = False
    for character in line.strip():
        if escaped:
            current.append(character)
            escaped = False
            continue
        if character == '\\':
            current.append(character)
            escaped = True
            continue
        if character == '`':
            in_code = not in_code
        if character == '|' and not in_code:
            cells.append(''.join(current).strip())
            current = []
        else:
            current.append(character)
    cells.append(''.join(current).strip())
    if cells and not cells[0]:
        cells.pop(0)
    if cells and not cells[-1]:
        cells.pop()
    return cells


def markdown_tables(text):
    lines = text.splitlines()
    section = ''
    subsection = ''
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.startswith('## '):
            section = line
            subsection = ''
        elif line.startswith('### '):
            subsection = line
        if not line.startswith('|'):
            index += 1
            continue
        block = []
        while index < len(lines) and lines[index].startswith('|'):
            block.append(split_table_row(lines[index]))
            index += 1
        if len(block) >= 2 and all(re.fullmatch(r':?-{3,}:?', cell) for cell in block[1]):
            yield section, subsection, block[0], block[2:]
        elif section.startswith('## 2.') and all(len(row) == 4 for row in block):
            yield section, subsection, ['Module', 'Exact declaration', 'Status', 'Validation'], block


def add_record(records, key, value, label):
    if key in records:
        matrix_errors.append(f'{label}: duplicate record for {key}')
        return
    records[key] = value


def parse_value(text):
    return tomllib.loads(f'value = {text}')['value']


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


def mismatch(kind, owner, resource, detail):
    missing.append(f'{kind}: {owner}: {resource} ({detail})')


tables = list(markdown_tables(matrix))
declarations = {}
direct_packages = {}
hook_records = {}
dotfile_records = {}
legacy_dotfile_records = {}
setting_records = {}
nested_preference_additions = {}
executable_records = {}
repository_records = {}
preference_records = {}
login_records = {}

for section, subsection, header, rows in tables:
    if section.startswith('## 2.') and header == ['Module', 'Exact declaration', 'Status', 'Validation']:
        for cells in rows:
            if len(cells) != 4:
                matrix_errors.append(f'package/runtime table: expected four cells, got {cells}')
                continue
            owner = exact_code(cells[0])
            if owner is None:
                matrix_errors.append(f'package/runtime table: invalid owner cell {cells[0]}')
                continue
            declaration = cells[1].strip()
            literal = exact_code(declaration)
            if literal is not None:
                try:
                    parsed = tomllib.loads(literal)
                except tomllib.TOMLDecodeError as error:
                    matrix_errors.append(f'{owner}: invalid TOML declaration {literal}: {error}')
                    continue
                if len(parsed) != 1:
                    matrix_errors.append(f'{owner}: expected one declaration, got {literal}')
                    continue
                name, value = next(iter(parsed.items()))
                kind = 'package' if re.match(r'^(brew|brew-cask|mas):', name) else 'tool'
                add_record(declarations, (owner, name), {
                    'kind': kind,
                    'value': value,
                    'status': cells[2],
                    'validation': cells[3],
                }, 'package/runtime')
                continue
            spans = code_spans(declaration)
            if spans and spans[0].startswith('macos-app:'):
                if len(spans) != 5:
                    matrix_errors.append(f'{owner}: invalid direct-package record {declaration}')
                    continue
                add_record(direct_packages, (owner, spans[0]), {
                    'version': spans[1],
                    'url_fragment': spans[2],
                    'sha256': spans[3],
                    'artifact': spans[4],
                }, 'direct package')
                continue
            if 'Homebrew' in declaration:
                command = re.search(
                    r'/opt/homebrew/bin/brew install(?P<cask> --cask)? (?P<package>[^\s;`]+)',
                    declaration,
                )
                if command is not None:
                    package = command.group('package')
                    cask = command.group('cask') is not None
                    exact_command = command.group(0)
                elif len(spans) >= 2 and 'hook' in declaration:
                    package = spans[0]
                    cask = None
                    exact_command = None
                else:
                    matrix_errors.append(f'{owner}: invalid Homebrew hook record {declaration}')
                    continue
                if 'post-packages' not in declaration:
                    matrix_errors.append(f'{owner}: Homebrew hook has no post-packages owner: {declaration}')
                    continue
                add_record(hook_records, (owner, package), {
                    'hook': 'post-packages',
                    'cask': cask,
                    'command': exact_command,
                }, 'Homebrew hook')
                continue
            matrix_errors.append(f'{owner}: unrecognized package/runtime record {declaration}')

    if section.startswith('## 2.') and header == [
        'Exact resource', 'Owner and status', 'Deployment, validation, and removal',
    ]:
        for cells in rows:
            if len(cells) != 3:
                matrix_errors.append(f'current-resource table: expected three cells, got {cells}')
                continue
            resource = exact_code(cells[0])
            owner_text, separator, status = cells[1].partition(';')
            owner = exact_code(owner_text.strip())
            if resource is None or not separator:
                matrix_errors.append(f'current-resource table: invalid row {cells}')
                continue
            record = {'status': status.strip(), 'detail': cells[2]}
            if resource.startswith('~/'):
                if owner is None:
                    matrix_errors.append(f'{resource}: invalid module owner')
                    continue
                add_record(dotfile_records, (owner, resource), record, 'dotfile')
            elif resource.startswith('settings.'):
                if owner is None:
                    matrix_errors.append(f'{resource}: invalid module owner')
                    continue
                add_record(setting_records, (owner, resource), record, 'setting')
            elif resource.startswith('bin/'):
                executable_records[resource] = {
                    **record,
                    'owner': owner_text.strip(),
                }
            elif resource.startswith('NSGlobalDomain '):
                if owner is None:
                    matrix_errors.append(f'{resource}: invalid module owner')
                    continue
                add_record(nested_preference_additions, (owner, resource), record, 'nested preference')
            else:
                matrix_errors.append(f'current-resource table: unknown resource identity {resource}')

    if section.startswith('## 3.') and header == ['Destination', 'Exact declaration', 'Status', 'Validation']:
        for cells in rows:
            if len(cells) != 4:
                matrix_errors.append(f'repository table: expected four cells, got {cells}')
                continue
            destination = exact_code(cells[0])
            declaration = exact_code(cells[1])
            if destination is None or declaration is None:
                matrix_errors.append(f'repository table: invalid row {cells}')
                continue
            try:
                value = tomllib.loads(f'value = {{ {declaration} }}')['value']
            except tomllib.TOMLDecodeError as error:
                matrix_errors.append(f'{destination}: invalid repository declaration: {error}')
                continue
            add_record(repository_records, destination, {
                'value': value,
                'status': cells[2],
            }, 'repository')

    if section.startswith('## 4.') and header == ['Source file', 'Status', 'Destination, reason, and validation']:
        for cells in rows:
            if len(cells) != 3:
                matrix_errors.append(f'root-dotfile table: expected three cells, got {cells}')
                continue
            source = exact_code(cells[0])
            if source is None:
                matrix_errors.append(f'root-dotfile table: invalid source {cells[0]}')
                continue
            if 'NATIVE' not in cells[1]:
                continue
            for target in code_spans(cells[2]):
                if target.startswith('~/'):
                    add_record(legacy_dotfile_records, target, {
                        'source': source,
                        'status': cells[1],
                        'detail': cells[2],
                    }, 'legacy dotfile')

    if section.startswith('## 6.') and header == ['Domain/key', 'Exact value/declaration', 'Status', 'Validation']:
        for cells in rows:
            if len(cells) != 4:
                matrix_errors.append(f'preference table: expected four cells, got {cells}')
                continue
            identity = exact_code(cells[0])
            value_spans = code_spans(cells[1])
            if identity is None or not value_spans:
                if 'MANUAL' in cells[2]:
                    continue
                matrix_errors.append(f'preference table: invalid row {cells}')
                continue
            nested = re.fullmatch(r'(\S+) (.+?)\.path = (\[.*\])', identity)
            try:
                if nested:
                    domain, key, path_text = nested.groups()
                    path = tuple(parse_value(path_text))
                else:
                    domain, key = identity.split(' ', 1)
                    path = ()
                value = parse_value(value_spans[0])
            except (ValueError, tomllib.TOMLDecodeError) as error:
                matrix_errors.append(f'{identity}: invalid preference record: {error}')
                continue
            add_record(preference_records, (domain, key, path), {
                'value': value,
                'status': cells[2],
                'validation': cells[3],
            }, 'preference')

    if subsection == '### Login items' and header == ['Application', 'Exact value', 'Status', 'Validation']:
        for cells in rows:
            if len(cells) != 4:
                matrix_errors.append(f'login-item table: expected four cells, got {cells}')
                continue
            spans = code_spans(cells[1])
            if len(spans) != 3 or spans[1] != '[bootstrap.macos.login_items]':
                matrix_errors.append(f'login-item table: invalid exact value {cells[1]}')
                continue
            try:
                policy = tomllib.loads(spans[2])
            except tomllib.TOMLDecodeError as error:
                matrix_errors.append(f'{spans[0]}: invalid login-item policy: {error}')
                continue
            if set(policy) != {'exclusive'} or type(policy['exclusive']) is not bool:
                matrix_errors.append(f'{spans[0]}: expected only a boolean exclusive policy')
                continue
            add_record(login_records, spans[0], {
                'application': cells[0].strip(),
                'exclusive': policy['exclusive'],
                'status': cells[2],
                'validation': cells[3],
            }, 'login item')


def require_declaration(owner, kind, name, value):
    if kind == 'package' and (owner, name) in direct_packages:
        expected = direct_packages[(owner, name)]
        if not isinstance(value, dict):
            mismatch(kind, owner, name, 'expected direct-package dictionary')
            return
        for key in ('version', 'sha256', 'artifact'):
            if not same_value(value.get(key), expected[key]):
                mismatch(kind, owner, name, f'{key}={value.get(key)!r}, expected {expected[key]!r}')
        if expected['url_fragment'] not in str(value.get('url', '')):
            mismatch(kind, owner, name, f'URL does not contain {expected["url_fragment"]}')
        return
    record = declarations.get((owner, name))
    if record is None:
        mismatch(kind, owner, name, 'no exact owner/resource record')
        return
    if record['kind'] != kind:
        mismatch(kind, owner, name, f'matrix records a {record["kind"]}')
        return
    if not same_value(value, record['value']):
        mismatch(kind, owner, name, f'value={value!r}, expected {record["value"]!r}')


def require_hook(owner, hook_name, description, package, cask, command):
    record = hook_records.get((owner, package))
    if record is None:
        mismatch('Homebrew hook', owner, package, 'no exact owner/resource record')
        return
    if record['hook'] != hook_name:
        mismatch('Homebrew hook', owner, package, f'owner={hook_name}, expected {record["hook"]}')
    if record['cask'] is not None and record['cask'] != cask:
        mismatch('Homebrew hook', owner, package, 'formula/cask identity differs')
    if record['command'] is not None and record['command'] != command:
        mismatch('Homebrew hook', owner, package, f'command={command!r}, expected {record["command"]!r}')
    lifecycle = description.lower()
    if 'presence-only' not in lifecycle or 'upgrades are separate' not in lifecycle:
        mismatch('Homebrew hook lifecycle', owner, package, 'requires presence-only health checks and separate upgrades')


def require_dotfile(owner, target, specification):
    record = dotfile_records.get((owner, target))
    owned_target = any(target == candidate for _, candidate in dotfile_records)
    legacy = legacy_dotfile_records.get(target)
    if record is None and owned_target:
        mismatch('dotfile', owner, target, 'exact target belongs to another module')
        return
    if record is None and legacy is None:
        mismatch('dotfile', owner, target, 'no exact owner/resource record')
        return
    details = ' '.join(item['detail'] for item in (record, legacy) if item is not None)
    if not isinstance(specification, dict):
        mismatch('dotfile', owner, target, 'expected a declaration dictionary')
        return
    if 'source' in specification:
        source = str(specification['source'])
        if not source.startswith(f'dotfiles/{owner}/'):
            mismatch('dotfile source', owner, target, f'owner-mismatched source {source}')
        if record is None and legacy is not None:
            expected_name = pathlib.PurePosixPath(legacy['source']).name
            actual_name = pathlib.PurePosixPath(source).name
            if actual_name != expected_name and actual_name not in legacy['detail']:
                mismatch('dotfile source', owner, target, f'source={source}, matrix source={legacy["source"]}')
        if specification.get('merge') is True:
            if 'merge' not in details.lower():
                mismatch('dotfile deployment', owner, target, 'matrix does not declare a merge')
        elif specification.get('mode') != 'copy':
            mismatch('dotfile deployment', owner, target, 'copy declaration does not match matrix')
        if 'format' in specification and str(specification['format']).lower() not in details.lower():
            mismatch('dotfile format', owner, target, f'format={specification["format"]}')
    elif 'line' in specification:
        if record is None or 'edit' not in record['detail'].lower():
            mismatch('dotfile deployment', owner, target, 'no exact managed-line record')
    elif 'block' in specification:
        if record is None or not any(word in record['detail'].lower() for word in ('block', 'edit', 'wrapper')):
            mismatch('dotfile deployment', owner, target, 'no exact managed-block record')
    else:
        mismatch('dotfile deployment', owner, target, 'unknown source/line/block shape')


def flatten_settings(owner, values, prefix='settings'):
    for key, value in values.items():
        path = f'{prefix}.{key}'
        if isinstance(value, dict):
            flatten_settings(owner, value, path)
            continue
        counts['settings'] += 1
        record = setting_records.get((owner, path))
        if record is None:
            mismatch('setting', owner, path, 'no exact owner/resource record')
            continue
        for command in code_spans(record['detail']):
            if command not in str(value):
                mismatch('setting value', owner, path, f'missing documented command {command!r}')


def require_preference(owner, domain, key, path, value):
    if owner != 'system-preferences':
        mismatch('preference', owner, f'{domain} {key}', 'matrix owner is system-preferences')
        return
    identity = (domain, key, tuple(path))
    record = preference_records.get(identity)
    resource = f'{domain} {key}' + (f'.{".".join(path)}' if path else '')
    if record is None:
        mismatch('preference', owner, resource, 'no exact domain/key/path record')
        return
    if not same_value(value, record['value']):
        mismatch('preference value', owner, resource, f'value={value!r}, expected {record["value"]!r}')
    if path and (owner, resource) not in nested_preference_additions:
        mismatch('nested preference lifecycle', owner, resource, 'no exact owner/preservation record')


def require_repository(owner, destination, value):
    record = repository_records.get(destination)
    if record is None:
        mismatch('repository', owner, destination, 'no exact destination record')
        return
    if not same_value(value, record['value']):
        mismatch('repository declaration', owner, destination, f'value={value!r}, expected {record["value"]!r}')


def normalized_name(value):
    return re.sub(r'[^a-z0-9]', '', value.lower())


def require_login_item(owner, path):
    record = login_records.get(path)
    if record is None:
        mismatch('login item', owner, path, 'no exact path record')
        return
    expected_owners = {
        record_owner
        for (record_owner, name), declaration in declarations.items()
        if declaration['kind'] == 'package'
        and 'path verified' in declaration['validation'].lower()
        and normalized_name(name.split(':', 1)[-1]) == normalized_name(record['application'])
    }
    if owner not in expected_owners:
        mismatch('login item owner', owner, path, f'expected module(s) {sorted(expected_owners)}')


def require_login_policy(owner, paths, exclusive):
    if type(exclusive) is not bool:
        mismatch('login item policy', owner, 'exclusive', f'expected boolean, got {exclusive!r}')
        return
    expected = {
        login_records[path]['exclusive']
        for path in paths
        if path in login_records
    }
    if len(expected) > 1:
        matrix_errors.append(f'{owner}: login-item rows disagree on exclusive policy')
    elif expected and exclusive not in expected:
        mismatch('login item policy', owner, 'exclusive', f'value={exclusive!r}, expected {expected.pop()!r}')


dock_keys = {
    'autohide', 'orientation', 'tilesize', 'magnification', 'show_recents', 'mru_spaces',
}
keyboard_keys = {'key_repeat', 'initial_key_repeat', 'press_and_hold'}

for config in sorted(root.glob('config*.toml')):
    owner = 'root' if config.name == 'config.toml' else config.name[len('config.'):-len('.toml')]
    data = tomllib.loads(config.read_text())
    bootstrap = data.get('bootstrap', {})
    for name, value in bootstrap.get('packages', {}).items():
        counts['packages'] += 1
        require_declaration(owner, 'package', name, value)
    for name, value in data.get('tools', {}).items():
        counts['tools'] += 1
        require_declaration(owner, 'tool', name, value)
    for target, specification in data.get('dotfiles', {}).items():
        counts['dotfiles'] += 1
        require_dotfile(owner, target, specification)
    flatten_settings(owner, data.get('settings', {}))
    for domain, values in bootstrap.get('macos', {}).get('defaults', {}).items():
        for key, value in values.items():
            counts['preferences'] += 1
            require_preference(owner, domain, key, (), value)
    for entry in bootstrap.get('macos', {}).get('defaults_entries', []):
        counts['preferences'] += 1
        require_preference(owner, entry['domain'], entry['key'], entry['path'], entry['value'])
    for key, value in bootstrap.get('macos', {}).get('dock', {}).items():
        assert key in dock_keys, key
        counts['preferences'] += 1
        mapped = {'show_recents': 'show-recents', 'mru_spaces': 'mru-spaces'}.get(key, key)
        require_preference(owner, 'com.apple.dock', mapped, (), value)
    for key, value in bootstrap.get('macos', {}).get('keyboard', {}).items():
        assert key in keyboard_keys, key
        counts['preferences'] += 1
        mapped = {
            'key_repeat': 'KeyRepeat',
            'initial_key_repeat': 'InitialKeyRepeat',
            'press_and_hold': 'ApplePressAndHoldEnabled',
        }[key]
        require_preference(owner, 'NSGlobalDomain', mapped, (), value)
    for destination, value in bootstrap.get('repos', {}).items():
        counts['repositories'] += 1
        require_repository(owner, destination, value)
    login_items = bootstrap.get('macos', {}).get('login_items', {})
    if login_items:
        if not isinstance(login_items, dict) or set(login_items) != {'apps', 'exclusive'}:
            mismatch('login item configuration', owner, 'bootstrap.macos.login_items', 'expected only apps and exclusive')
            continue
        paths = login_items['apps']
        if not isinstance(paths, list) or not all(isinstance(path, str) for path in paths):
            mismatch('login item configuration', owner, 'apps', 'expected a list of application paths')
            continue
        for path in paths:
            counts['login_items'] += 1
            require_login_item(owner, path)
        require_login_policy(owner, paths, login_items['exclusive'])
    for hook_name, hook in bootstrap.get('hooks', {}).items():
        commands = hook.get('run', [])
        if isinstance(commands, str):
            commands = [commands]
        for command in commands:
            for match in re.finditer(
                r'/opt/homebrew/bin/brew install(?P<cask> --cask)? (?P<package>[^\s;]+)',
                command,
            ):
                require_hook(
                    owner,
                    hook_name,
                    hook.get('description', ''),
                    match.group('package'),
                    match.group('cask') is not None,
                    match.group(0),
                )

for executable in sorted((root / 'bin').glob('*')):
    if executable.is_file():
        counts['executables'] += 1
        resource = str(executable.relative_to(root))
        if resource not in executable_records:
            mismatch('executable', 'Active Git checkout', resource, 'no exact resource record')

for kind, count in counts.items():
    print(f'{kind}={count}')
if matrix_errors:
    print('Malformed migration-matrix records:', file=sys.stderr)
    for item in sorted(set(matrix_errors)):
        print(item, file=sys.stderr)
if missing:
    print('Missing or mismatched exact migration-matrix records:', file=sys.stderr)
    for item in sorted(set(missing)):
        print(item, file=sys.stderr)
if matrix_errors or missing:
    raise SystemExit(1)
print('current_inventory=PASS')
