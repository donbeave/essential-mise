//! `essential-mise.toml`: the manifest a config repository ships.
//!
//! These structs are the single source of truth for the contract; the prose
//! lives in `docs/config-repo.md`. Parsing is strict (`deny_unknown_fields`).

use std::path::{Path, PathBuf};

/// Manifest file name at the config repository root.
pub const MANIFEST_FILE: &str = "essential-mise.toml";

/// The only manifest schema version this console understands.
pub const SCHEMA_VERSION: u32 = 1;

/// Parsed `essential-mise.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Manifest version; must be [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Display name for the TUI header.
    pub name: String,
    /// Oldest console version allowed to use this repository
    /// (`MAJOR.MINOR.PATCH`).
    #[serde(default)]
    pub min_console: Option<String>,
    /// Setup behavior.
    pub setup: Setup,
    /// Selectable modules, in TUI order.
    #[serde(default)]
    pub modules: Vec<Module>,
}

/// `[setup]` table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Modules applied at the end of `setup`, without being offered in the TUI.
    pub base_modules: Vec<String>,
}

/// One `[[modules]]` entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    /// Module name; matches `config.<name>.toml`.
    pub name: String,
    /// One-line description shown beside the module.
    pub description: String,
}

/// Why a manifest could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// No manifest at the expected path.
    Missing(PathBuf),
    /// The file exists but could not be read.
    Unreadable(PathBuf, String),
    /// TOML syntax or schema violation (including unknown fields).
    Invalid(String),
    /// `schema` is not [`SCHEMA_VERSION`].
    UnsupportedSchema(u32),
    /// Two `[[modules]]` entries share a name.
    DuplicateModule(String),
    /// `min_console` is not `MAJOR.MINOR.PATCH`.
    BadMinConsole(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "{} not found", path.display()),
            Self::Unreadable(path, e) => write!(f, "cannot read {}: {e}", path.display()),
            Self::Invalid(e) => write!(f, "{MANIFEST_FILE} is invalid: {e}"),
            Self::UnsupportedSchema(n) => write!(
                f,
                "{MANIFEST_FILE}: unsupported schema {n} (this console reads schema {SCHEMA_VERSION})"
            ),
            Self::DuplicateModule(name) => {
                write!(f, "{MANIFEST_FILE}: duplicate module \"{name}\"")
            }
            Self::BadMinConsole(v) => write!(
                f,
                "{MANIFEST_FILE}: min_console \"{v}\" is not a MAJOR.MINOR.PATCH version"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// Read and validate `<config_dir>/essential-mise.toml`.
    pub fn load(config_dir: &Path) -> Result<Self, ManifestError> {
        let path = config_dir.join(MANIFEST_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ManifestError::Missing(path));
            }
            Err(e) => return Err(ManifestError::Unreadable(path, e.to_string())),
        };
        Self::parse(&text)
    }

    /// Parse and validate manifest text.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let manifest: Self =
            toml::from_str(text).map_err(|e| ManifestError::Invalid(e.to_string()))?;
        if manifest.schema != SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema(manifest.schema));
        }
        let mut seen = std::collections::HashSet::new();
        for module in &manifest.modules {
            if !seen.insert(module.name.as_str()) {
                return Err(ManifestError::DuplicateModule(module.name.clone()));
            }
        }
        if let Some(min) = &manifest.min_console
            && Version::parse(min).is_none()
        {
            return Err(ManifestError::BadMinConsole(min.clone()));
        }
        Ok(manifest)
    }

    /// Parsed `min_console`, when set (validated by [`Manifest::parse`]).
    pub fn min_console_version(&self) -> Option<Version> {
        self.min_console.as_deref().and_then(Version::parse)
    }
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

    const VALID: &str = r#"
schema = 1
name = "demo"
min_console = "0.2.0"

[setup]
base_modules = ["foundation"]

[[modules]]
name = "terminal"
description = "Shell and prompt"

[[modules]]
name = "editors"
description = "Editors"
"#;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emc-manifest-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn valid_manifest_loads_from_disk() {
        let dir = temp_dir("valid");
        std::fs::write(dir.join(MANIFEST_FILE), VALID).expect("write");
        let manifest = Manifest::load(&dir).expect("valid manifest");
        assert_eq!(manifest.name, "demo");
        assert_eq!(manifest.setup.base_modules, ["foundation"]);
        assert_eq!(manifest.modules.len(), 2);
        assert_eq!(manifest.modules[0].name, "terminal");
        assert_eq!(manifest.min_console_version(), Some(Version(0, 2, 0)));
    }

    #[test]
    fn missing_file_is_reported_with_its_path() {
        let dir = temp_dir("missing");
        let err = Manifest::load(&dir.join("nope")).expect_err("no manifest");
        assert!(matches!(err, ManifestError::Missing(_)), "{err}");
        assert!(err.to_string().contains(MANIFEST_FILE));
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        for text in [
            VALID.replace("schema = 1", "schema = 1\nextra = true"),
            VALID.replace("base_modules", "stray = 1\nbase_modules"),
            VALID.replace(
                "description = \"Editors\"",
                "description = \"E\"\nicon = \"x\"",
            ),
        ] {
            let err = Manifest::parse(&text).expect_err("unknown field");
            assert!(matches!(err, ManifestError::Invalid(_)), "{err}");
        }
    }

    #[test]
    fn schema_other_than_one_is_rejected() {
        let err = Manifest::parse(&VALID.replace("schema = 1", "schema = 2")).expect_err("schema");
        assert_eq!(err, ManifestError::UnsupportedSchema(2));
    }

    #[test]
    fn duplicate_module_names_are_rejected() {
        let text = VALID.replace("name = \"editors\"", "name = \"terminal\"");
        let err = Manifest::parse(&text).expect_err("duplicate");
        assert_eq!(err, ManifestError::DuplicateModule("terminal".to_string()));
    }

    #[test]
    fn bad_min_console_is_rejected() {
        for bad in ["latest", "1.2", "1.2.3.4", "1.2.x", "v1.2.3", ""] {
            let text = VALID.replace("0.2.0", bad);
            let err = Manifest::parse(&text).expect_err(bad);
            assert_eq!(err, ManifestError::BadMinConsole(bad.to_string()));
        }
    }

    #[test]
    fn min_console_is_optional() {
        let text = VALID.replace("min_console = \"0.2.0\"\n", "");
        assert_eq!(Manifest::parse(&text).expect("ok").min_console, None);
    }

    #[test]
    fn min_console_gate_compares_numerically() {
        let manifest = Manifest::parse(VALID).expect("valid");
        assert!(check_min_console(&manifest, "0.2.0").is_ok());
        assert!(check_min_console(&manifest, "0.10.0").is_ok());
        let err = check_min_console(&manifest, "0.1.5").expect_err("too old");
        assert!(err.contains("0.2.0") && err.contains("0.1.5") && err.contains("bootstrap.sh"));
    }

    #[test]
    fn starter_template_manifest_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/config-starter");
        let manifest = Manifest::load(&dir).expect("starter manifest");
        assert!(!manifest.modules.is_empty());
        assert!(manifest.modules.iter().all(|m| !m.description.is_empty()));
    }
}
