//! Executor-independent cancellation shared by Rust consumers and transports.
//! @ref LLP 0068#2-synchronous-and-why — consumer owns execution; cancellation is carried explicitly
use crate::boundary::HostError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

#[derive(Default)]
struct State {
    aborted: bool,
    next: u64,
    callbacks: HashMap<u64, Arc<dyn Fn() + Send + Sync>>,
}

#[derive(Clone, Default)]
pub struct AbortSignal(Arc<Mutex<State>>);

#[derive(Clone, Default)]
pub struct AbortController {
    signal: AbortSignal,
}

impl AbortController {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn signal(&self) -> AbortSignal {
        self.signal.clone()
    }
    /// Idempotent. Callbacks run outside the state lock and may reenter.
    pub fn abort(&self) {
        let callbacks = {
            let mut state = self.signal.0.lock().unwrap();
            if state.aborted {
                return;
            }
            state.aborted = true;
            std::mem::take(&mut state.callbacks)
        };
        for callback in callbacks.into_values() {
            callback();
        }
    }
}

impl AbortSignal {
    pub fn aborted(&self) -> bool {
        self.0.lock().unwrap().aborted
    }
    pub fn check(&self) -> Result<(), HostError> {
        if self.aborted() {
            Err(HostError::Failed(
                "AbortError: The operation was aborted".into(),
            ))
        } else {
            Ok(())
        }
    }
    pub fn register(&self, callback: impl Fn() + Send + Sync + 'static) -> AbortRegistration {
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(callback);
        let mut state = self.0.lock().unwrap();
        let id = state.next;
        state.next += 1;
        if state.aborted {
            drop(state);
            callback();
        } else {
            state.callbacks.insert(id, callback);
        }
        AbortRegistration {
            state: Arc::downgrade(&self.0),
            id,
        }
    }
}

/// Unregisters on drop. A callback already claimed by abort may still run;
/// callbacks must own their resources rather than borrow a request's stack.
pub struct AbortRegistration {
    state: Weak<Mutex<State>>,
    id: u64,
}
impl Drop for AbortRegistration {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state.lock().unwrap().callbacks.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn registration_and_abort_never_lose_a_race() {
        for _ in 0..100 {
            let controller = AbortController::new();
            let signal = controller.signal();
            let count = Arc::new(AtomicUsize::new(0));
            let observed = count.clone();
            let worker = std::thread::spawn(move || {
                signal.register(move || {
                    observed.fetch_add(1, Ordering::SeqCst);
                })
            });
            controller.abort();
            let _registration = worker.join().unwrap();
            controller.abort();
            assert_eq!(count.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn removal_and_reentrant_callbacks() {
        let controller = AbortController::new();
        drop(
            controller
                .signal()
                .register(|| panic!("unregistered callback ran")),
        );
        let other = controller.clone();
        let _registration = controller.signal().register(move || other.abort());
        controller.abort();
        assert!(controller.signal().check().is_err());
    }
}
