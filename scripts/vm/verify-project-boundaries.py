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
root = pathlib.Path('/private/tmp/essential-mise-project-boundaries')
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
env.pop('MISE_CEILING_PATHS', None)


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

parent_marker = pathlib.Path('/private/tmp/essential-mise-parent-hook')
parent_marker.unlink(missing_ok=True)
(project / 'mise.local.toml').write_text('''[env]
ESSENTIAL_MISE_SYNTHETIC_PROJECT = "isolated"
[bootstrap.repos."/private/tmp/essential-mise-project-only"]
url = "https://example.invalid/project-only.git"
[bootstrap.hooks.post-dotfiles]
run = "/usr/bin/python3 -c 'from pathlib import Path; Path(\\"/private/tmp/essential-mise-parent-hook\\").write_text(\\"unexpected\\")'"
''')
run('trust-project-fixture', [mise, 'trust', project / 'mise.local.toml'], project)
project_preview = run('project-resource-preview', [mise, '-E', 'foundation', 'bootstrap', '--dry-run'], project)
assert 'essential-mise-project-only' in project_preview
assert 'essential-mise-parent-hook' in project_preview
unrelated = root / 'unrelated'
unrelated.mkdir()
unrelated_preview = run('unrelated-resource-preview', [mise, '-E', 'foundation', 'bootstrap', '--dry-run'], unrelated)
assert 'essential-mise-project-only' not in unrelated_preview
assert 'essential-mise-parent-hook' not in unrelated_preview
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
run('add-launcher-worktree', ['git', '-C', source_config, 'worktree', 'add', '--force', '--detach', launcher_config, 'HEAD'], source_config)
launcher_env = {**env, 'MISE_CONFIG_DIR': str(launcher_config)}
launcher_output = run(
    'bootstrap-next-steps',
    ['/bin/bash', source_config / 'bootstrap.sh'],
    project,
    launcher_env,
)
next_step_lines = [line.strip() for line in launcher_output.splitlines() if line.startswith('  MISE_CEILING_PATHS=')]
assert len(next_step_lines) == 3, next_step_lines
for index, line in enumerate(next_step_lines):
    command = shlex.split(line)
    assert command.pop(0) == f'MISE_CEILING_PATHS={launcher_config}', command
    assert command[:2] == ['mise', '-C'], command
    assert command[2] == str(launcher_config), command
    if '--dry-run' not in command:
        command.append('--dry-run')
    preview_env = {**launcher_env, 'MISE_CEILING_PATHS': str(launcher_config)}
    preview = run(f'next-step-scope-{index}', command, project, preview_env)
    assert 'essential-mise-project-only' not in preview
    assert 'essential-mise-parent-hook' not in preview
assert not parent_marker.exists()

nested_config = project / 'nested config'
nested_script = root / 'bootstrap-nested.sh'
launcher_source = (source_config / 'bootstrap.sh').read_text()
repository_declaration = "REPO='donbeave/essential-mise'"
assert launcher_source.count(repository_declaration) == 1
nested_script.write_text(launcher_source.replace(repository_declaration, f"REPO='{source_config}'"))
nested_env = {**env, 'MISE_CONFIG_DIR': str(nested_config)}
first_adoption = run('nested-first-adoption', ['/bin/bash', nested_script], project, nested_env)
assert 'essential-mise-project-only' not in first_adoption
assert 'essential-mise-parent-hook' not in first_adoption
assert not parent_marker.exists()

unbounded = run(
    'nested-unbounded-control',
    [mise, '-C', nested_config, '-E', 'foundation', 'bootstrap', '--dry-run'],
    nested_config,
    nested_env,
)
assert 'essential-mise-project-only' in unbounded
assert 'essential-mise-parent-hook' in unbounded
run(
    'nested-set-expected-origin',
    ['git', '-C', nested_config, 'remote', 'set-url', 'origin', 'https://github.com/donbeave/essential-mise.git'],
    nested_config,
)
nested_rerun = run('nested-foundation-rerun', ['/bin/bash', nested_script], project, nested_env)
assert 'essential-mise-project-only' not in nested_rerun
assert 'essential-mise-parent-hook' not in nested_rerun
assert not parent_marker.exists()

report = (
    'project_selector_precedence=PASS\nproject_resource_isolation=PASS\n'
    'project_env_isolation=PASS\nworkspace_ignore_and_activation=PASS\n'
    'bootstrap_next_step_scope=PASS\nnested_adoption_isolation=PASS\n'
    'nested_foundation_rerun_isolation=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
