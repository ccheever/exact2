//! One owner-thread Android runtime over the existing native runner and kernel.

use crate::bridge::{Bridge, General, GeneralRuntime};
use crate::wire::Encoder;
use exact_apple::abi::Hooks;
use exact_runner::DataSource;

/// A runtime and the reusable bytes its JNI presenter borrows for one turn.
/// `Bridge` makes the session thread confined; the registry refuses reentry.
pub struct Session<D: DataSource, G: GeneralRuntime<D> = General<D>> {
    /// Hooks installed before boot, owned by the platform adapter.
    pub hooks: Hooks,
    /// The shared native host, including its input and request executor.
    pub bridge: Bridge<D, G>,
    encoder: Encoder,
    output: Vec<u8>,
}

impl<D: DataSource, G: GeneralRuntime<D>> Default for Session<D, G> {
    fn default() -> Self {
        Self {
            hooks: Hooks::none(),
            bridge: Bridge::new(),
            encoder: Encoder::default(),
            output: Vec::new(),
        }
    }
}

impl<D: DataSource, G: GeneralRuntime<D>> Session<D, G> {
    /// Encode the latest shared-host batch. The returned byte count and
    /// [`Self::output`] form a lease ending at the next operation or destroy.
    pub fn publish(&mut self, length: u32) -> u32 {
        if self.bridge.binary_output() {
            return length;
        }
        let json = self.bridge.output_bytes(length as usize);
        if self.encoder.encode(json, &mut self.output).is_err() {
            self.encoder
                .encode(
                    br#"{"ops":[],"error":"invalid native batch","timers":false,"motion":false}"#,
                    &mut self.output,
                )
                .expect("literal refusal is valid");
        }
        u32::try_from(self.output.len()).unwrap_or(0)
    }

    /// An agent reply is JSON, consumed through the same borrowed buffer.
    pub fn publish_agent(&mut self, length: u32) -> u32 {
        self.output.clear();
        self.output
            .extend_from_slice(self.bridge.output_bytes(length as usize));
        u32::try_from(self.output.len()).unwrap_or(0)
    }

    /// Immutable wire bytes, valid until the next mutating operation.
    pub fn output(&self) -> &[u8] {
        if self.bridge.binary_output() {
            self.bridge.output_bytes(usize::MAX)
        } else {
            &self.output
        }
    }
}

/// Owner-thread handles, never pointers and never reused within the process.
/// A destroyed handle and an exhausted identifier are refused explicitly.
pub struct Registry<D: DataSource, G: GeneralRuntime<D> = General<D>> {
    entries: std::collections::BTreeMap<u32, std::rc::Rc<std::cell::RefCell<Session<D, G>>>>,
}

static NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn next_handle(counter: &std::sync::atomic::AtomicU32) -> u32 {
    use std::sync::atomic::Ordering;
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            (next != 0).then(|| next.checked_add(1).unwrap_or(0))
        })
        .unwrap_or(0)
}

impl<D: DataSource, G: GeneralRuntime<D>> Default for Registry<D, G> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
        }
    }
}

impl<D: DataSource, G: GeneralRuntime<D>> Registry<D, G> {
    /// Create a session. Zero denotes exhausted handle space.
    pub fn create(&mut self) -> u32 {
        let id = next_handle(&NEXT_HANDLE);
        if id == 0 {
            return 0;
        }
        self.entries.insert(
            id,
            std::rc::Rc::new(std::cell::RefCell::new(Session::default())),
        );
        id
    }

    /// Destroy a session. Executor retirement clears its wake callback before
    /// returning, so the adapter can release its callback context afterward.
    pub fn destroy(&mut self, id: u32) {
        self.entries.remove(&id);
    }

    /// Borrow one registry entry without retaining a borrow of the registry.
    pub fn get(&self, id: u32) -> Option<std::rc::Rc<std::cell::RefCell<Session<D, G>>>> {
        self.entries.get(&id).cloned()
    }
}

thread_local! {
    static REFUSAL: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Refuse a missing runtime or reentry without borrowing the active session.
pub fn refusal(agent: bool) -> u32 {
    REFUSAL.with(|out| {
        let mut out = out.borrow_mut();
        if agent {
            out.clear();
            out.extend_from_slice(br#"{"error":"missing or busy Android runtime"}"#);
        } else {
            Encoder::default()
                .encode(
                    br#"{"ops":[],"timers":false,"motion":false,"error":"missing or busy Android runtime"}"#,
                    &mut out,
                )
                .expect("literal refusal is valid");
        }
        out.len() as u32
    })
}

/// The most recent registry refusal, valid until another refused call.
pub fn refusal_ptr() -> *const u8 {
    REFUSAL.with(|out| out.borrow().as_ptr())
}

/// Run one non-reentrant transaction on a live owner-thread session.
pub fn with_session<D: DataSource + 'static, G: GeneralRuntime<D> + 'static, T>(
    registry: &'static std::thread::LocalKey<std::cell::RefCell<Registry<D, G>>>,
    id: u32,
    call: impl FnOnce(&mut Session<D, G>) -> T,
    refused: impl FnOnce() -> T,
) -> T {
    let entry = registry.with(|r| r.borrow().get(id));
    match entry.and_then(|entry| {
        let mut session = entry.try_borrow_mut().ok()?;
        Some(call(&mut session))
    }) {
        Some(value) => value,
        None => refused(),
    }
}

/// Perform one bridge call and publish its buffer as a single transaction.
pub fn transaction<D: DataSource + 'static, G: GeneralRuntime<D> + 'static>(
    registry: &'static std::thread::LocalKey<std::cell::RefCell<Registry<D, G>>>,
    id: u32,
    agent: bool,
    call: impl FnOnce(&mut Bridge<D, G>, Hooks) -> u32,
) -> u32 {
    with_session(
        registry,
        id,
        |session| {
            let length = call(&mut session.bridge, session.hooks);
            if agent {
                session.publish_agent(length)
            } else {
                session.publish(length)
            }
        },
        || refusal(agent),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroyed_handles_are_not_reused_and_reentry_is_refused() {
        let mut registry = Registry::<caltrain_data::Caltrain>::default();
        let old = registry.create();
        registry.destroy(old);
        registry.destroy(old);
        let next = registry.create();
        assert_ne!(old, next);
        assert!(registry.get(old).is_none());
        let entry = registry.get(next).unwrap();
        let held = entry.borrow_mut();
        assert!(entry.try_borrow_mut().is_err());
        drop(held);
        assert!(entry.try_borrow_mut().is_ok());
    }

    #[test]
    fn concurrent_owner_threads_never_resolve_each_others_handles() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let other_barrier = barrier.clone();
        let other = std::thread::spawn(move || {
            let mut registry = Registry::<caltrain_data::Caltrain>::default();
            other_barrier.wait();
            registry.create()
        });
        let mut registry = Registry::<caltrain_data::Caltrain>::default();
        barrier.wait();
        let own = registry.create();
        let foreign = other.join().unwrap();
        assert_ne!(own, foreign);
        assert!(registry.get(own).is_some());
        assert!(registry.get(foreign).is_none());
    }

    #[test]
    fn exhausted_handle_counter_never_wraps_to_a_previously_used_id() {
        let counter = std::sync::atomic::AtomicU32::new(u32::MAX);
        assert_eq!(next_handle(&counter), u32::MAX);
        assert_eq!(next_handle(&counter), 0);
        assert_eq!(next_handle(&counter), 0);
    }

    #[test]
    fn real_caltrain_event_changes_the_shared_runner() {
        let plan = contract::compile_path(std::path::Path::new("../../apps/caltrain/app.contract"))
            .unwrap();
        let plan = contract::bake(plan, caltrain_data::Caltrain)
            .unwrap()
            .encode();
        let mut session = Session::<caltrain_data::Caltrain>::default();
        let length = session
            .bridge
            .boot(&plan, caltrain_data::Caltrain, session.hooks, 390., 844.);
        session.publish(length);
        assert_eq!(&session.output()[..4], b"EXA1");
        let input = session
            .bridge
            .input_write(br#"{"op":"tree","target":"change-station","shallow":true}"#);
        let length = session.bridge.agent(input);
        let tree = String::from_utf8_lossy(session.bridge.output_bytes(length as usize));
        assert!(tree.contains("change-station"), "{tree}");
        let id = tree
            .split("\"roots\":[")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap();
        let id: u32 = id.parse().unwrap();
        let length = session.bridge.dispatch(id, 0, 0, 1.);
        session.publish(length);
        assert_eq!(&session.output()[..4], b"EXA1");
        let input = session.bridge.input_write(br#"{"op":"state"}"#);
        let length = session.bridge.agent(input);
        let state = String::from_utf8_lossy(session.bridge.output_bytes(length as usize));
        assert!(state.contains("\"screen\":\"stations\""), "{state}");
    }
}
