//! Config repository source given to `setup --config` (contract items 6-7):
//! `OWNER/REPO`, an HTTPS URL, an SSH URL, or a `file://` URL.

/// How the source is reached; drives the repository-access stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// `https://github.com/...` (or `OWNER/REPO` shorthand).
    GithubHttps,
    /// `git@github.com:...` or `ssh://git@github.com/...`.
    GithubSsh,
    /// HTTPS on another host.
    OtherHttps,
    /// SSH on another host.
    OtherSsh,
    /// `file://` URL; never probed.
    File,
}

/// A normalized config repository source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSource {
    /// Clone URL (`OWNER/REPO` expanded to a GitHub HTTPS URL).
    pub url: String,
    /// Transport class of [`ConfigSource::url`].
    pub kind: SourceKind,
}

impl ConfigSource {
    /// Parse and normalize user input. `Err` is a user-facing message.
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        let bad = || {
            format!(
                "\"{input}\" is not a config source; use OWNER/REPO, an https:// URL, \
                 an SSH URL (git@host:OWNER/REPO.git), or a file:// URL"
            )
        };
        if input.is_empty() {
            return Err(bad());
        }
        if let Some(rest) = input.strip_prefix("file://") {
            if rest.is_empty() {
                return Err(bad());
            }
            return Ok(Self {
                url: input.to_string(),
                kind: SourceKind::File,
            });
        }
        if input.starts_with("https://") {
            let kind = if github_slug(input).is_some() {
                SourceKind::GithubHttps
            } else {
                SourceKind::OtherHttps
            };
            return Ok(Self {
                url: input.to_string(),
                kind,
            });
        }
        if input.starts_with("git@") || input.starts_with("ssh://") {
            let kind = if github_slug(input).is_some() {
                SourceKind::GithubSsh
            } else {
                SourceKind::OtherSsh
            };
            return Ok(Self {
                url: input.to_string(),
                kind,
            });
        }
        if is_owner_repo(input) {
            return Ok(Self {
                url: format!("https://github.com/{input}.git"),
                kind: SourceKind::GithubHttps,
            });
        }
        Err(bad())
    }

    /// Whether `origin` (a checkout's `remote get-url origin`) names the same
    /// repository. GitHub HTTPS and SSH forms compare by `owner/repo`,
    /// case-insensitive, ignoring a trailing `.git`.
    pub fn matches_origin(&self, origin: &str) -> bool {
        identity(&self.url) == identity(origin.trim())
    }
}

fn is_owner_repo(input: &str) -> bool {
    let Some((owner, repo)) = input.split_once('/') else {
        return false;
    };
    let ok = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    ok(owner) && ok(repo)
}

/// Lowercased `owner/repo` for GitHub HTTPS and SSH URLs.
fn github_slug(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    let path = [
        "https://github.com/",
        "ssh://git@github.com/",
        "git@github.com:",
    ]
    .iter()
    .find_map(|prefix| lower.strip_prefix(prefix))?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    is_owner_repo(path).then(|| path.to_string())
}

/// Comparison key: GitHub slug when available, else the URL without a
/// trailing slash, `.git`, or `file://` prefix.
fn identity(url: &str) -> String {
    if let Some(slug) = github_slug(url) {
        return format!("github.com/{slug}");
    }
    let url = url.strip_prefix("file://").unwrap_or(url);
    let url = url.trim_end_matches('/');
    url.strip_suffix(".git").unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> ConfigSource {
        ConfigSource::parse(input).expect(input)
    }

    #[test]
    fn owner_repo_expands_to_github_https() {
        let s = parse("OWNER/REPO");
        assert_eq!(s.url, "https://github.com/OWNER/REPO.git");
        assert_eq!(s.kind, SourceKind::GithubHttps);
        assert_eq!(parse("my-org/my.config_repo").kind, SourceKind::GithubHttps);
    }

    #[test]
    fn https_urls_keep_their_form() {
        for url in [
            "https://github.com/OWNER/REPO.git",
            "https://github.com/OWNER/REPO",
        ] {
            let s = parse(url);
            assert_eq!(s.url, url);
            assert_eq!(s.kind, SourceKind::GithubHttps);
        }
        assert_eq!(
            parse("https://git.example.com/o/r.git").kind,
            SourceKind::OtherHttps
        );
    }

    #[test]
    fn ssh_urls_are_classified() {
        assert_eq!(
            parse("git@github.com:OWNER/REPO.git").kind,
            SourceKind::GithubSsh
        );
        assert_eq!(
            parse("ssh://git@github.com/OWNER/REPO").kind,
            SourceKind::GithubSsh
        );
        assert_eq!(
            parse("git@git.example.com:o/r.git").kind,
            SourceKind::OtherSsh
        );
    }

    #[test]
    fn file_urls_are_never_probed() {
        let s = parse("file:///tmp/config-repo");
        assert_eq!(s.url, "file:///tmp/config-repo");
        assert_eq!(s.kind, SourceKind::File);
    }

    #[test]
    fn invalid_sources_are_rejected() {
        for bad in [
            "",
            "   ",
            "OWNER",
            "a/b/c",
            "/abs/path",
            "http://github.com/o/r",
            "file://",
            "o/r r",
            "o:r",
        ] {
            assert!(
                ConfigSource::parse(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn github_forms_match_ignoring_case_and_dot_git() {
        let s = parse("Owner/Repo");
        for origin in [
            "https://github.com/owner/repo",
            "https://github.com/OWNER/REPO.git",
            "git@github.com:owner/repo.git",
            "git@github.com:Owner/Repo",
            "ssh://git@github.com/owner/repo.git",
            "https://github.com/owner/repo/\n",
        ] {
            assert!(s.matches_origin(origin), "{origin:?} must match");
        }
        for origin in [
            "https://github.com/owner/other",
            "https://github.com/other/repo",
            "https://gitlab.com/owner/repo",
            "",
        ] {
            assert!(!s.matches_origin(origin), "{origin:?} must not match");
        }
    }

    #[test]
    fn ssh_source_matches_https_origin() {
        assert!(parse("git@github.com:o/r.git").matches_origin("https://github.com/O/R"));
    }

    #[test]
    fn non_github_origins_compare_by_url() {
        let s = parse("https://git.example.com/o/r.git");
        assert!(s.matches_origin("https://git.example.com/o/r"));
        assert!(!s.matches_origin("https://git.example.com/o/x"));
        let f = parse("file:///tmp/repo");
        assert!(f.matches_origin("/tmp/repo"));
        assert!(f.matches_origin("file:///tmp/repo/"));
    }
}
