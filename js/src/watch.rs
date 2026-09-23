//! What another thread's interrupt reaches (LLP 1048.000 D10): the module's
//! live runtime, if one is up, and whether an interrupt is outstanding.

use crate::engine::{Engine, Raw};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
pub(crate) struct Watch {
    engine: Mutex<Option<Raw>>,
    interrupted: AtomicBool,
}

impl Watch {
    /// Stop the call running now, or the next one to start: Hermes raises
    /// the interrupt at its next async break check. The lock keeps the
    /// runtime alive while it is triggered.
    pub(crate) fn trigger(&self) {
        let engine = self.lock();
        self.interrupted.store(true, Ordering::SeqCst);
        if let Some(raw) = *engine {
            raw.interrupt();
        }
    }

    /// Whether an interrupt ended the call that just failed. Taking it lets
    /// the next call run.
    pub(crate) fn take(&self) -> bool {
        self.interrupted.swap(false, Ordering::SeqCst)
    }

    fn lock(&self) -> MutexGuard<'_, Option<Raw>> {
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// An engine its watch reaches from the moment it exists until just before
/// it is destroyed. An interrupt that came first stops its first execution.
pub(crate) struct Watched {
    engine: Engine,
    watch: Arc<Watch>,
}

impl Watched {
    pub(crate) fn new(engine: Engine, watch: Arc<Watch>) -> Watched {
        let raw = engine.raw();
        let mut attached = watch.lock();
        *attached = Some(raw);
        if watch.interrupted.load(Ordering::SeqCst) {
            raw.interrupt();
        }
        drop(attached);
        Watched { engine, watch }
    }
}

impl Deref for Watched {
    type Target = Engine;
    fn deref(&self) -> &Engine {
        &self.engine
    }
}

impl DerefMut for Watched {
    fn deref_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        // Before the engine's own drop destroys the runtime: an interrupt
        // holding the lock finishes first, and none reaches it after.
        let mut attached = self.watch.lock();
        if *attached == Some(self.engine.raw()) {
            *attached = None;
        }
    }
}
