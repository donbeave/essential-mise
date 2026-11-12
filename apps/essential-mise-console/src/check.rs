//! "Checking your Mac": `status` off the UI thread.
//!
//! The UI never blocks on `mise`. A check runs on a worker thread and the app
//! polls its handle on every tick, so the screen can show a spinner at once
//! and a result as soon as it arrives.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::backend::Backend;
use crate::status::StatusReport;

/// One check, run to completion on this thread: `status` for `envs`, with
/// packages outside `keep` marked as skipped.
pub fn run(
    backend: &dyn Backend,
    envs: &[String],
    keep: &[String],
) -> Result<StatusReport, String> {
    backend
        .status(envs)
        .map(|report| report.scoped(keep))
        .map_err(|e| e.to_string())
}

/// A check running on a worker thread.
pub struct CheckHandle {
    rx: Receiver<Result<StatusReport, String>>,
}

/// Start a check on a worker thread.
pub fn spawn(backend: Arc<dyn Backend>, envs: Vec<String>, keep: Vec<String>) -> CheckHandle {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new()
        .name("emc-check".to_string())
        .spawn({
            let tx = tx.clone();
            move || {
                let _ = tx.send(run(&*backend, &envs, &keep));
            }
        });
    if let Err(e) = spawned {
        let _ = tx.send(Err(format!("could not start the check: {e}")));
    }
    CheckHandle { rx }
}

impl CheckHandle {
    /// The result, once the worker has finished. Never blocks.
    pub fn poll(&self) -> Option<Result<StatusReport, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err("the check stopped unexpectedly".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::MockBackend;
    use std::time::{Duration, Instant};

    #[test]
    fn poll_does_not_block_and_delivers_the_report() {
        let backend: Arc<dyn Backend> =
            Arc::new(MockBackend::new().with_status_delay(Duration::from_millis(150)));
        let started = Instant::now();
        let handle = spawn(backend, vec!["alpha".to_string()], Vec::new());
        assert!(handle.poll().is_none(), "the worker is still running");
        assert!(started.elapsed() < Duration::from_millis(100));
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(result) = handle.poll() {
                break result;
            }
            assert!(Instant::now() < deadline, "check never finished");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(result.expect("mock check succeeds").is_clean());
    }
}
