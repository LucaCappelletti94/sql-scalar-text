//! A process-wide value that is dropped when the process exits.
//!
//! Include in a test binary with:
//!
//! ```rust,ignore
//! #[path = "support/exit_slot.rs"]
//! mod exit_slot;
//! ```
//!
//! A `static` is never dropped, and the test harness leaves through
//! `process::exit`, which runs no destructors either. A testcontainers guard
//! kept in a `static` therefore never removes its container, and nothing else
//! does; testcontainers 0.27 has no reaper. The slot fills once, like
//! `OnceLock`, and registers one `atexit` callback that drops every filled
//! slot: a normal finish, a failed test and a panic all pass through it. A
//! terminating signal does not: `SIGINT`, `SIGTERM` and `SIGKILL` end the
//! process without libc's exit processing, so an interrupted or timed-out
//! run still leaves its container behind.

use parking_lot::Mutex;
use std::{
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Once,
};

pub struct ExitSlot<T>(Mutex<Option<T>>);

/// What to drop at exit, one entry per filled slot. Each entry names its
/// slot through a `'static` reference and captures nothing else.
static DROPS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

static REGISTER: Once = Once::new();

unsafe extern "C" {
    /// libc. The callback is a plain `extern "C" fn` with no captures and
    /// touches only Rust-owned statics, so nothing is owned across the
    /// boundary in either direction and the pointer is valid for the whole
    /// process.
    fn atexit(callback: extern "C" fn()) -> i32;
}

type Drops = Mutex<Vec<Box<dyn FnOnce() + Send>>>;

/// The `atexit` callback. A panic unwinding out of an `extern "C"` function
/// aborts the process, which would fail a green test run at the last moment
/// and leave every later slot undropped, so nothing here may unwind.
extern "C" fn drop_every_slot() {
    drain(&DROPS);
}

/// Run every registered drop, in batches, until the registry is empty.
///
/// The lock is released before each batch runs: a destructor may fill
/// another slot, which registers on the same registry, and that late
/// registration must both not deadlock and still be run before exit.
fn drain(registry: &Drops) {
    loop {
        let batch = std::mem::take(&mut *registry.lock());
        if batch.is_empty() {
            return;
        }
        run_drops(batch);
    }
}

/// Run each drop in order, containing a panic so the rest still run. A
/// `Drop` that panics is a defect in the value's type; the exit path
/// survives it rather than compounding it into an abort.
fn run_drops(drops: Vec<Box<dyn FnOnce() + Send>>) {
    for drop_slot in drops {
        if catch_unwind(AssertUnwindSafe(drop_slot)).is_err() {
            // Not `eprintln!`: it panics when stderr is closed, and nothing
            // here may unwind.
            let _ = writeln!(
                std::io::stderr().lock(),
                "exit slot: a value's destructor panicked; continuing with the rest"
            );
        }
    }
}

impl<T: Send + 'static> ExitSlot<T> {
    pub const fn new() -> Self {
        Self(Mutex::new(None))
    }

    /// Fill the slot with `init` if it is empty, then apply `read` to the
    /// value. The slot's lock is held during `read`, so `read` must not touch
    /// this slot.
    ///
    /// # Panics
    ///
    /// Panics if the process refuses the exit callback, before the value is
    /// stored, so the value `init` produced is dropped by unwinding rather
    /// than kept with nothing left to drop it.
    pub fn with<R>(&'static self, init: impl FnOnce() -> T, read: impl FnOnce(&T) -> R) -> R {
        let mut slot = self.0.lock();
        if slot.is_none() {
            let value = init();
            REGISTER.call_once(|| {
                // SAFETY: `drop_every_slot` is an `extern "C" fn` without
                // captures, valid for the life of the process, as `atexit`
                // requires. `Once` registers it a single time.
                let registered = unsafe { atexit(drop_every_slot) };
                assert_eq!(
                    registered, 0,
                    "register the exit callback that drops the slot"
                );
            });
            DROPS.lock().push(Box::new(move || self.clear()));
            *slot = Some(value);
        }
        read(slot.as_ref().expect("filled just above"))
    }

    /// Take the value under the lock, drop it after: a destructor must never
    /// run while the slot's own mutex is held.
    fn clear(&self) {
        let value = self.0.lock().take();
        drop(value);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn a_panicking_drop_closure_neither_escapes_nor_skips_later_ones() {
        static LATER_RAN: AtomicBool = AtomicBool::new(false);
        let drops: Vec<Box<dyn FnOnce() + Send>> = vec![
            Box::new(|| panic!("a drop that misbehaves")),
            Box::new(|| {
                LATER_RAN.store(true, Ordering::Relaxed);
            }),
        ];
        // The exit callback is an `extern "C"` boundary: a panic escaping it
        // aborts the process. This must return normally.
        run_drops(drops);
        assert!(
            LATER_RAN.load(Ordering::Relaxed),
            "the closure after the panicking one did not run"
        );
    }

    #[test]
    fn a_drop_registered_during_the_drain_runs_and_does_not_deadlock() {
        static REGISTRY: Drops = Mutex::new(Vec::new());
        static SECOND_RAN: AtomicBool = AtomicBool::new(false);
        REGISTRY.lock().push(Box::new(|| {
            // What a destructor filling another slot does: register on the
            // registry being drained.
            REGISTRY
                .lock()
                .push(Box::new(|| SECOND_RAN.store(true, Ordering::Relaxed)));
        }));
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drain(&REGISTRY);
            let _ = done.send(());
        });
        assert!(
            finished
                .recv_timeout(std::time::Duration::from_secs(10))
                .is_ok(),
            "the drain held its registry lock while running a drop: deadlock"
        );
        assert!(
            SECOND_RAN.load(Ordering::Relaxed),
            "a drop registered during the drain never ran"
        );
    }
}
