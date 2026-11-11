#!/usr/bin/python3
import argparse
import os
import pathlib
import shlex
import shutil
import subprocess
import sys

home = pathlib.Path.home()
mise = home / '.local/bin/mise'
parser = argparse.ArgumentParser(description='Verify that project and workspace configuration stays isolated from the config repo.')
parser.add_argument('--config-dir', default=os.environ.get('ESSENTIAL_MISE_CONFIG_DIR', str(home / '.config/mise')), help='config repo checkout inside the guest')
parser.add_argument('--bootstrap', required=True, help='path to the engine bootstrap.sh inside the guest')
parser.add_argument('--env-a', default='foundation', help='module selected by the project .miserc.toml')
parser.add_argument('--env-b', default='terminal', help='module selected by the project .miserc.local.toml')
parser.add_argument('--env-c', default='foundation', help='module selected on the command line')
parser.add_argument('--base-modules', default='config.toml,config.foundation.toml', help='comma-separated files copied into the workspace fixture')
parser.add_argument('--env-workspaces', help='optional workspace module name; the workspace section is skipped when omitted')
args = parser.parse_args()
source_config = pathlib.Path(args.config_dir).expanduser()
bootstrap_script = pathlib.Path(args.bootstrap).expanduser()
env_a, env_b, env_c = args.env_a, args.env_b, args.env_c
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


def run(name, arguments, cwd, custom_env=None, expected=0):
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
    if result.returncode != expected:
        raise AssertionError(f'{name} exited {result.returncode}, expected {expected}: {output}')
    return output


if args.env_workspaces:
    run('workspace-ignore', ['git', '-C', source_config, 'check-ignore', '-v', f'config.{args.env_workspaces}.local.toml'], source_config)
project = root / 'project'
project.mkdir(parents=True)
(project / '.miserc.toml').write_text(f'env = ["{env_a}"]\n')
loaded = run('project-selector', [mise, 'config'], project)
assert f'config.{env_a}.toml' in loaded and f'config.{env_b}.toml' not in loaded
(project / '.miserc.local.toml').write_text(f'env = ["{env_b}"]\n')
loaded = run('project-local-selector', [mise, 'config'], project)
assert f'config.{env_b}.toml' in loaded and f'config.{env_a}.toml' not in loaded
loaded = run('cli-selector-override', [mise, '-E', env_c, 'config'], project)
assert f'config.{env_c}.toml' in loaded and f'config.{env_b}.toml' not in loaded

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

if args.env_workspaces:
    workspace_config = root / 'workspace-config'
    workspace_config.mkdir()
    for name in args.base_modules.split(','):
        shutil.copy2(source_config / name, workspace_config / name)
    (workspace_config / f'config.{args.env_workspaces}.local.toml').write_text(f'''[bootstrap.repos."/private/tmp/essential-mise-synthetic-workspace"]
    url = "https://example.invalid/synthetic-workspace.git"
    ''')
    workspace_env = {**env, 'MISE_CONFIG_DIR': str(workspace_config)}
    run('trust-workspace-fixture', [mise, 'trust', workspace_config / f'config.{args.env_workspaces}.local.toml'], unrelated, workspace_env)
    workspace_preview = run('workspace-selected', [mise, '-E', args.env_workspaces, 'bootstrap', '--dry-run'], unrelated, workspace_env)
    assert 'essential-mise-synthetic-workspace' in workspace_preview
    workspace_unselected = run('workspace-unselected', [mise, 'bootstrap', '--dry-run'], unrelated, workspace_env)
    assert 'essential-mise-synthetic-workspace' not in workspace_unselected

nested_config = project / 'nested config'
original_origin = run('origin-before', ['git', '-C', source_config, 'remote', 'get-url', 'origin'], source_config).strip()
nested_script = bootstrap_script
nested_command = ['/bin/bash', nested_script, 'bootstrap', '--config', source_config]
nested_env = {**env, 'MISE_CONFIG_DIR': str(nested_config)}
colon_config = root / 'essential:mise'
colon_output = run(
    'colon-config-refusal',
    nested_command,
    project,
    {**nested_env, 'MISE_CONFIG_DIR': str(colon_config)},
    expected=1,
)
assert 'MISE_CONFIG_DIR cannot contain a colon' in colon_output
assert not colon_config.exists()
colon_tmpdir = root / 'tmp:colon'
colon_tmpdir.mkdir()
colon_tmp_output = run(
    'colon-tmpdir-refusal',
    nested_command,
    project,
    {**nested_env, 'TMPDIR': str(colon_tmpdir)},
    expected=1,
)
assert 'Temporary adoption directory cannot contain a colon' in colon_tmp_output
assert not nested_config.exists()
first_adoption = run('nested-first-adoption', nested_command, project, nested_env)
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
    ['git', '-C', nested_config, 'remote', 'set-url', 'origin', original_origin],
    nested_config,
)
nested_rerun = run('nested-foundation-rerun', nested_command, project, nested_env)
assert 'essential-mise-project-only' not in nested_rerun
assert 'essential-mise-parent-hook' not in nested_rerun
assert not parent_marker.exists()

report = (
    'project_selector_precedence=PASS\nproject_resource_isolation=PASS\n'
    'project_env_isolation=PASS\n'
    f'workspace_ignore_and_activation={"PASS" if args.env_workspaces else "SKIP"}\n'
    'nested_adoption_isolation=PASS\n'
    'nested_foundation_rerun_isolation=PASS\ncolon_config_refusal=PASS\n'
    'colon_tmpdir_refusal=PASS\n'
)
(evidence / 'report.txt').write_text(report)
print(report, end='')
