#!/usr/bin/python3
import os
import pathlib
import shlex
import shutil
import subprocess
import sys

home = pathlib.Path.home()
mise = home / '.local/bin/mise'
source_config = home / '.config/mise'
root = pathlib.Path('/tmp/essential-mise-project-boundaries')
evidence = pathlib.Path('/tmp/essential-mise-evidence/project-boundaries')
shutil.rmtree(root, ignore_errors=True)
shutil.rmtree(evidence, ignore_errors=True)
evidence.mkdir(parents=True)
(evidence / 'report.txt').write_text('IN_PROGRESS\n')


def record_failure(exception_type, exception, traceback):
    (evidence / 'report.txt').write_text(f'FAIL\n{exception_type.__name__}: {exception}\n')


sys.excepthook = record_failure
env = {
    **os.environ,
    'PATH': f'{mise.parent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
}
env.pop('MISE_ENV', None)
env.pop('MISE_GLOBAL_CONFIG_FILE', None)


def run(name, arguments, cwd, custom_env=None):
    result = subprocess.run(
        [str(argument) for argument in arguments],
        cwd=cwd,
        env=custom_env or env,
        capture_output=True,
        text=True,
    )
    output = result.stdout + result.stderr
    (evidence / f'{name}.log').write_text(output)
    (evidence / f'{name}.exit').write_text(f'{result.returncode}\n')
    result.check_returncode()
    return output


run('workspace-ignore', ['git', '-C', source_config, 'check-ignore', '-v', 'config.workspaces.local.toml'], source_config)
project = root / 'project'
project.mkdir(parents=True)
(project / '.miserc.toml').write_text('env = ["development"]\n')
loaded = run('project-selector', [mise, 'config'], project)
assert 'config.development.toml' in loaded and 'config.ai-cli.toml' not in loaded
(project / '.miserc.local.toml').write_text('env = ["ai-cli"]\n')
loaded = run('project-local-selector', [mise, 'config'], project)
assert 'config.ai-cli.toml' in loaded and 'config.development.toml' not in loaded
loaded = run('cli-selector-override', [mise, '-E', 'terminal', 'config'], project)
assert 'config.terminal.toml' in loaded and 'config.ai-cli.toml' not in loaded

(project / 'mise.local.toml').write_text('''[env]
ESSENTIAL_MISE_SYNTHETIC_PROJECT = "isolated"
[bootstrap.repos."/private/tmp/essential-mise-project-only"]
url = "https://example.invalid/project-only.git"
''')
run('trust-project-fixture', [mise, 'trust', project / 'mise.local.toml'], project)
project_preview = run('project-resource-preview', [mise, '-E', 'foundation', 'bootstrap', '--dry-run'], project)
assert 'essential-mise-project-only' in project_preview
unrelated = root / 'unrelated'
unrelated.mkdir()
unrelated_preview = run('unrelated-resource-preview', [mise, '-E', 'foundation', 'bootstrap', '--dry-run'], unrelated)
assert 'essential-mise-project-only' not in unrelated_preview
project_env = run('project-env', [mise, 'env', 'ESSENTIAL_MISE_SYNTHETIC_PROJECT'], project)
assert 'isolated' in project_env
unrelated_env = run('unrelated-env', [mise, 'env', 'ESSENTIAL_MISE_SYNTHETIC_PROJECT'], unrelated)
assert 'isolated' not in unrelated_env

workspace_config = root / 'workspace-config'
workspace_config.mkdir()
for name in ('config.toml', 'config.foundation.toml'):
    shutil.copy2(source_config / name, workspace_config / name)
(workspace_config / 'config.workspaces.local.toml').write_text('''[bootstrap.repos."/private/tmp/essential-mise-synthetic-workspace"]
url = "https://example.invalid/synthetic-workspace.git"
''')
workspace_env = {**env, 'MISE_CONFIG_DIR': str(workspace_config)}
run('trust-workspace-fixture', [mise, 'trust', workspace_config / 'config.workspaces.local.toml'], unrelated, workspace_env)
workspace_preview = run('workspace-selected', [mise, '-E', 'workspaces', 'bootstrap', '--dry-run'], unrelated, workspace_env)
assert 'essential-mise-synthetic-workspace' in workspace_preview
workspace_unselected = run('workspace-unselected', [mise, 'bootstrap', '--dry-run'], unrelated, workspace_env)
assert 'essential-mise-synthetic-workspace' not in workspace_unselected

launcher_config = root / 'launcher config with spaces'
run('add-launcher-worktree', ['git', '-C', source_config, 'worktree', 'add', '--detach', launcher_config, 'HEAD'], source_config)
launcher_env = {**env, 'MISE_CONFIG_DIR': str(launcher_config)}
launcher_output = run(
    'bootstrap-next-steps',
    ['/bin/bash', source_config / 'bootstrap.sh'],
    project,
    launcher_env,
)
next_step_lines = [line.strip() for line in launcher_output.splitlines() if line.startswith('  mise ')]
assert len(next_step_lines) == 3, next_step_lines
for index, line in enumerate(next_step_lines):
    command = shlex.split(line)
    assert command[:2] == ['mise', '-C'], command
    assert command[2] == str(launcher_config), command
    if '--dry-run' not in command:
        command.append('--dry-run')
    preview = run(f'next-step-scope-{index}', command, project, launcher_env)
    assert 'essential-mise-project-only' not in preview

report = (
    'project_selector_precedence=PASS\nproject_resource_isolation=PASS\n'
    'project_env_isolation=PASS\nworkspace_ignore_and_activation=PASS\n'
    'bootstrap_next_step_scope=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
