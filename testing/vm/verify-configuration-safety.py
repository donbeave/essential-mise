#!/usr/bin/python3
import argparse
import os
import pathlib
import shutil
import subprocess
import sys

parser = argparse.ArgumentParser(description='Verify that dotfile merges preserve synthetic user configuration.')
parser.add_argument('--config-dir', default=os.environ.get('ESSENTIAL_MISE_CONFIG_DIR'), help='config repo checkout inside the guest')
parser.add_argument('--env-git', help='module selection that applies git dotfiles; omit to skip the git section')
parser.add_argument('--git-ignore-source', help='path inside the config dir expected as core.excludesFile')
parser.add_argument('--expect', action='append', default=[], metavar='KEY=VALUE', help='git setting expected after repeated runs; repeatable')
parser.add_argument('--env-editor', help='module selection that merges an editor settings file; omit to skip')
parser.add_argument('--editor-settings-path', help='settings file relative to HOME, for example .config/editor/settings.json')
parser.add_argument('--editor-absent', action='append', default=[], metavar='TEXT', help='text that must be absent after the merge; repeatable')
args = parser.parse_args()
if not args.config_dir:
    parser.error('--config-dir or ESSENTIAL_MISE_CONFIG_DIR is required')
if args.env_editor and not args.editor_settings_path:
    parser.error('--env-editor requires --editor-settings-path')
config_dir = pathlib.Path(args.config_dir).expanduser()
mise = pathlib.Path.home() / '.local/bin/mise'
root = pathlib.Path('/tmp/essential-mise-configuration-safety')
evidence = pathlib.Path('/tmp/essential-mise-evidence/configuration-safety')
shutil.rmtree(root, ignore_errors=True)
shutil.rmtree(evidence, ignore_errors=True)
evidence.mkdir(parents=True)
(evidence / 'report.txt').write_text('IN_PROGRESS\n')


def record_failure(exception_type, exception, traceback):
    (evidence / 'report.txt').write_text(f'FAIL\n{exception_type.__name__}: {exception}\n')


sys.excepthook = record_failure
base_env = {
    **os.environ,
    'PATH': f'{mise.parent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'GIT_CONFIG_NOSYSTEM': '1',
}


def run(name, arguments, env, cwd=None):
    result = subprocess.run(
        [str(argument) for argument in arguments],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
    )
    (evidence / f'{name}.stdout').write_text(result.stdout)
    (evidence / f'{name}.stderr').write_text(result.stderr)
    (evidence / f'{name}.exit').write_text(f'{result.returncode}\n')
    result.check_returncode()
    return result.stdout


def git_settings(home, xdg=None):
    env = {**base_env, 'HOME': str(home)}
    if xdg is None:
        env.pop('XDG_CONFIG_HOME', None)
    else:
        env['XDG_CONFIG_HOME'] = str(xdg)
    return env


home = root / 'git home'
home.mkdir(parents=True)
gitconfig = home / '.gitconfig'
gitconfig.write_text('''# synthetic user comment
[core]
    autocrlf = false
    sshCommand = ssh -i /synthetic/key
[credential "https://example.invalid"]
    helper = osxkeychain
    helper = /synthetic/helper
[includeIf "gitdir:/synthetic/"]
    path = /synthetic/include
[pull]
    rebase = true
    ff = only
''')
env = git_settings(home)
env['MISE_CONFIG_DIR'] = str(config_dir)
env['MISE_CEILING_PATHS'] = str(config_dir)
env.pop('MISE_ENV', None)
results = []
if args.env_git:
    for iteration in (1, 2):
        run(f'git-dotfiles-{iteration}', [mise, '-C', config_dir, '-E', args.env_git, 'bootstrap', '--only', 'dotfiles', '--yes'], env)
    expected_settings = dict(item.split('=', 1) for item in args.expect)
    if args.git_ignore_source:
        expected_settings['core.excludesFile'] = str(config_dir / args.git_ignore_source)
    for key, expected in expected_settings.items():
        actual = run(f'read-{key}', ['git', 'config', '--global', '--get-all', key], env).splitlines()
        assert actual == [expected], (key, actual, expected)
    assert run('read-ssh', ['git', 'config', '--global', '--get', 'core.sshCommand'], env).strip() == 'ssh -i /synthetic/key'
    helpers = run('read-helpers', ['git', 'config', '--global', '--get-all', 'credential.https://example.invalid.helper'], env).splitlines()
    assert helpers == ['osxkeychain', '/synthetic/helper'], helpers
    assert run('read-include', ['git', 'config', '--global', '--get', 'includeIf.gitdir:/synthetic/.path'], env).strip() == '/synthetic/include'
    assert '# synthetic user comment' in gitconfig.read_text()
    (evidence / 'gitconfig').write_text(gitconfig.read_text())
    results.append('git_dotfile_preservation=PASS')
else:
    results.append('git_dotfile_preservation=SKIP')

if args.env_git:
    xdg = root / 'custom xdg space'
    repository = root / 'ignore-repository'
    (repository / '.fixture').mkdir(parents=True)
    (repository / '.fixture/settings.local.json').write_text('{}\n')
    ignore_env = git_settings(home, xdg)
    run('git-init-ignore', ['git', 'init', '--quiet', repository], ignore_env)
    assert not (xdg / 'git/ignore').exists()
    run('custom-xdg-ignore', ['git', '-C', repository, 'check-ignore', '-v', '.fixture/settings.local.json'], ignore_env)
    results.append('custom_xdg_ignore=PASS')
else:
    results.append('custom_xdg_ignore=SKIP')

if args.env_editor:
    merge_home = root / 'merge-home'
    settings = merge_home / args.editor_settings_path
    settings.parent.mkdir(parents=True)
    settings.write_text('''{
  // synthetic user comment
  "user_setting": true,
  "theme": "UserTheme",
}
''')
    merge_env = git_settings(merge_home)
    merge_env['MISE_CONFIG_DIR'] = str(config_dir)
    merge_env.pop('MISE_ENV', None)
    run('editor-merge', [mise, '-C', config_dir, '-E', args.env_editor, 'bootstrap', '--only', 'dotfiles', '--yes'], merge_env)
    merged = settings.read_text()
    (evidence / 'editor-settings.jsonc').write_text(merged)
    assert '// synthetic user comment' in merged
    assert '"user_setting": true' in merged
    for text in args.editor_absent:
        assert text not in merged, text
    results.append('editor_jsonc_preservation=PASS')
else:
    results.append('editor_jsonc_preservation=SKIP')
report = '\n'.join(results) + '\n'
(evidence / 'report.txt').write_text(report)
print(report, end='')
