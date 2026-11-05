//! Exact rerun-command formatting shared by S3 (failure modal) and S4
//! (drift notes).
//!
//! The command is the full `mise` argv the backend runs (L6 shape), so the
//! modal text and the log header always agree with what actually ran. Argv
//! comes from [`MiseBackend`](super::super::backend::MiseBackend) builders —
//! the single source of truth — this module only formats.
//!
//! Screens must not reimplement this; they call [`rerun_command`].

use std::path::Path;

/// Format the full `mise` invocation for one phase:
/// `mise -C <dir> -E <envs…> bootstrap …`.
pub fn rerun_command(
    mise_bin: &Path,
    config_dir: &Path,
    envs: &[String],
    phase_argv: &[String],
) -> String {
    let mut parts = vec![
        mise_bin.display().to_string(),
        "-C".to_string(),
        config_dir.display().to_string(),
    ];
    for env in envs {
        parts.push("-E".to_string());
        parts.push(env.clone());
    }
    parts.extend(phase_argv.iter().cloned());
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (std::path::PathBuf, std::path::PathBuf) {
        (
            std::path::PathBuf::from("mise"),
            std::path::PathBuf::from("/Users/test/.config/mise"),
        )
    }

    #[test]
    fn phase_two_command_is_exact() {
        let (mise, dir) = paths();
        let envs = ["neovim".to_string(), "shell".to_string()];
        let phase = [
            "bootstrap".to_string(),
            "packages".to_string(),
            "apply".to_string(),
            "brew:ripgrep".to_string(),
        ];
        assert_eq!(
            rerun_command(&mise, &dir, &envs, &phase),
            "mise -C /Users/test/.config/mise -E neovim -E shell \
             bootstrap packages apply brew:ripgrep"
        );
    }

    #[test]
    fn phase_one_command_is_exact() {
        let (mise, dir) = paths();
        let envs = ["terminal".to_string()];
        let phase = [
            "bootstrap".to_string(),
            "--skip".to_string(),
            "packages".to_string(),
            "--upgrade".to_string(),
            "--update".to_string(),
        ];
        assert_eq!(
            rerun_command(&mise, &dir, &envs, &phase),
            "mise -C /Users/test/.config/mise -E terminal \
             bootstrap --skip packages --upgrade --update"
        );
    }

    #[test]
    fn manager_flag_survives_formatting() {
        let (mise, dir) = paths();
        let envs: [String; 0] = [];
        let phase = [
            "bootstrap".to_string(),
            "packages".to_string(),
            "apply".to_string(),
            "--manager".to_string(),
            "brew".to_string(),
        ];
        assert_eq!(
            rerun_command(&mise, &dir, &envs, &phase),
            "mise -C /Users/test/.config/mise \
             bootstrap packages apply --manager brew"
        );
    }
}
