//! `lint`: check a config repository against the contract in
//! `docs/config-repo.md`. Pure filesystem reads; one problem per line.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::metadata::{
    ModuleMeta, Problem, REPO_FILE, RepoMeta, module_file, module_names, stray_problems,
};

/// Check `config_dir`; returns one human-readable line per problem
/// (empty when the repository is valid). Output order is deterministic.
pub fn lint(config_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();

    let modules = module_names(config_dir);
    let mut headers = vec![REPO_FILE.to_string()];
    headers.extend(modules.iter().map(|name| module_file(name)));
    for file in &headers {
        problems.extend(check_header(config_dir, file));
    }
    let mut lines: Vec<String> = problems.iter().map(Problem::to_string).collect();

    check_toml_files(config_dir, &mut lines);
    check_dotfile_sources(config_dir, &modules, &mut lines);
    check_gitignore(config_dir, &mut lines);
    lines
}

/// Header validity and stray `#:` lines of one `config*.toml` file.
fn check_header(config_dir: &Path, file: &str) -> Vec<Problem> {
    let text = match std::fs::read_to_string(config_dir.join(file)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return vec![Problem {
                path: file.to_string(),
                line: 0,
                message: "not found (it carries the repository header: schema and name)"
                    .to_string(),
            }];
        }
        Err(e) => {
            return vec![Problem {
                path: file.to_string(),
                line: 0,
                message: format!("cannot read: {e}"),
            }];
        }
    };
    let header = if file == REPO_FILE {
        RepoMeta::parse(file, &text).err()
    } else {
        ModuleMeta::parse(file, &text).err()
    };
    let mut problems: Vec<Problem> = header.into_iter().collect();
    problems.extend(stray_problems(file, &text));
    problems.sort_by_key(|p| p.line);
    problems
}

/// Every `*.toml` under the repository parses. Hidden directories,
/// `node_modules`, and `target` are not searched.
fn check_toml_files(config_dir: &Path, problems: &mut Vec<String>) {
    let mut paths = Vec::new();
    collect_toml(config_dir, &mut paths);
    paths.sort();
    for path in paths {
        let rel = relative(config_dir, &path);
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

    fn write(dir: &Path, file: &str, text: &str) {
        std::fs::write(dir.join(file), text).expect("write");
    }

    #[test]
    fn missing_config_toml_fails() {
        let dir = scratch("no-repo");
        std::fs::remove_file(dir.join("config.toml")).expect("rm");
        assert_eq!(
            lint(&dir),
            vec!["config.toml: not found (it carries the repository header: schema and name)"]
        );
    }

    #[test]
    fn config_toml_without_header_fails() {
        let dir = scratch("repo-no-header");
        write(&dir, "config.toml", "min_version = \"2026.10.23\"\n");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.toml:1: missing metadata header"));
    }

    #[test]
    fn repo_header_rules() {
        let dir = scratch("repo-rules");
        let body = "min_version = \"2026.10.23\"\n";
        let cases = [
            ("#: name = \"x\"\n", "config.toml:1: missing field `schema`"),
            ("#: schema = 1\n", "config.toml:1: missing field `name`"),
            (
                "#: schema = 2\n#: name = \"x\"\n",
                "config.toml:1: unsupported schema 2 (this console reads schema 1)",
            ),
            (
                "#: schema = 1\n#: name = \"\"\n",
                "config.toml:2: name must not be empty",
            ),
            (
                "#: schema = 1\n#: name = \"x\"\n#: min_console = \"1.2\"\n",
                "config.toml:3: min_console \"1.2\" is not a MAJOR.MINOR.PATCH version",
            ),
        ];
        for (header, expected) in cases {
            write(&dir, "config.toml", &format!("{header}{body}"));
            assert_eq!(lint(&dir), vec![expected.to_string()], "{header}");
        }
    }

    #[test]
    fn module_without_header_fails() {
        let dir = scratch("mod-no-header");
        write(&dir, "config.extra.toml", "[bootstrap.packages]\n");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.extra.toml:1: missing metadata header"));
    }

    #[test]
    fn module_header_rules() {
        let dir = scratch("mod-rules");
        let cases = [
            (
                "#: order = 1\n",
                "config.extra.toml:1: missing field `description`",
            ),
            (
                "#: description = \" \"\n",
                "config.extra.toml:1: description must not be empty",
            ),
            (
                "# note\n#: description = \"d\"\n#: icon = \"x\"\n",
                "config.extra.toml:3: unknown field `icon`",
            ),
        ];
        for (header, expected) in cases {
            write(&dir, "config.extra.toml", header);
            let problems = lint(&dir);
            assert_eq!(problems.len(), 1, "{problems:?}");
            assert!(problems[0].starts_with(expected), "{problems:?}");
        }
    }

    #[test]
    fn duplicate_header_key_fails_with_its_line() {
        let dir = scratch("mod-dup");
        write(
            &dir,
            "config.extra.toml",
            "#: description = \"d\"\n#: order = 1\n#: order = 2\n",
        );
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.extra.toml:3: duplicate key"));
    }

    #[test]
    fn base_is_valid_for_any_module() {
        let dir = scratch("mod-base");
        write(
            &dir,
            "config.extra.toml",
            "#: description = \"d\"\n#: base = true\n",
        );
        assert_eq!(lint(&dir), Vec::<String>::new());
    }

    #[test]
    fn stray_sigil_below_the_header_fails() {
        let dir = scratch("stray");
        write(
            &dir,
            "config.extra.toml",
            "#: description = \"d\"\n\n[bootstrap.packages]\n#: order = 1\n",
        );
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.extra.toml:4: `#:` metadata line outside"));
    }

    #[test]
    fn header_after_code_fails_twice() {
        let dir = scratch("after-code");
        write(
            &dir,
            "config.extra.toml",
            "[bootstrap.packages]\n#: description = \"d\"\n",
        );
        let problems = lint(&dir);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("config.extra.toml:1: missing metadata header"));
        assert!(problems[1].starts_with("config.extra.toml:2: `#:` metadata line outside"));
    }

    #[test]
    fn stray_sigil_in_config_toml_fails() {
        let dir = scratch("stray-repo");
        let text = std::fs::read_to_string(dir.join("config.toml")).expect("read");
        write(&dir, "config.toml", &format!("{text}\n#: order = 1\n"));
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.toml:"));
    }

    #[test]
    fn sigil_in_a_multiline_string_is_not_stray() {
        let dir = scratch("ml-string");
        write(
            &dir,
            "config.extra.toml",
            "#: description = \"d\"\n\n[dotfiles]\n\"~/.x\" = { block = '''\n#: not metadata\n''' }\n",
        );
        assert_eq!(lint(&dir), Vec::<String>::new());
    }

    #[test]
    fn crlf_and_bom_headers_pass() {
        let dir = scratch("crlf-bom");
        write(
            &dir,
            "config.extra.toml",
            "\u{feff}#: description = \"d\"\r\n#: order = 2\r\n\r\n[bootstrap.packages]\r\n",
        );
        assert_eq!(lint(&dir), Vec::<String>::new());
    }

    #[test]
    fn invalid_toml_body_fails_with_a_valid_header() {
        let dir = scratch("bad-body");
        write(&dir, "config.extra.toml", "#: description = \"d\"\n[x\n");
        let problems = lint(&dir);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("config.extra.toml: invalid TOML"));
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
            "#: description = \"d\"\n[dotfiles]\n\"~/a\" = { source = \"~/x\" }\n\"~/b\" = { source = \"/etc/x\" }\n",
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
