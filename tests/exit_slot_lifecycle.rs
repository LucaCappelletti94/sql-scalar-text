//! A value in an [`exit_slot::ExitSlot`] is dropped when the process exits.
//!
//! The engine harnesses keep their container in a `static`, and a static is
//! never dropped, so the `Drop` that removes the container never ran and every
//! test run left a PostgreSQL and a MySQL behind; testcontainers 0.27 has no
//! reaper. The slot exists to run that `Drop` at exit. Proven here with a value
//! whose `Drop` leaves a file, in a child process, so no daemon is needed.

#[path = "support/exit_slot.rs"]
mod exit_slot;

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU32, Ordering},
};

use exit_slot::ExitSlot;

struct LeavesMarker(PathBuf);

impl Drop for LeavesMarker {
    fn drop(&mut self) {
        // Infallible: a failed write leaves no marker, which the parent reads
        // as "not dropped" and reports.
        let _ = fs::write(&self.0, "dropped");
    }
}

static MARKER: ExitSlot<LeavesMarker> = ExitSlot::new();

const MARKER_ENV: &str = "EXIT_SLOT_MARKER";

/// Runs in a child process: fills the slot and returns; the test harness then
/// leaves through `process::exit`, which runs no destructors of its own.
#[test]
#[ignore = "child probe for value_is_dropped_when_the_process_exits"]
fn probe_fills_slot_and_exits() {
    let path = PathBuf::from(std::env::var_os(MARKER_ENV).expect("marker path"));
    MARKER.with(|| LeavesMarker(path), |_| ());
}

#[test]
fn a_panicking_drop_closure_neither_escapes_nor_skips_later_ones() {
    static LATER_RAN: AtomicU32 = AtomicU32::new(0);
    let drops: Vec<Box<dyn FnOnce() + Send>> = vec![
        Box::new(|| panic!("a drop that misbehaves")),
        Box::new(|| {
            LATER_RAN.fetch_add(1, Ordering::Relaxed);
        }),
    ];
    // The exit callback is an `extern "C"` boundary: a panic escaping it
    // aborts the process. This must return normally.
    exit_slot::run_drops(drops);
    assert_eq!(
        LATER_RAN.load(Ordering::Relaxed),
        1,
        "the closure after the panicking one did not run"
    );
}

#[test]
fn value_is_dropped_when_the_process_exits() {
    let dir = std::env::temp_dir().join(format!("exit-slot-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let marker = dir.join("marker");
    let output = Command::new(std::env::current_exe().expect("own path"))
        .args(["--ignored", "--exact", "probe_fills_slot_and_exits"])
        .env(MARKER_ENV, &marker)
        .output()
        .expect("run the probe binary");
    assert!(
        output.status.success(),
        "probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written = fs::read_to_string(&marker);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(
        written.as_deref().ok(),
        Some("dropped"),
        "the slot's value was not dropped when the probe exited"
    );
}

static COUNTER: ExitSlot<u32> = ExitSlot::new();

#[test]
fn init_runs_once_and_later_calls_read_the_same_value() {
    static INITS: AtomicU32 = AtomicU32::new(0);
    let init = || {
        INITS.fetch_add(1, Ordering::Relaxed);
        7
    };
    let first = COUNTER.with(init, |value| *value);
    let second = COUNTER.with(init, |value| *value);
    assert_eq!((first, second), (7, 7));
    assert_eq!(
        INITS.load(Ordering::Relaxed),
        1,
        "init ran again on a filled slot"
    );
}
