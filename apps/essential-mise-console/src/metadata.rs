//! `#:` metadata comments: what a config repository declares about itself.
//!
//! The metadata lives in the leading comment block of the config files
//! themselves (`config.toml` for the repository, `config.<name>.toml` for each
//! module). A metadata line starts at column 0 with `#:`, optionally followed
//! by one space, then TOML `key = value` text. The structs below are the single
//! source of truth for the contract; the prose lives in `docs/config-repo.md`.
//! Parsing is strict (`deny_unknown_fields`) and every error carries the
//! original `file:line`.

use std::collections::BTreeSet;
use std::path::Path;

/// Repository-level metadata file (also the repository's global config).
pub const REPO_FILE: &str = "config.toml";

/// The only metadata schema version this console understands.
pub const SCHEMA_VERSION: u32 = 1;

const SIGIL: &str = "#:";

/// One problem in a config file: `path:line: message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Path relative to the config directory.
    pub path: String,
    /// 1-based line; `0` for problems about the whole file.
    pub line: usize,
    /// What is wrong and how to fix it.
    pub message: String,
}

impl Problem {
    fn new(path: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            path: path.to_string(),
            line,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.path, self.message)
        } else {
            write!(f, "{}:{}: {}", self.path, self.line, self.message)
        }
    }
}

/// Header of `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoMeta {
    /// Metadata version; must be [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Display name for the TUI header.
    pub name: String,
    /// Oldest console version allowed to use this repository
    /// (`MAJOR.MINOR.PATCH`).
    #[serde(default)]
    pub min_console: Option<String>,
}

/// Header of `config.<name>.toml`. The module name comes from the file name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleMeta {
    /// One-line description shown beside the module.
    pub description: String,
    /// Position in the module list (ascending; modules without one come last).
    #[serde(default)]
    pub order: Option<u32>,
    /// Applied automatically at the end of `setup`; not listed in the TUI.
    #[serde(default)]
    pub base: bool,
}

/// The leading `#:` block of a file, with a map back to file lines.
struct Header {
    /// Collected metadata text: prefix stripped, lines joined with `\n`.
    text: String,
    /// 1-based file line of each line in `text`.
    lines: Vec<usize>,
}

impl Header {
    fn first_line(&self) -> usize {
        self.lines.first().copied().unwrap_or(1)
    }

    /// File line for a byte offset in `text`.
    fn line_at(&self, offset: usize) -> usize {
        let index = self.text[..offset.min(self.text.len())]
            .matches('\n')
            .count();
        self.lines
            .get(index)
            .copied()
            .unwrap_or_else(|| self.first_line())
    }

    /// File line of the line that sets `key`, else the first header line.
    fn line_of_key(&self, key: &str) -> usize {
        self.text
            .split('\n')
            .position(|line| {
                line.trim_start()
                    .strip_prefix(key)
                    .is_some_and(|rest| rest.starts_with([' ', '\t', '=']))
            })
            .and_then(|index| self.lines.get(index).copied())
            .unwrap_or_else(|| self.first_line())
    }
}

/// Result of scanning one file for `#:` lines.
struct Scan {
    header: Header,
    /// `(line, message)` for `#:` comments that are not valid metadata lines.
    stray: Vec<(usize, &'static str)>,
}

const STRAY_BELOW: &str = "`#:` metadata line outside the leading comment block; move it above \
                           the first line of TOML";
const STRAY_INDENTED: &str = "`#:` metadata line must start at column 0";

/// Lines without terminators; tolerates CRLF and a UTF-8 BOM.
fn file_lines(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

fn scan(text: &str) -> Scan {
    let lines = file_lines(text);
    let mut header = Header {
        text: String::new(),
        lines: Vec::new(),
    };
    let mut stray = Vec::new();

    let mut body_start = lines.len();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('#') {
            body_start = index;
            break;
        }
        if let Some(rest) = line.strip_prefix(SIGIL) {
            if !header.lines.is_empty() {
                header.text.push('\n');
            }
            header.text.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            header.lines.push(index + 1);
        } else if trimmed.starts_with(SIGIL) {
            stray.push((index + 1, STRAY_INDENTED));
        }
    }

    for (index, message) in sigil_comments(&lines[body_start..]) {
        stray.push((body_start + index + 1, message));
    }
    Scan { header, stray }
}

/// `#:` comments in TOML body lines, skipping string contents. Returns
/// `(index into lines, message)`.
fn sigil_comments(lines: &[&str]) -> Vec<(usize, &'static str)> {
    #[derive(Clone, Copy, PartialEq)]
    enum In {
        Code,
        MultiBasic,
        MultiLiteral,
    }
    let mut state = In::Code;
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match state {
                In::MultiBasic => {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i..].starts_with(b"\"\"\"") {
                        i += 3;
                        state = In::Code;
                    } else {
                        i += 1;
                    }
                }
                In::MultiLiteral => {
                    if bytes[i..].starts_with(b"'''") {
                        i += 3;
                        state = In::Code;
                    } else {
                        i += 1;
                    }
                }
                In::Code => match bytes[i] {
                    b'#' => {
                        if bytes[i + 1..].starts_with(b":") {
                            found.push((index, STRAY_BELOW));
                        }
                        break;
                    }
                    b'"' if bytes[i..].starts_with(b"\"\"\"") => {
                        i += 3;
                        state = In::MultiBasic;
                    }
                    b'\'' if bytes[i..].starts_with(b"'''") => {
                        i += 3;
                        state = In::MultiLiteral;
                    }
                    quote @ (b'"' | b'\'') => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != quote {
                            i += if quote == b'"' && bytes[i] == b'\\' {
                                2
                            } else {
                                1
                            };
                        }
                        i += 1;
                    }
                    _ => i += 1,
                },
            }
        }
    }
    found
}

/// Misplaced `#:` lines in a config file, as problems for `path`.
pub fn stray_problems(path: &str, text: &str) -> Vec<Problem> {
    scan(text)
        .stray
        .into_iter()
        .map(|(line, message)| Problem::new(path, line, message))
        .collect()
}

fn parse_header<T: serde::de::DeserializeOwned>(
    path: &str,
    text: &str,
) -> Result<(T, Header), Problem> {
    let header = scan(text).header;
    if header.lines.is_empty() {
        return Err(Problem::new(
            path,
            1,
            "missing metadata header: start the file with `#: key = \"value\"` lines \
             (see docs/config-repo.md)",
        ));
    }
    match toml::from_str::<T>(&header.text) {
        Ok(meta) => Ok((meta, header)),
        Err(e) => {
            let line = e
                .span()
                .map_or_else(|| header.first_line(), |span| header.line_at(span.start));
            Err(Problem::new(path, line, e.message().to_string()))
        }
    }
}

impl RepoMeta {
    /// Read and validate the `#:` header of `config.toml` text.
    pub fn parse(path: &str, text: &str) -> Result<Self, Problem> {
        let (meta, header) = parse_header::<Self>(path, text)?;
        if meta.schema != SCHEMA_VERSION {
            return Err(Problem::new(
                path,
                header.line_of_key("schema"),
                format!(
                    "unsupported schema {} (this console reads schema {SCHEMA_VERSION})",
                    meta.schema
                ),
            ));
        }
        if meta.name.trim().is_empty() {
            return Err(Problem::new(
                path,
                header.line_of_key("name"),
                "name must not be empty",
            ));
        }
        if let Some(min) = &meta.min_console
            && Version::parse(min).is_none()
        {
            return Err(Problem::new(
                path,
                header.line_of_key("min_console"),
                format!("min_console \"{min}\" is not a MAJOR.MINOR.PATCH version"),
            ));
        }
        Ok(meta)
    }

    /// Parsed `min_console`, when set (validated by [`RepoMeta::parse`]).
    pub fn min_console_version(&self) -> Option<Version> {
        self.min_console.as_deref().and_then(Version::parse)
    }
}

impl ModuleMeta {
    /// Read and validate the `#:` header of `config.<name>.toml` text.
    pub fn parse(path: &str, text: &str) -> Result<Self, Problem> {
        let (mut meta, header) = parse_header::<Self>(path, text)?;
        meta.description = meta.description.trim().to_string();
        if meta.description.is_empty() {
            return Err(Problem::new(
                path,
                header.line_of_key("description"),
                "description must not be empty",
            ));
        }
        if meta.description.contains(['\n', '\r']) {
            return Err(Problem::new(
                path,
                header.line_of_key("description"),
                "description must be a single line",
            ));
        }
        Ok(meta)
    }
}

/// Names of top-level `config.<name>.toml` files that count as modules:
/// `config.toml` and `*.local.toml` are excluded. Sorted.
pub fn module_names(config_dir: &Path) -> BTreeSet<String> {
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

/// File name of module `name`.
pub fn module_file(name: &str) -> String {
    format!("config.{name}.toml")
}

/// A module offered in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Module name, derived from `config.<name>.toml`.
    pub name: String,
    /// One-line description shown beside the module.
    pub description: String,
}

/// Everything the console reads from the `#:` headers of a config repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Display name for the TUI header.
    pub name: String,
    /// Oldest console version allowed to use this repository.
    pub min_console: Option<String>,
    /// Modules applied at the end of `setup`, sorted by name.
    pub base_modules: Vec<String>,
    /// Selectable modules in TUI order: `order` ascending, absent order last,
    /// then name.
    pub modules: Vec<Module>,
}

/// Why a config repository's metadata could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    /// One entry per bad file, in file order (`config.toml` first).
    pub problems: Vec<Problem>,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lines: Vec<String> = self.problems.iter().map(Problem::to_string).collect();
        write!(f, "{}", lines.join("\n"))
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// Read `config.toml` and every `config.<name>.toml` header in `config_dir`.
    pub fn load(config_dir: &Path) -> Result<Self, ManifestError> {
        let mut problems = Vec::new();

        let repo = read(config_dir, REPO_FILE).and_then(|text| RepoMeta::parse(REPO_FILE, &text));
        let repo = repo.map_err(|p| problems.push(p)).ok();

        let mut base_modules = Vec::new();
        let mut modules: Vec<(Option<u32>, Module)> = Vec::new();
        for name in module_names(config_dir) {
            let file = module_file(&name);
            match read(config_dir, &file).and_then(|text| ModuleMeta::parse(&file, &text)) {
                Ok(meta) if meta.base => base_modules.push(name),
                Ok(meta) => modules.push((
                    meta.order,
                    Module {
                        name,
                        description: meta.description,
                    },
                )),
                Err(p) => problems.push(p),
            }
        }
        modules.sort_by(|(a_order, a), (b_order, b)| {
            (a_order.is_none(), a_order, &a.name).cmp(&(b_order.is_none(), b_order, &b.name))
        });

        match repo {
            Some(repo) if problems.is_empty() => Ok(Self {
                name: repo.name,
                min_console: repo.min_console,
                base_modules,
                modules: modules.into_iter().map(|(_, m)| m).collect(),
            }),
            _ => Err(ManifestError { problems }),
        }
    }

    /// Parsed `min_console`, when set (validated by [`RepoMeta::parse`]).
    pub fn min_console_version(&self) -> Option<Version> {
        self.min_console.as_deref().and_then(Version::parse)
    }
}

fn read(config_dir: &Path, file: &str) -> Result<String, Problem> {
    std::fs::read_to_string(config_dir.join(file)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Problem::new(file, 0, format!("not found in {}", config_dir.display()))
        } else {
            Problem::new(file, 0, format!("cannot read: {e}"))
        }
    })
}

/// `MAJOR.MINOR.PATCH` version, ordered numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    /// Parse exactly three dot-separated unsigned integers.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let mut next = || {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok()
        };
        let version = Self(next()?, next()?, next()?);
        parts.next().is_none().then_some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Enforce `min_console` against the running console version.
///
/// `Err` carries the user-facing message, including the upgrade command.
pub fn check_min_console(manifest: &Manifest, running: &str) -> Result<(), String> {
    let Some(min) = manifest.min_console_version() else {
        return Ok(());
    };
    let current = Version::parse(running)
        .ok_or_else(|| format!("cannot parse the console version \"{running}\""))?;
    if current >= min {
        return Ok(());
    }
    Err(format!(
        "This configuration requires essential-mise-console {min} or newer (running {current}). \
         Upgrade by running the latest bootstrap.sh from \
         https://github.com/donbeave/essential-mise, then rerun this command."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const REPO: &str = "#: schema = 1\n#: name = \"demo\"\n#: min_console = \"0.2.0\"\n";

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emc-meta-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn repo_with(tag: &str, modules: &[(&str, &str)]) -> PathBuf {
        let dir = temp_dir(tag);
        std::fs::write(dir.join(REPO_FILE), REPO).expect("write config.toml");
        for (name, text) in modules {
            std::fs::write(dir.join(module_file(name)), text).expect("write module");
        }
        dir
    }

    fn module(text: &str) -> Result<ModuleMeta, Problem> {
        ModuleMeta::parse("config.demo.toml", text)
    }

    fn module_problem(text: &str) -> String {
        module(text).expect_err("invalid header").to_string()
    }

    #[test]
    fn valid_module_header() {
        let meta =
            module("#: description = \"Slack, Zoom\"\n#: order = 40\n#: base = true\n\n[x]\n")
                .expect("valid");
        assert_eq!(
            meta,
            ModuleMeta {
                description: "Slack, Zoom".into(),
                order: Some(40),
                base: true
            }
        );
    }

    #[test]
    fn optional_keys_default() {
        let meta = module("#: description = \"d\"\n").expect("valid");
        assert_eq!((meta.order, meta.base), (None, false));
    }

    #[test]
    fn prefix_space_is_optional_and_only_one_is_stripped() {
        let meta = module("#:description = \"a\"\n#:  order = 3\n").expect("valid");
        assert_eq!(meta.order, Some(3));
    }

    #[test]
    fn ordinary_comments_and_blank_lines_may_surround_the_header() {
        let text =
            "# Terminal tools\n\n#: description = \"d\"\n# more prose\n#: order = 5\n\n[x]\n";
        assert_eq!(module(text).expect("valid").order, Some(5));
    }

    #[test]
    fn missing_header_is_reported_at_line_one() {
        let message = module_problem("[x]\na = 1\n");
        assert!(
            message.starts_with("config.demo.toml:1: missing metadata header"),
            "{message}"
        );
    }

    #[test]
    fn missing_description_points_at_the_header() {
        let message = module_problem("# top\n#: order = 1\n");
        assert_eq!(message, "config.demo.toml:2: missing field `description`");
    }

    #[test]
    fn empty_and_multiline_descriptions_are_rejected() {
        let message = module_problem("#: description = \"   \"\n");
        assert_eq!(message, "config.demo.toml:1: description must not be empty");
        let message = module_problem("#: order = 1\n#: description = \"a\\nb\"\n");
        assert_eq!(
            message,
            "config.demo.toml:2: description must be a single line"
        );
    }

    #[test]
    fn description_is_trimmed() {
        assert_eq!(
            module("#: description = \"  x \"\n")
                .expect("ok")
                .description,
            "x"
        );
    }

    #[test]
    fn unknown_key_is_rejected_at_its_line() {
        let message = module_problem("#: description = \"d\"\n#: icon = \"x\"\n");
        assert!(
            message.starts_with("config.demo.toml:2: unknown field `icon`"),
            "{message}"
        );
    }

    #[test]
    fn duplicate_key_is_rejected_at_its_line() {
        let message = module_problem("#: description = \"d\"\n#: order = 1\n#: order = 2\n");
        assert!(
            message.starts_with("config.demo.toml:3: duplicate key"),
            "{message}"
        );
    }

    #[test]
    fn wrong_value_type_is_rejected_at_its_line() {
        let message = module_problem("#: description = \"d\"\n#: order = \"first\"\n");
        assert!(
            message.starts_with("config.demo.toml:2: invalid type"),
            "{message}"
        );
        assert!(module("#: description = \"d\"\n#: order = -1\n").is_err());
    }

    #[test]
    fn toml_syntax_error_maps_to_the_file_line() {
        let message = module_problem("# x\n#: description = \"d\"\n#: order = = 1\n");
        assert!(message.starts_with("config.demo.toml:3:"), "{message}");
    }

    #[test]
    fn header_after_code_is_not_a_header() {
        let text = "[x]\n#: description = \"d\"\n";
        assert!(module(text).is_err());
        assert_eq!(
            stray_problems("config.demo.toml", text),
            vec![Problem::new("config.demo.toml", 2, STRAY_BELOW)]
        );
    }

    #[test]
    fn sigil_below_the_header_is_stray() {
        let text = "#: description = \"d\"\n\n[x]\na = 1\n#: order = 1\nb = 2 #: trailing\n";
        let lines: Vec<usize> = stray_problems("f", text).iter().map(|p| p.line).collect();
        assert_eq!(lines, [5, 6]);
    }

    #[test]
    fn indented_sigil_in_the_leading_block_is_stray() {
        let text = "#: description = \"d\"\n  #: order = 1\n";
        assert_eq!(
            stray_problems("f", text),
            vec![Problem::new("f", 2, STRAY_INDENTED)]
        );
        assert_eq!(module(text).expect("header still valid").order, None);
    }

    #[test]
    fn sigil_inside_multiline_strings_is_ignored() {
        let text = "#: description = \"d\"\n\n[x]\nbasic = \"\"\"\n#: order = 1\n\\\"\"\"\n#: still inside\n\"\"\"\nliteral = '''\n#: nope\n'''\nq = \"#: in a string\"\nr = '#: too'\n";
        assert_eq!(stray_problems("f", text), Vec::new());
        assert_eq!(module(text).expect("valid").order, None);
        let after = format!("{text}#: real stray\n");
        assert_eq!(stray_problems("f", &after).len(), 1);
    }

    #[test]
    fn crlf_line_endings_are_handled() {
        let text = "#: description = \"d\"\r\n#: order = 7\r\n\r\n[x]\r\n#: stray\r\n";
        assert_eq!(module(text).expect("valid").order, Some(7));
        assert_eq!(stray_problems("f", text)[0].line, 5);
        let message = module_problem("#: description = \"d\"\r\n#: bogus = 1\r\n");
        assert!(message.starts_with("config.demo.toml:2:"), "{message}");
    }

    #[test]
    fn utf8_bom_is_handled() {
        let text = "\u{feff}#: description = \"d\"\n[x]\n";
        assert_eq!(module(text).expect("valid").description, "d");
    }

    #[test]
    fn repo_header_requires_schema_one_and_a_name() {
        let ok = RepoMeta::parse("config.toml", REPO).expect("valid");
        assert_eq!(ok.name, "demo");
        assert_eq!(ok.min_console_version(), Some(Version(0, 2, 0)));

        let schema2 = REPO.replace("schema = 1", "schema = 2");
        let e = RepoMeta::parse("config.toml", &schema2).expect_err("schema");
        assert!(
            e.to_string()
                .starts_with("config.toml:1: unsupported schema 2"),
            "{e}"
        );

        let e = RepoMeta::parse("config.toml", "#: schema = 1\n").expect_err("name");
        assert_eq!(e.to_string(), "config.toml:1: missing field `name`");

        let e = RepoMeta::parse("config.toml", "#: schema = 1\n#: name = \" \"\n")
            .expect_err("empty name");
        assert_eq!(e.to_string(), "config.toml:2: name must not be empty");

        let e = RepoMeta::parse("config.toml", "#: name = \"x\"\n").expect_err("schema");
        assert_eq!(e.to_string(), "config.toml:1: missing field `schema`");
    }

    #[test]
    fn repo_header_rejects_unknown_keys() {
        let text = format!("{REPO}#: base = true\n");
        let e = RepoMeta::parse("config.toml", &text).expect_err("unknown");
        assert!(
            e.to_string()
                .starts_with("config.toml:4: unknown field `base`"),
            "{e}"
        );
    }

    #[test]
    fn bad_min_console_is_rejected_at_its_line() {
        for bad in ["latest", "1.2", "1.2.3.4", "1.2.x", "v1.2.3", ""] {
            let text = REPO.replace("0.2.0", bad);
            let e = RepoMeta::parse("config.toml", &text).expect_err(bad);
            assert_eq!(
                e.to_string(),
                format!("config.toml:3: min_console \"{bad}\" is not a MAJOR.MINOR.PATCH version")
            );
        }
    }

    #[test]
    fn min_console_is_optional() {
        let text = "#: schema = 1\n#: name = \"d\"\n";
        assert_eq!(
            RepoMeta::parse("config.toml", text)
                .expect("ok")
                .min_console,
            None
        );
    }

    #[test]
    fn min_console_gate_compares_numerically() {
        let dir = repo_with("gate", &[]);
        let manifest = Manifest::load(&dir).expect("valid");
        assert!(check_min_console(&manifest, "0.2.0").is_ok());
        assert!(check_min_console(&manifest, "0.10.0").is_ok());
        let err = check_min_console(&manifest, "0.1.5").expect_err("too old");
        assert!(err.contains("0.2.0") && err.contains("0.1.5") && err.contains("bootstrap.sh"));
    }

    #[test]
    fn module_name_comes_from_the_file_name() {
        let dir = repo_with("names", &[("terminal", "#: description = \"Shell\"\n")]);
        let manifest = Manifest::load(&dir).expect("valid");
        assert_eq!(manifest.name, "demo");
        assert_eq!(
            manifest.modules,
            vec![Module {
                name: "terminal".into(),
                description: "Shell".into()
            }]
        );
    }

    #[test]
    fn config_toml_and_local_files_are_not_modules() {
        let dir = repo_with("local", &[]);
        std::fs::write(dir.join("config.work.local.toml"), "").expect("write");
        std::fs::write(dir.join("miserc.local.toml"), "").expect("write");
        std::fs::write(dir.join("notes.toml"), "").expect("write");
        let manifest = Manifest::load(&dir).expect("valid");
        assert!(manifest.modules.is_empty() && manifest.base_modules.is_empty());
        assert_eq!(module_names(&dir), BTreeSet::new());
    }

    #[test]
    fn modules_sort_by_order_then_absent_order_then_name() {
        let dir = repo_with(
            "sort",
            &[
                ("zeta", "#: description = \"z\"\n#: order = 10\n"),
                ("alpha", "#: description = \"a\"\n"),
                ("beta", "#: description = \"b\"\n"),
                ("gamma", "#: description = \"g\"\n#: order = 2\n"),
                ("delta", "#: description = \"d\"\n#: order = 10\n"),
            ],
        );
        let names: Vec<String> = Manifest::load(&dir)
            .expect("valid")
            .modules
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["gamma", "delta", "zeta", "alpha", "beta"]);
    }

    #[test]
    fn base_modules_are_excluded_from_the_catalogue() {
        let dir = repo_with(
            "base",
            &[
                ("foundation", "#: description = \"Base\"\n#: base = true\n"),
                ("terminal", "#: description = \"Shell\"\n#: order = 1\n"),
                (
                    "core",
                    "#: description = \"Core\"\n#: base = true\n#: order = 5\n",
                ),
            ],
        );
        let manifest = Manifest::load(&dir).expect("valid");
        assert_eq!(manifest.base_modules, ["core", "foundation"]);
        assert_eq!(manifest.modules.len(), 1);
        assert_eq!(manifest.modules[0].name, "terminal");
    }

    #[test]
    fn missing_config_toml_is_reported_with_its_name() {
        let dir = temp_dir("no-repo");
        let err = Manifest::load(&dir).expect_err("no config.toml");
        assert_eq!(err.problems.len(), 1);
        assert!(
            err.to_string().starts_with("config.toml: not found"),
            "{err}"
        );
    }

    #[test]
    fn load_reports_every_bad_file() {
        let dir = repo_with(
            "many",
            &[
                ("a", "[x]\n"),
                ("b", "#: description = \"ok\"\n"),
                ("c", "#: nope = 1\n"),
            ],
        );
        let err = Manifest::load(&dir).expect_err("bad modules");
        let files: Vec<&str> = err.problems.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(files, ["config.a.toml", "config.c.toml"]);
    }

    #[test]
    fn starter_template_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/config-starter");
        let manifest = Manifest::load(&dir).expect("starter");
        assert_eq!(manifest.base_modules, ["foundation"]);
        assert!(!manifest.modules.is_empty());
        assert!(manifest.modules.iter().all(|m| !m.description.is_empty()));
    }
}
