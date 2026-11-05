//! Backend unit tests (T2.1): origin parsing over synthetic TOML.

use super::*;
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// Fresh temp dir for one test; removed afterwards.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "emc-test-{name}-{}-{}",
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn write_env(dir: &Path, env: &str, body: &str) {
    let mut file = std::fs::File::create(dir.join(format!("config.{env}.toml")))
        .expect("create synthetic env toml");
    file.write_all(body.as_bytes())
        .expect("write synthetic env toml");
}

#[test]
fn origin_map_reads_declaring_module() {
    let dir = test_dir("origin");
    write_env(
        &dir,
        "alpha",
        "[bootstrap.packages]\n\"brew:shared\" = \"latest\"\n\"brew:only-alpha\" = \"latest\"\n",
    );
    write_env(
        &dir,
        "beta",
        "[bootstrap.packages]\n\"brew:shared\" = \"latest\"\n\"npm:only-beta\" = \"1.2.3\"\n",
    );

    let backend = MiseBackend::new(dir.clone(), vec!["alpha".to_string(), "beta".to_string()]);
    let map = backend.origin_map(&dir).expect("origin_map parses");

    let shared = ResId {
        kind: "brew".to_string(),
        name: "shared".to_string(),
    };
    let envs = map.get(&shared).expect("shared package present");
    assert_eq!(envs.len(), 2, "multi-declare origin lists both envs");
    assert!(envs.contains(&"alpha".to_string()));
    assert!(envs.contains(&"beta".to_string()));

    let alpha_only = ResId {
        kind: "brew".to_string(),
        name: "only-alpha".to_string(),
    };
    assert_eq!(
        map.get(&alpha_only),
        Some(&vec!["alpha".to_string()]),
        "single-declare origin lists one env"
    );

    std::fs::remove_dir_all(&dir).ok();
}
