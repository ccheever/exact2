//! Executor-independent event subscriptions for Rust consumers.
//!
//! A watch exposes a blocking/pollable [`std::sync::mpsc::Receiver`]; Ibex
//! does not choose a future type or async runtime for its caller. The handle's
//! sole operation is cancellation. Dropping it performs the same cancellation
//! so a forgotten watch cannot retain its source indefinitely.
//!
//! @ref LLP 0068#5-open-questions — OQ3 is answered by an mpsc receiver, not an executor

use std::sync::{Arc, Mutex};

struct SubscriptionState {
    cancel: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}

/// A live event watch.
///
/// `unsubscribe` is idempotent. Work a delivery loop already reserved may
/// finish; no delivery that was still queued when `unsubscribe` began may
/// start afterward.
pub struct Subscription {
    state: Arc<SubscriptionState>,
}

impl Subscription {
    pub(crate) fn new(cancel: impl FnOnce() + Send + 'static) -> Self {
        Self {
            state: Arc::new(SubscriptionState {
                cancel: Mutex::new(Some(Box::new(cancel))),
            }),
        }
    }

    /// Stop the watch and release its source-side registration.
    pub fn unsubscribe(&self) {
        let cancel = self
            .state
            .cancel
            .lock()
            .expect("event subscription poisoned")
            .take();
        if let Some(cancel) = cancel {
            cancel();
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{mpsc, Weak},
    };

    /// L3's first source is deliberately test-only. WebSocket is the first
    /// production source, in L4; this proves the Rust door without pulling an
    /// engine or an executor into the crate.
    struct TestEventSource<T> {
        state: Arc<Mutex<TestState<T>>>,
    }

    struct TestState<T> {
        next: u64,
        watchers: HashMap<u64, mpsc::Sender<T>>,
    }

    impl<T: Clone + Send + 'static> TestEventSource<T> {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(TestState {
                    next: 1,
                    watchers: HashMap::new(),
                })),
            }
        }

        fn watch(&self) -> (mpsc::Receiver<T>, Subscription) {
            let (sender, receiver) = mpsc::channel();
            let id = {
                let mut state = self.state.lock().unwrap();
                let id = state.next;
                state.next += 1;
                state.watchers.insert(id, sender);
                id
            };
            let source: Weak<Mutex<TestState<T>>> = Arc::downgrade(&self.state);
            let subscription = Subscription::new(move || {
                if let Some(source) = source.upgrade() {
                    source.lock().unwrap().watchers.remove(&id);
                }
            });
            (receiver, subscription)
        }

        fn emit(&self, value: T) {
            let mut state = self.state.lock().unwrap();
            state
                .watchers
                .retain(|_, watcher| watcher.send(value.clone()).is_ok());
        }
    }

    #[test]
    fn rust_watch_is_a_receiver_and_unsubscribe_disconnects_it() {
        let source = TestEventSource::new();
        let (events, subscription) = source.watch();
        source.emit("first");
        assert_eq!(events.recv().unwrap(), "first");

        subscription.unsubscribe();
        source.emit("late");
        assert_eq!(events.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }
}
