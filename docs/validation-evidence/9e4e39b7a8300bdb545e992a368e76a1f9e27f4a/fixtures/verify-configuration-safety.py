#!/usr/bin/python3
import json
import os
import pathlib
import shutil
import subprocess
import sys

config_dir = pathlib.Path.home() / '.config/mise'
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
[credential "https://github.com"]
    helper = osxkeychain
    helper = /synthetic/helper
[includeIf "gitdir:/synthetic/"]
    path = /synthetic/include
[pull]
    rebase = false
''')
env = git_settings(home)
commands = {
    'core.autocrlf': 'input',
    'core.excludesFile': str(config_dir / 'dotfiles/development/git/ignore'),
    'pull.rebase': 'false',
    'branch.autoSetupRebase': 'never',
    'fetch.prune': 'true',
    'merge.conflictStyle': 'zdiff3',
    'rerere.enabled': 'true',
    'push.autoSetupRemote': 'true',
    'init.defaultBranch': 'main',
    'alias.merge-main': '!git fetch origin && git merge --no-ff origin/main',
}
for iteration in (1, 2):
    for key, value in commands.items():
        run(f'git-{iteration}-{key}', ['git', 'config', '--global', key, value], env)
for key, expected in commands.items():
    actual = run(f'read-{key}', ['git', 'config', '--global', '--get', key], env).strip()
    assert actual == expected, (key, actual, expected)
assert run('read-ssh', ['git', 'config', '--global', '--get', 'core.sshCommand'], env).strip() == 'ssh -i /synthetic/key'
helpers = run('read-helpers', ['git', 'config', '--global', '--get-all', 'credential.https://github.com.helper'], env).splitlines()
assert helpers == ['osxkeychain', '/synthetic/helper'], helpers
assert run('read-include', ['git', 'config', '--global', '--get', 'includeIf.gitdir:/synthetic/.path'], env).strip() == '/synthetic/include'
assert '# synthetic user comment' in gitconfig.read_text()
(evidence / 'gitconfig').write_text(gitconfig.read_text())

xdg = root / 'custom xdg space'
repository = root / 'ignore-repository'
(repository / '.claude').mkdir(parents=True)
(repository / '.claude/settings.local.json').write_text('{}\n')
ignore_env = git_settings(home, xdg)
run('git-init-ignore', ['git', 'init', '--quiet', repository], ignore_env)
assert not (xdg / 'git/ignore').exists()
run('custom-xdg-ignore', ['git', '-C', repository, 'check-ignore', '-v', '.claude/settings.local.json'], ignore_env)

merge_home = root / 'merge-home'
zed = merge_home / '.config/zed/settings.json'
zed.parent.mkdir(parents=True)
zed.write_text('''{
  // synthetic user comment
  "vim_mode": true,
  "theme": "UserTheme",
}
''')
merge_env = git_settings(merge_home)
merge_env['MISE_CONFIG_DIR'] = str(config_dir)
merge_env.pop('MISE_ENV', None)
run('zed-merge', [mise, '-C', config_dir, '-E', 'development-editors', 'bootstrap', '--only', 'dotfiles', '--yes'], merge_env)
zed_text = zed.read_text()
(evidence / 'zed-settings.jsonc').write_text(zed_text)
assert '// synthetic user comment' in zed_text
assert '"vim_mode": true' in zed_text
assert '"theme": "UserTheme"' not in zed_text
assert '"agent"' not in zed_text and '"agent_servers"' not in zed_text
report = (
    'git_repeated_cli_preservation=PASS\ncustom_xdg_ignore=PASS\n'
    'zed_jsonc_preservation=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
