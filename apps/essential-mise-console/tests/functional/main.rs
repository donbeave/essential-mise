//! tuiscotti baselines + interaction coverage for the four screens.
//!
//! The PTY harness mirrors `/tmp/tcb` (`support.rs` with
//! `tui_test::{Session, …}`); baselines live under `baselines/` as frozen
//! Tuiscotti samples. A missing or changed baseline fails; approve changes
//! only via the pinned `tuiscotti` CLI (`accept --candidate …`), never by
//! editing baselines by hand.

#![cfg(any(target_os = "macos", target_os = "linux"))]

mod screens;
#[allow(dead_code)]
mod support;
