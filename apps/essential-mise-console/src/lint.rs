//! `lint`: check a config repository against the contract in
//! `docs/config-repo.md`. Pure filesystem reads; one problem per line.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::manifest::{MANIFEST_FILE, Manifest};

/// Check `config_dir`; returns one human-readable line per problem
/// (empty when the repository is valid). Output order is deterministic.
pub fn lint(config_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();

    let manifest = match Manifest::load(config_dir) {
        Ok(manifest) => Some(manifest),
        Err(e) => {
            problems.push(e.to_string());
            None
        }
    };

    let files = module_files(config_dir);
    if let Some(manifest) = &manifest {
        check_modules(manifest, &files, &mut problems);
    }
    check_toml_files(config_dir, &mut problems);
    check_dotfile_sources(config_dir, &files, &mut problems);
    check_gitignore(config_dir, &mut problems);
    problems
}

/// Names of top-level `config.<name>.toml` files that count as modules:
/// `config.toml` and `*.local.toml` are excluded.
fn module_files(config_dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(config_dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        if file.ends_with(".local.toml") {
            continue;
        }
        if let Some(name) = file
            .strip_prefix("config.")
            .and_then(|rest| rest.strip_suffix(".toml"))
            && !name.is_empty()
            && entry.path().is_file()
        {
            names.insert(name.to_string());
        }
    }
    names
}

fn check_modules(manifest: &Manifest, files: &BTreeSet<String>, problems: &mut Vec<String>) {
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    for module in &manifest.modules {
        listed.insert(module.name.as_str());
        if !files.contains(&module.name) {
            problems.push(format!(
                "module \"{0}\" is listed in {MANIFEST_FILE} but config.{0}.toml does not exist",
                module.name
            ));
        }
    }
    for base in &manifest.setup.base_modules {
        listed.insert(base.as_str());
        if !files.contains(base) {
            problems.push(format!(
                "base module \"{0}\" is listed in {MANIFEST_FILE} but config.{0}.toml does not exist",
                base
            ));
        }
    }
    for name in files {
        if !listed.contains(name.as_str()) {
            problems.push(format!(
                "config.{name}.toml is not listed in {MANIFEST_FILE} (add a [[modules]] entry or a base module)"
            ));
        }
    }
}

/// Every `*.toml` under the repository parses. Hidden directories,
/// `node_modules`, and `target` are not searched.
fn check_toml_files(config_dir: &Path, problems: &mut Vec<String>) {
    let mut paths = Vec::new();
    collect_toml(config_dir, &mut paths);
    paths.sort();
    for path in paths {
        let rel = relative(config_dir, &path);
        // The manifest is reported by the strict check above.
        if rel == MANIFEST_FILE {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if let Err(e) = toml::from_str::<toml::Table>(&text) {
                    problems.push(format!(
                        "{rel}: invalid TOML: {}",
                        first_line(&e.to_string())
                    ));
                }
            }
            Err(e) => problems.push(format!("{rel}: cannot read: {e}")),
        }
    }
}

fn collect_toml(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                collect_toml(&path, out);
            }
        } else if name.ends_with(".toml") {
            out.push(path);
        }
    }
}

/// Every `[dotfiles]` entry with a repo-relative `source` points at an
/// existing path. Looks at top-level `config*.toml` files; sources starting
/// with `~` or `/`, or containing a template expression, are skipped.
fn check_dotfile_sources(
    config_dir: &Path,
    module_names: &BTreeSet<String>,
    problems: &mut Vec<String>,
) {
    let mut files: Vec<String> = module_names
        .iter()
        .map(|name| format!("config.{name}.toml"))
        .collect();
    files.push("config.toml".to_string());
    files.sort();
    for file in files {
        let Ok(text) = std::fs::read_to_string(config_dir.join(&file)) else {
            continue;
        };
        let Ok(table) = toml::from_str::<toml::Table>(&text) else {
            continue;
        };
        let Some(dotfiles) = table.get("dotfiles").and_then(|v| v.as_table()) else {
            continue;
        };
        for (target, entry) in dotfiles {
            let Some(source) = entry.get("source").and_then(|v| v.as_str()) else {
                continue;
            };
            if source.starts_with('~') || source.starts_with('/') || source.contains("{{") {
                continue;
            }
            if !config_dir.join(source).exists() {
                problems.push(format!(
                    "{file}: [dotfiles] \"{target}\" source \"{source}\" does not exist"
                ));
            }
        }
    }
}

fn check_gitignore(config_dir: &Path, problems: &mut Vec<String>) {
    let covered = std::fs::read_to_string(config_dir.join(".gitignore"))
        .map(|text| gitignore_covers(&text, "miserc.local.toml"))
        .unwrap_or(false);
    if !covered {
        problems.push(".gitignore does not cover miserc.local.toml".to_string());
    }
}

/// Whether a plain-pattern `.gitignore` ignores the top-level `file`.
/// Supports exact names, a leading `/`, and `*` wildcards; negations and
/// directory patterns are not treated as coverage.
fn gitignore_covers(text: &str, file: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') || line.ends_with('/')
        {
            return false;
        }
        glob_match(line.strip_prefix('/').unwrap_or(line), file)
    })
}

fn glob_match(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            let Some(rest) = text.strip_prefix(head) else {
                return false;
            };
            (0..=rest.len())
                .filter(|i| rest.is_char_boundary(*i))
                .any(|i| glob_match(tail, &rest[i..]))
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starter() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/config-starter")
    }

    /// Copy the starter into a fresh temp dir so a test can break one rule.
    fn scratch(tag: &str) -> PathBuf {
        let dst = std::env::temp_dir().join(format!("emc-lint-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        copy_dir(&starter(), &dst);
        dst
    }

    fn copy_dir(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).expect("mkdir");
        for entry in std::fs::read_dir(src).expect("read_dir").flatten() {
            let to = dst.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &to);
            } else {
                std::fs::copy(entry.path(), &to).expect("copy");
            }
        }
    }

    #[test]
    fn starter_template_passes() {
        assert_eq!(lint(&starter()), Vec::<String>::new());
    }

    #[test]
    fn missing_manifest_fails() {
        let dir = scratch("no-manifest");
        std::fs::remove_file(dir.join(MANIFEST_FILE)).expect("rm");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("essential-mise.toml not found"));
    }

    #[test]
    fn unparsable_manifest_fails() {
        let dir = scratch("bad-manifest");
        std::fs::write(dir.join(MANIFEST_FILE), "schema = 1\nbogus = 2\n").expect("write");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("essential-mise.toml is invalid"));
    }

    #[test]
    fn listed_module_without_file_fails() {
        let dir = scratch("no-file");
        std::fs::remove_file(dir.join("config.terminal.toml")).expect("rm");
        assert_eq!(
            lint(&dir),
            vec![
                "module \"terminal\" is listed in essential-mise.toml but config.terminal.toml does not exist"
            ]
        );
    }

    #[test]
    fn unlisted_module_file_fails() {
        let dir = scratch("unlisted");
        std::fs::write(dir.join("config.extra.toml"), "").expect("write");
        assert_eq!(
            lint(&dir),
            vec![
                "config.extra.toml is not listed in essential-mise.toml (add a [[modules]] entry or a base module)"
            ]
        );
    }

    #[test]
    fn local_and_global_files_are_not_modules() {
        let dir = scratch("local");
        std::fs::write(dir.join("config.work.local.toml"), "").expect("write");
        std::fs::write(dir.join("miserc.local.toml"), "env = []\n").expect("write");
        assert_eq!(lint(&dir), Vec::<String>::new());
    }

    #[test]
    fn dangling_dotfile_source_fails() {
        let dir = scratch("dangling");
        std::fs::remove_file(dir.join("dotfiles/terminal/starship.toml")).expect("rm");
        assert_eq!(
            lint(&dir),
            vec![
                "config.terminal.toml: [dotfiles] \"~/.config/starship.toml\" source \"dotfiles/terminal/starship.toml\" does not exist"
            ]
        );
    }

    #[test]
    fn absolute_and_home_sources_are_skipped() {
        let dir = scratch("abs-source");
        std::fs::write(
            dir.join("config.terminal.toml"),
            "[dotfiles]\n\"~/a\" = { source = \"~/x\" }\n\"~/b\" = { source = \"/etc/x\" }\n",
        )
        .expect("write");
        assert_eq!(lint(&dir), Vec::<String>::new());
    }

    #[test]
    fn invalid_toml_anywhere_fails() {
        let dir = scratch("bad-toml");
        std::fs::write(dir.join("dotfiles/terminal/broken.toml"), "a = = 1\n").expect("write");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("dotfiles/terminal/broken.toml: invalid TOML"));
    }

    #[test]
    fn gitignore_must_cover_local_selection() {
        let dir = scratch("gitignore");
        std::fs::write(dir.join(".gitignore"), ".DS_Store\n").expect("write");
        assert_eq!(
            lint(&dir),
            vec![".gitignore does not cover miserc.local.toml"]
        );
        std::fs::remove_file(dir.join(".gitignore")).expect("rm");
        assert_eq!(lint(&dir).len(), 1);
        std::fs::write(dir.join(".gitignore"), "*.local.toml\n").expect("write");
        assert_eq!(lint(&dir), Vec::<String>::new());
        std::fs::write(
            dir.join(".gitignore"),
            "# miserc.local.toml\n!miserc.local.toml\n",
        )
        .expect("write");
        assert_eq!(lint(&dir).len(), 1);
    }
}
