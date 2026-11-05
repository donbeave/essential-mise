//! Trimmed PTY harness mirroring `/tmp/tcb` (`support.rs`):
//! `tui_test::{Session, …}` spawn, text waits, stable snapshots, and frozen
//! Tuiscotti baselines. No animations, no mouse: keyboard-only journeys.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_test::{
    KeyAction, LaunchEnvMode, LaunchOptions, OpenOptions, Operation, OperationResult, RunOptions,
    Session, SnapshotCell, SnapshotScope, TerminalSnapshot,
};

const TIMEOUT: Duration = Duration::from_secs(10);

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target directory holds the tmp directory")
        .join("functional")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

pub struct App {
    session: Session,
    directory: PathBuf,
    actions: RefCell<Vec<String>>,
    checkpoint: Cell<usize>,
}

impl App {
    pub fn new(
        name: &str,
        bin: &str,
        args: &[&str],
        extra_env: &[(&str, &str)],
        cols: u16,
        rows: u16,
    ) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = evidence_root().join(format!("{name}-{}-{stamp}", std::process::id()));
        let home = directory.join("home");
        std::fs::create_dir_all(&home).expect("create isolated application home");
        let mut launch = LaunchOptions::new(bin)
            .with_args(args.iter().copied())
            .with_cwd(&home)
            .with_env_mode(LaunchEnvMode::Cleared)
            .with_env("HOME", &home)
            .with_env("XDG_CONFIG_HOME", home.join("config"))
            .with_env("XDG_CACHE_HOME", home.join("cache"))
            .with_env("XDG_DATA_HOME", home.join("data"))
            .with_env("SHELL", "/bin/sh")
            .with_env("PATH", "/usr/bin:/bin")
            .with_env("TERM", "xterm-256color")
            .with_env("COLORTERM", "truecolor")
            .with_env("LANG", "C.UTF-8")
            .with_env("LC_ALL", "C.UTF-8")
            .with_env("TZ", "UTC")
            .with_env("EMC_MOCK", "1");
        for (k, v) in extra_env {
            launch = launch.with_env(k, v);
        }
        let app = Self {
            session: Session::new(name),
            directory,
            actions: RefCell::new(vec![format!("spawn {bin} {args:?} {cols}x{rows}")]),
            checkpoint: Cell::new(0),
        };
        let defaults = OpenOptions::default();
        app.session
            .spawn_with(
                launch,
                RunOptions {
                    backend: defaults.backend,
                    program: String::new(),
                    args: Vec::new(),
                    profile: defaults.profile,
                    cols,
                    rows,
                    cwd: None,
                    env: Vec::new(),
                    wait_ready: Some(false),
                    restart: false,
                    timeouts: defaults.timeouts,
                    recording: defaults.recording,
                    style: defaults.style,
                },
            )
            .expect("spawn real application on a PTY");
        app
    }

    pub fn screen(&self) -> TerminalSnapshot {
        self.capture()
    }

    fn capture(&self) -> TerminalSnapshot {
        self.session
            .capture_snapshot(SnapshotScope::ActiveScreen)
            .expect("capture live terminal")
    }

    pub fn wait_text(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("visible {needle}"), |screen| {
            text(screen).contains(needle)
        })
    }

    pub fn wait_absent(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("absent {needle}"), |screen| {
            !text(screen).contains(needle)
        })
    }

    fn wait_screen(
        &self,
        reason: &str,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let screen = self.screen();
            self.assert_running(reason);
            if predicate(&screen) {
                self.record(reason, &screen);
                return screen;
            }
            assert!(
                Instant::now() < deadline,
                "timed out: {reason}\n{}",
                text(&screen)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn stable_screen(&self) -> TerminalSnapshot {
        let fingerprint = |screen: &TerminalSnapshot| {
            let mut bytes = serde_json::to_vec(screen.cells()).expect("serialize cells");
            let cursor = screen.cursor();
            bytes.extend_from_slice(&cursor.col().to_le_bytes());
            bytes.extend_from_slice(&cursor.row().to_le_bytes());
            bytes.push(u8::from(cursor.visible()));
            bytes
        };
        let deadline = Instant::now() + Duration::from_millis(5000);
        let mut screen = self.screen();
        let mut previous = fingerprint(&screen);
        let mut stable_since = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let candidate = self.screen();
            let current = fingerprint(&candidate);
            if current != previous {
                stable_since = Instant::now();
            }
            screen = candidate;
            previous = current;
            if stable_since.elapsed() >= Duration::from_millis(700) || Instant::now() >= deadline {
                break;
            }
        }
        screen
    }

    pub fn visual(&self, test_id: &str) {
        let screen = self.stable_screen();
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            screen,
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build Tuiscotti state sample");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let store = tuiscotti_latest::check::FrozenStore::new(
            &root.join("tests/functional/baselines"),
            &evidence_root().join("artifacts"),
        )
        .expect("open Tuiscotti frozen store");
        let outcome = store
            .check(&sample.to_bytes().expect("serialize Tuiscotti sample"))
            .expect("run Tuiscotti frozen check");
        assert!(
            outcome.is_pass(),
            "visual mismatch for {test_id}: {outcome:?}"
        );
        self.log(format!("visual {test_id} pass"));
    }

    pub fn key(&self, key: &str) {
        self.log(format!("key {key}"));
        self.session
            .execute(Operation::Key {
                keys: vec![key.to_string()],
                action: KeyAction::Press,
            })
            .unwrap_or_else(|e| panic!("key {key}: {e}"));
    }

    /// Send a quit key, then wait for process exit.
    pub fn finish_with_key(&self, key: &str) {
        self.key(key);
        self.session
            .execute(Operation::WaitExit {
                timeout_ms: Some(10_000),
            })
            .expect("application must exit");
        let OperationResult::State(state) =
            self.session.execute(Operation::State).expect("exit state")
        else {
            panic!("missing exit state");
        };
        self.log(format!("exit {:?}", state.exited));
    }

    fn assert_running(&self, reason: &str) {
        let OperationResult::State(state) = self
            .session
            .execute(Operation::State)
            .expect("process state")
        else {
            panic!("missing process state while {reason}");
        };
        assert!(
            state.exited.is_none(),
            "application exited while {reason}: {:?}\n{}",
            state.exited,
            text(&self.capture())
        );
    }

    fn log(&self, message: String) {
        self.actions.borrow_mut().push(message);
        std::fs::write(
            self.directory.join("actions.txt"),
            self.actions.borrow().join("\n"),
        )
        .expect("persist functional action trace");
    }

    fn record(&self, label: &str, screen: &TerminalSnapshot) {
        let number = self.checkpoint.get();
        self.checkpoint.set(number + 1);
        self.log(format!("checkpoint {number:03}: {label}"));
        let path = self.directory.join(format!("{number:03}"));
        std::fs::write(
            path.with_extension("json"),
            serde_json::to_vec(screen).unwrap(),
        )
        .expect("save terminal cells");
        std::fs::write(path.with_extension("txt"), text(screen)).expect("save terminal text");
    }
}

pub fn text(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn row_text(screen: &TerminalSnapshot, row: u32) -> String {
    screen
        .row(row)
        .expect("row in terminal")
        .iter()
        .map(SnapshotCell::text)
        .collect::<Vec<_>>()
        .join("")
        .trim_end()
        .to_string()
}

pub fn find(screen: &TerminalSnapshot, needle: &str) -> Point {
    assert!(!needle.is_empty(), "locator must not be empty");
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        if let Some(col) = line.find(needle) {
            return Point {
                x: col as u16,
                y: row as u16,
            };
        }
    }
    panic!("needle {needle:?} not on screen:\n{}", text(screen));
}
