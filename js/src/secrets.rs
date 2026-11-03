//! A secret lease survives JavaScript awaits; acquisition never occupies an
//! ordered host completion or a worker owner's turn.
use exact_runner::{Dispatch, SecretLease, SecretWait, SharedSecrets, Store};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) const TICKET: u64 = u64::MAX - 4;
type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) struct Shared(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    backend: Option<Arc<dyn SharedSecrets>>,
    wake: Option<Wake>,
    calls: HashMap<u64, Entry>,
}

struct Entry {
    name: String,
    wait: Option<Box<dyn SecretWait>>,
    result: Option<Result<Arc<dyn SecretLease>, String>>,
    dispatched: bool,
    requested: bool,
    started: bool,
    fetch: Option<u64>,
}

impl Shared {
    pub(crate) fn configure(&self, backend: Arc<dyn SharedSecrets>) {
        self.0.lock().unwrap().backend = Some(backend);
    }

    pub(crate) fn replacement(&self) -> Self {
        let state = self.0.lock().unwrap();
        Self(Arc::new(Mutex::new(State {
            backend: state.backend.clone(),
            wake: state.wake.clone(),
            calls: HashMap::new(),
        })))
    }

    pub(crate) fn wake(&self, wake: Wake) {
        self.0.lock().unwrap().wake = Some(wake);
    }

    pub(crate) fn contains(&self, call: u64) -> bool {
        self.0
            .lock()
            .unwrap()
            .calls
            .get(&call)
            .is_some_and(|e| !e.started)
    }

    pub(crate) fn holds(&self, call: u64) -> bool {
        self.0
            .lock()
            .unwrap()
            .calls
            .get(&call)
            .is_some_and(|e| e.started)
    }

    pub(crate) fn begin_effect(&self, call: u64) -> Result<(), String> {
        if self.holds(call) {
            self.lease(call)?.1.begin_effect()?;
        }
        Ok(())
    }

    pub(crate) fn sent(&self, call: u64, fetch: Option<u64>) {
        if let Some(entry) = self.0.lock().unwrap().calls.get_mut(&call) {
            entry.fetch = fetch;
        }
    }

    pub(crate) fn keeps_sent(&self, fetch: u64) -> bool {
        self.0
            .lock()
            .unwrap()
            .calls
            .values()
            .any(|e| e.started && e.fetch == Some(fetch))
    }

    pub(crate) fn retire(&self, call: u64) {
        self.0.lock().unwrap().calls.remove(&call);
    }

    pub(crate) fn retire_fetch(&self, fetch: u64) -> bool {
        let mut state = self.0.lock().unwrap();
        let call = state
            .calls
            .iter()
            .find(|(_, entry)| entry.fetch == Some(fetch))
            .map(|(call, _)| *call);
        if let Some(call) = call {
            state.calls.remove(&call);
        }
        call.is_some()
    }

    pub(crate) fn unparked(&self, live: &[u64]) -> Vec<u64> {
        self.0
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter_map(|(call, entry)| (!entry.started && !live.contains(call)).then_some(*call))
            .collect()
    }

    pub(crate) fn cancel_wait(&self, call: u64) {
        let mut state = self.0.lock().unwrap();
        if state.calls.get(&call).is_some_and(|e| !e.started) {
            state.calls.remove(&call);
        }
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().calls.clear();
    }

    pub(crate) fn dispatch(&self, call: u64) -> Option<Dispatch> {
        let mut state = self.0.lock().unwrap();
        let entry = state.calls.get_mut(&call)?;
        if entry.started || entry.dispatched {
            return None;
        }
        entry.requested = true;
        if entry.result.is_none() {
            entry.result = entry.wait.as_mut().and_then(|wait| wait.take());
        }
        if entry.result.is_none() {
            return Some(Dispatch::Held);
        }
        entry.wait = None;
        entry.dispatched = true;
        Some(Dispatch::Again)
    }

    pub(crate) fn release(&self) -> Vec<(u64, Dispatch)> {
        let calls: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter_map(|(call, entry)| entry.requested.then_some(*call))
            .collect();
        calls
            .into_iter()
            .filter_map(|call| match self.dispatch(call) {
                Some(Dispatch::Again) => Some((call, Dispatch::Again)),
                _ => None,
            })
            .collect()
    }

    fn lease(&self, call: u64) -> Result<(String, Arc<dyn SecretLease>), String> {
        let state = self.0.lock().unwrap();
        let entry = state.calls.get(&call).ok_or("secret lease was retired")?;
        let lease = entry
            .result
            .as_ref()
            .ok_or("secret lease is not ready")?
            .clone()?;
        Ok((entry.name.clone(), lease))
    }

    pub(crate) fn ready(&self, call: u64, store: Option<&mut Store>) -> String {
        let result = (|| {
            let store = store.ok_or_else(|| super::door::no_store("store.exclusive"))?;
            let (name, lease) = self.lease(call)?;
            store
                .check_exclusive(&name)
                .map_err(super::door::store_error)?;
            match lease.read()? {
                Some(value) => store.set_durable(&name, &value),
                None => store.forget_durable(&name),
            }
            .map_err(super::door::store_error)?;
            self.0
                .lock()
                .unwrap()
                .calls
                .get_mut(&call)
                .ok_or("secret lease was retired")?
                .started = true;
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => String::new(),
            Err(error) => {
                self.0.lock().unwrap().calls.remove(&call);
                error
            }
        }
    }

    pub(crate) fn door(
        &self,
        op: u32,
        a: &str,
        b: &str,
        store: Option<&mut Store>,
    ) -> Result<Option<String>, String> {
        let call = a.parse::<u64>().map_err(|_| "secret lease: invalid call")?;
        if op == 25 {
            let entry = self.0.lock().unwrap().calls.remove(&call);
            if b == "commit" {
                if let Some(Ok(lease)) = entry.and_then(|entry| entry.result) {
                    lease.finish()?;
                }
            }
            return Ok(None);
        }
        let store = store.ok_or_else(|| super::door::no_store("store.exclusive"))?;
        if op == 24 {
            store.check_exclusive(b).map_err(super::door::store_error)?;
            let mut state = self.0.lock().unwrap();
            if state.calls.contains_key(&call) {
                return Err("the answer already holds a secret lease".into());
            }
            let backend = state
                .backend
                .as_ref()
                .ok_or("shared secrets are unavailable on this host")?;
            let wait = backend.acquire(b, state.wake.clone().unwrap_or_else(|| Arc::new(|| {})));
            state.calls.insert(
                call,
                Entry {
                    name: b.into(),
                    wait: Some(wait),
                    result: None,
                    dispatched: false,
                    requested: false,
                    started: false,
                    fetch: None,
                },
            );
        } else {
            let write: serde_json::Value = serde_json::from_str(b).map_err(|e| e.to_string())?;
            let (name, lease) = self.lease(call)?;
            if write["name"].as_str() != Some(&name) {
                return Err("secret lease does not cover this name".into());
            }
            store
                .check_exclusive(&name)
                .map_err(super::door::store_error)?;
            let value = match &write["value"] {
                serde_json::Value::String(value) => Some(value.as_str()),
                serde_json::Value::Null => None,
                _ => return Err("secret lease: invalid value".into()),
            };
            lease.write(value)?;
            match value {
                Some(value) => store.set_durable(&name, value),
                None => store.forget_durable(&name),
            }
            .map_err(super::door::store_error)?;
        }
        Ok(None)
    }
}

impl super::Module {
    pub(crate) fn prune_secret_calls(&mut self) {
        let live: Vec<_> = self
            .parked
            .iter()
            .chain(self.streams.iter())
            .map(|(_, parked)| parked.call)
            .collect();
        let unparked = self.host.secrets.unparked(&live);
        self.forget_calls(unparked);
    }
}
