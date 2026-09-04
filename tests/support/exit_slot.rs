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
//! slot: a normal finish, a failed test and a panic all pass through it.
//! `SIGKILL` does not; that is the one path this cannot cover.

use std::sync::{Mutex, Once, PoisonError};

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

extern "C" fn drop_every_slot() {
    let drops = std::mem::take(&mut *DROPS.lock().unwrap_or_else(PoisonError::into_inner));
    for drop_slot in drops {
        drop_slot();
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
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
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
            DROPS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Box::new(move || self.clear()));
            *slot = Some(value);
        }
        read(slot.as_ref().expect("filled just above"))
    }

    fn clear(&self) {
        drop(self.0.lock().unwrap_or_else(PoisonError::into_inner).take());
    }
}
