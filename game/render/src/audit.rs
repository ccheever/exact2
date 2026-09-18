//! Allocation/upload accounting, independent of simulation saves and perf sampling.
use serde_json::{json, Value};
use std::{cell::RefCell, rc::Rc};

const KEYS: [&str; 11] = [
    "shaderModules",
    "renderPipelines",
    "computePipelines",
    "bindGroupLayouts",
    "textures",
    "textureBytesUploaded",
    "vertexBuffers",
    "indexBuffers",
    "meshBytesUploaded",
    "bufferReallocations",
    "slotCapacityGrowth",
];
pub(crate) const SHADER: usize = 0;
pub(crate) const PIPELINE: usize = 1;
pub(crate) const LAYOUT: usize = 3;
pub(crate) const TEXTURE: usize = 4;
pub(crate) const TEX_BYTES: usize = 5;
pub(crate) const VERTEX: usize = 6;
pub(crate) const INDEX: usize = 7;
pub(crate) const MESH_BYTES: usize = 8;
pub(crate) const GROW: usize = 9;
pub(crate) const SLOTS: usize = 10;
#[derive(Default)]
struct State {
    after: bool,
    tick: u64,
    before: [u64; 11],
    during: [u64; 11],
    exceptions: [u64; 11],
    exception_events: u64,
    violations: u64,
    last_exception: Option<Value>,
    last: Option<Value>,
    name: Option<(String, bool)>,
    streaming: [u64; 2],
    restoring: bool,
    restore: [u64; 11],
    restore_events: u64,
    last_restore: Option<Value>,
}
#[derive(Clone, Default)]
pub(crate) struct Audit(Rc<RefCell<State>>);
thread_local! { static ACTIVE: RefCell<Option<Audit>> = const { RefCell::new(None) }; }
pub(crate) struct Scope(Option<Audit>);
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|a| *a.borrow_mut() = self.0.take());
    }
}
pub(crate) struct Name(Audit, Option<(String, bool)>);
impl Drop for Name {
    fn drop(&mut self) {
        self.0 .0.borrow_mut().name = self.1.take();
    }
}
pub(crate) struct Restore(Audit, bool);
impl Drop for Restore {
    fn drop(&mut self) {
        self.0 .0.borrow_mut().restoring = self.1;
    }
}
impl Audit {
    pub fn restoring(&self, restoring: bool) -> Restore {
        let old = std::mem::replace(&mut self.0.borrow_mut().restoring, restoring);
        Restore(self.clone(), old)
    }
    pub fn current() -> Self {
        ACTIVE.with(|a| a.borrow().clone().unwrap_or_default())
    }
    pub fn enter(&self) -> Scope {
        Scope(ACTIVE.with(|a| a.replace(Some(self.clone()))))
    }
    pub fn name(&self, name: &str, exception: bool) -> Name {
        Name(
            self.clone(),
            self.0.borrow_mut().name.replace((name.into(), exception)),
        )
    }
    pub fn tick(&self, tick: u64) {
        self.0.borrow_mut().tick = tick;
    }
    // Once reached, readiness accounting never resets: format/device recovery is work in play.
    pub fn ready(&self) {
        self.0.borrow_mut().after = true;
    }
    pub fn json(&self, device: bool) -> Value {
        let s = self.0.borrow();
        let counts = |values: &[u64; 11]| -> Value {
            KEYS.iter()
                .zip(values)
                .map(|(k, v)| ((*k).to_owned(), json!(v)))
                .collect()
        };
        let mut before = counts(&s.before);
        let mut after = counts(&s.during);
        before["streamingVertexBytes"] = json!(s.streaming[0]);
        after["streamingVertexBytes"] = json!(s.streaming[1]);
        after["violations"] = json!(s.violations);
        after["declaredExceptions"] = json!({"events":s.exception_events,
            "counts":counts(&s.exceptions), "last":s.last_exception});
        json!({"device":device,"beforeReady":before,"afterReady":after,"lastAfterReady":s.last,
            "restoreUploads":{"events":s.restore_events,"counts":counts(&s.restore),"last":s.last_restore}})
    }
}
pub(crate) fn record(what: usize, name: &str, amount: u64) {
    if amount == 0 {
        return;
    }
    ACTIVE.with(|active| {
        let active = active.borrow();
        let Some(a) = active.as_ref() else { return };
        let mut s = a.0.borrow_mut();
        let (name, exception) = s
            .name
            .as_ref()
            .map_or((name, false), |(n, e)| (n.as_str(), *e));
        let event = s
            .after
            .then(|| json!({"what":KEYS[what],"name":name,"tick":s.tick}));
        if !s.after {
            s.before[what] += amount;
        } else if exception {
            s.exceptions[what] += amount;
            s.exception_events += 1;
            s.last_exception = event;
        } else {
            s.during[what] += amount;
            s.violations += 1;
            if s.restoring {
                s.restore[what] += amount;
                s.restore_events += 1;
                s.last_restore = event.clone();
            }
            s.last = event;
        }
    });
}
pub(crate) fn streaming(bytes: u64) {
    ACTIVE.with(|a| {
        if let Some(a) = a.borrow().as_ref() {
            let mut s = a.0.borrow_mut();
            let i = usize::from(s.after);
            s.streaming[i] += bytes;
        }
    });
}
