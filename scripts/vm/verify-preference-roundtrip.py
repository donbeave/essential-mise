#!/usr/bin/python3
import os
import pathlib
import plistlib
import re
import shutil
import subprocess
import sys

config_dir = pathlib.Path.home() / '.config/mise'
mise = pathlib.Path.home() / '.local/bin/mise'
root = pathlib.Path('/private/tmp/essential-mise-preference-roundtrip')
evidence = pathlib.Path('/tmp/essential-mise-evidence/preference-roundtrip')
document = config_dir / 'docs/manual-actions.md'
shutil.rmtree(root, ignore_errors=True)
shutil.rmtree(evidence, ignore_errors=True)
root.mkdir(parents=True)
evidence.mkdir(parents=True)
(evidence / 'report.txt').write_text('IN_PROGRESS\n')


def record_failure(exception_type, exception, traceback):
    (evidence / 'report.txt').write_text(f'FAIL\n{exception_type.__name__}: {exception}\n')


sys.excepthook = record_failure
text = document.read_text()
blocks = re.findall(r'```bash\n(.*?)```', text, re.S)
capture_matches = [block for block in blocks if 'capture_preferences()' in block and 'defaults export' in block]
restore_matches = [block for block in blocks if 'restore_preferences()' in block and 'defaults import' in block]
assert len(capture_matches) == 1, len(capture_matches)
assert len(restore_matches) == 1, len(restore_matches)
capture_block = capture_matches[0]
restore_block = restore_matches[0]


def run(name, arguments, env=None, expected=0, timeout=120):
    result = subprocess.run(
        [str(argument) for argument in arguments],
        capture_output=True,
        text=True,
        env=env,
        timeout=timeout,
    )
    (evidence / f'{name}.stdout').write_text(result.stdout)
    (evidence / f'{name}.stderr').write_text(result.stderr)
    (evidence / f'{name}.exit').write_text(f'{result.returncode}\n')
    if result.returncode != expected:
        raise AssertionError(f'{name} exited {result.returncode}, expected {expected}: {result.stderr}')
    return result.stdout


def captured_path(output):
    matches = re.findall(r'^capture_dir=(.+)$', output, re.M)
    assert len(matches) == 1, output
    return pathlib.Path(matches[0])


stub_bin = root / 'stub-bin'
stub_bin.mkdir()
stub_state = root / 'stub-state'
stub_state.mkdir()
stub_log = root / 'stub-defaults.log'
stub = stub_bin / 'defaults'
stub.write_text('''#!/bin/bash
set -u
printf '%s\n' "$*" >> "$STUB_LOG"
action="$1"
domain="$2"
case " ${STUB_READ_ERROR_DOMAINS:-} " in
  *" $domain "*) [[ "$action" == 'read' ]] && { printf 'synthetic read failure\n' >&2; exit 1; } ;;
esac
case " ${STUB_ABSENT_DOMAINS:-} " in
  *" $domain "*) printf "Error: Domain '%s' not found.\n" "$domain" >&2; exit 1 ;;
esac
case " ${STUB_FAIL_EXPORT_DOMAINS:-} " in
  *" $domain "*) [[ "$action" == 'export' ]] && { printf 'synthetic export failure\n' >&2; exit 1; } ;;
esac
case " ${STUB_FAIL_IMPORT_DOMAINS:-} " in
  *" $domain "*) [[ "$action" == 'import' ]] && { printf 'synthetic import failure\n' >&2; exit 1; } ;;
esac
case "$action" in
  read) exit 0 ;;
  export) printf '<plist><dict/></plist>\n' > "$3" ;;
  import) /bin/cp "$3" "$STUB_STATE/$domain.current.plist" ;;
  delete) /bin/rm -f "$STUB_STATE/$domain.current.plist" ;;
  *) exit 2 ;;
esac
''')
stub.chmod(0o755)


def stub_env(**values):
    env = {
        **os.environ,
        'PATH': f'{stub_bin}:/usr/bin:/bin',
        'STUB_LOG': str(stub_log),
        'STUB_STATE': str(stub_state),
    }
    env.update({key: value for key, value in values.items() if value is not None})
    return env


success_env = stub_env(STUB_ABSENT_DOMAINS='NSGlobalDomain')
capture_output = run('stub-existing-absent-capture', ['/bin/zsh', '-c', capture_block], success_env)
capture_dir = captured_path(capture_output)
assert (capture_dir / 'NSGlobalDomain.before.state').read_text() == 'absent\n'
assert (capture_dir / 'com.apple.dock.before.state').read_text() == 'present\n'
assert (capture_dir / 'com.apple.dock.before.plist').stat().st_size > 0
restore_output = run('stub-existing-absent-restore', ['/bin/zsh', '-c', restore_block], {**success_env, 'capture_dir': str(capture_dir)})
assert 'capture_dir=' not in restore_output
stub_log_text = stub_log.read_text()
assert 'import NSGlobalDomain' not in stub_log_text
assert 'import com.apple.dock' in stub_log_text

run(
    'stub-failed-export',
    ['/bin/bash', '-c', capture_block],
    stub_env(STUB_FAIL_EXPORT_DOMAINS='NSGlobalDomain'),
    expected=1,
)
run(
    'stub-failed-read',
    ['/bin/bash', '-c', capture_block],
    stub_env(STUB_READ_ERROR_DOMAINS='com.apple.dock'),
    expected=1,
)
failed_import_env = stub_env(STUB_FAIL_IMPORT_DOMAINS='com.apple.dock')
capture_output = run('stub-before-failed-import', ['/bin/bash', '-c', capture_block], failed_import_env)
failed_import_dir = captured_path(capture_output)
run(
    'stub-failed-import',
    ['/bin/bash', '-c', restore_block],
    {**failed_import_env, 'capture_dir': str(failed_import_dir)},
    expected=1,
)

real_env = {
    **os.environ,
    'PATH': f'{mise.parent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'MISE_CEILING_PATHS': str(config_dir),
}
for key, value in (
    ('AppleFirstWeekday', 'syntheticKeep'),
    ('AppleICUDateFormatStrings', 'syntheticKeep'),
):
    run(f'sentinel-{key}', ['defaults', 'write', 'NSGlobalDomain', key, '-dict-add', value, '-string', 'preserve'], real_env)
run('sentinel-top-level', ['defaults', 'write', 'NSGlobalDomain', 'EssentialMiseRoundtrip', '-string', 'preserve'], real_env)
real_capture = run('real-capture', ['/bin/bash', '-c', capture_block], real_env)
real_capture_dir = captured_path(real_capture)
apply_error = None
restore_error = None
try:
    run(
        'real-apply',
        [mise, '-C', config_dir, '-E', 'system-preferences', 'bootstrap', '--yes'],
        real_env,
        timeout=300,
    )
    run(
        'real-status',
        [mise, '-C', config_dir, '-E', 'system-preferences', 'bootstrap', 'macos', 'defaults', 'status', '--missing'],
        real_env,
    )
    for key in ('AppleFirstWeekday', 'AppleICUDateFormatStrings'):
        current = run(f'current-{key}', ['defaults', 'read', 'NSGlobalDomain', key], real_env)
        assert 'syntheticKeep' in current and 'preserve' in current, current
    top_level = run('current-top-level', ['defaults', 'read', 'NSGlobalDomain', 'EssentialMiseRoundtrip'], real_env)
    assert 'preserve' in top_level, top_level
    run(
        'post-capture-marker',
        ['defaults', 'write', 'NSGlobalDomain', 'EssentialMisePostCapture', '-string', 'remove-on-restore'],
        real_env,
    )
except Exception as error:
    apply_error = error
finally:
    try:
        run('real-restore', ['/bin/bash', '-c', restore_block], {**real_env, 'capture_dir': str(real_capture_dir)})
    except Exception as error:
        restore_error = error
if apply_error is not None:
    raise apply_error
if restore_error is not None:
    raise restore_error

for state_file in sorted(real_capture_dir.glob('*.before.state')):
    domain = state_file.name[:-len('.before.state')]
    state = state_file.read_text().strip()
    if state == 'present':
        after = evidence / f'{domain}.after.plist'
        run(f'export-after-{domain}', ['defaults', 'export', domain, after], real_env)
        before = real_capture_dir / f'{domain}.before.plist'
        assert plistlib.loads(before.read_bytes()) == plistlib.loads(after.read_bytes()), f'Round-trip differs for {domain}'
    elif state == 'absent':
        run(f'absent-after-{domain}', ['defaults', 'read', domain], real_env, expected=1)
        error_text = (evidence / f'absent-after-{domain}.stderr').read_text()
        assert f"Domain '{domain}' not found." in error_text, error_text
    else:
        raise AssertionError(f'Invalid state for {domain}: {state}')
for key in ('AppleFirstWeekday', 'AppleICUDateFormatStrings'):
    restored = run(f'restored-{key}', ['defaults', 'read', 'NSGlobalDomain', key], real_env)
    assert 'syntheticKeep' in restored and 'preserve' in restored, restored
restored_top_level = run('restored-top-level', ['defaults', 'read', 'NSGlobalDomain', 'EssentialMiseRoundtrip'], real_env)
assert 'preserve' in restored_top_level, restored_top_level
run('post-capture-removed', ['defaults', 'read', 'NSGlobalDomain', 'EssentialMisePostCapture'], real_env, expected=1)

report = (
    'stub_existing_absent=PASS\nstub_failed_export=PASS\nstub_failed_read=PASS\n'
    'stub_failed_import=PASS\nreal_capture_apply_restore=PASS\n'
    'roundtrip_compare=PASS\nunrelated_content_preservation=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
