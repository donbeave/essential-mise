#!/usr/bin/python3
import os
import pathlib
import stat
import sys
import tempfile

BLOCKS = {
    'activate': '''if status is-interactive
    mise activate fish | source
else
    mise activate fish --shims | source
end''',
    'amp': '''if status is-interactive
    command -q amp; and amp completions fish | source
end''',
}


class FishConfigError(Exception):
    pass


def target_path():
    xdg = os.environ.get('XDG_CONFIG_HOME')
    root = pathlib.Path(xdg) if xdg else pathlib.Path.home() / '.config'
    if not root.is_absolute():
        raise FishConfigError('XDG_CONFIG_HOME must be absolute')
    return root / 'fish/config.fish'


def markers(name):
    return (
        f'# >>> mise:{name} >>> managed by mise — do not edit between markers',
        f'# <<< mise:{name} <<<',
    )


def managed_lines(name):
    opening, closing = markers(name)
    return [opening, *BLOCKS[name].splitlines(), closing]


def block_range(lines, name):
    opening, closing = markers(name)
    starts = [index for index, line in enumerate(lines) if line.strip().startswith(f'# >>> mise:{name} >>>')]
    ends = [index for index, line in enumerate(lines) if line.strip() == closing]
    if not starts and not ends:
        return None
    if len(starts) != 1 or len(ends) != 1 or starts[0] >= ends[0]:
        raise FishConfigError(f'Fish config has incomplete or duplicate mise:{name} markers')
    if lines[starts[0]].strip() != opening:
        raise FishConfigError(f'Fish config has an unrecognized mise:{name} opening marker')
    return starts[0], ends[0] + 1


def read_target(path):
    if not path.exists():
        return [], None
    resolved = path.resolve(strict=True)
    if not resolved.is_file():
        raise FishConfigError(f'Fish config is not a regular file: {path}')
    return resolved.read_text().splitlines(), stat.S_IMODE(resolved.stat().st_mode)


def write_target(path, lines, mode):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    destination = path.resolve(strict=False)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile('w', encoding='utf-8', dir=destination.parent, prefix='.config.fish.', delete=False) as output:
            temporary = pathlib.Path(output.name)
            output.write('\n'.join(lines) + '\n')
        os.chmod(temporary, mode if mode is not None else 0o600)
        os.replace(temporary, destination)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def apply(path, name):
    lines, mode = read_target(path)
    replacement = managed_lines(name)
    current = block_range(lines, name)
    if current is None:
        if lines:
            lines.extend([''] if lines[-1] else [])
        lines.extend(replacement)
    else:
        start, end = current
        lines[start:end] = replacement
    write_target(path, lines, mode)


def unapply(path, name):
    if not path.exists():
        return
    lines, mode = read_target(path)
    current = block_range(lines, name)
    if current is None:
        return
    start, end = current
    del lines[start:end]
    write_target(path, lines, mode)


def status(path, name):
    lines, _ = read_target(path)
    current = block_range(lines, name)
    if current is None:
        raise FishConfigError(f'Fish config is missing mise:{name}: {path}')
    start, end = current
    if lines[start:end] != managed_lines(name):
        raise FishConfigError(f'Fish config block mise:{name} differs: {path}')
    print(f'{name}=applied path={path}')


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in {'apply', 'unapply', 'status'} or sys.argv[2] not in BLOCKS:
        print(f'Usage: {sys.argv[0]} <apply|unapply|status> <activate|amp>', file=sys.stderr)
        return 2
    action, name = sys.argv[1:]
    path = target_path()
    if action == 'apply':
        apply(path, name)
    elif action == 'unapply':
        unapply(path, name)
    else:
        status(path, name)
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (FishConfigError, OSError, UnicodeError) as error:
        print(f'ERROR: {error}', file=sys.stderr)
        raise SystemExit(1)
