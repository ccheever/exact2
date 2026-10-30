//! The process's one pad registry (LLP 1133.022 D4, D10 item 1): the pads
//! the host reads (evdev on Linux, XInput's slots on Windows), each by its
//! slot index, its identity and latest standard reading, the pad in use
//! (`current`), and the outputs a package claims.
//!
//! The host's readers write it (`connect`, `read`, `disconnect`) on the
//! window's loop, beside the canvas routing that already reads the same
//! devices, so an index means one pad to a canvas and to a package. A
//! package reads it through its context (`exact_linux::package::Context::
//! pads`): the list, a change notice, and effects by index.
//!
//! **Claims.** Each effect is a claim made by one client (a session's
//! package instance) on a device, not a slot: a disconnect ends every
//! claim on the device, which answers `preempted`; a pad that returns is a
//! new connection. A new effect on a device preempts the one playing there,
//! whichever client made it; `reset` ends the device's effect, and a
//! client's `end` (its `destroy`) only its own claims.
//!
//! **A test build's pad.** With `EXACT_AGENT_PADS` set to a JSON list of
//! `{"index", "id", "vendor", "product", "pressed", "leaves"}` (a production
//! bake drops every `EXACT_AGENT_*` variable, `app.rs`),
//! those pads connect at the registry's first use, each pressing its
//! `pressed` buttons once and disconnecting `leaves` ms later on the host's
//! clock when it says, and their motors are a recorder whose every request
//! is journaled: the real path from the registry on, with no device.

use crate::gamepad::Reading;
use std::sync::{Arc, Mutex, OnceLock};

/// A claim's end: `complete` or `preempted`.
pub type Done = Box<dyn FnOnce(&'static str) + Send>;
type Watcher = Arc<dyn Fn() + Send + Sync>;

/// What a pad's device can be asked to do.
pub trait Output: Send {
    /// Both motors at `strong` and `weak`, in `[0, 1]`, for `ms`; false when
    /// the device refused.
    fn rumble(&mut self, strong: f32, weak: f32, ms: u32) -> bool;
    /// The motors stopped.
    fn stop(&mut self);
}

/// Who a pad is, as its host can tell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// Its name.
    pub id: String,
    /// Its USB vendor and product ids; 0 where the host cannot tell.
    pub vendor: u16,
    /// See `vendor`.
    pub product: u16,
    /// Its driver in sysfs (`xpad`, `hid-playstation`), on Linux.
    pub driver: Option<String>,
    /// XInput's slot, on Windows.
    pub user_index: Option<u8>,
}

impl Identity {
    /// The face buttons' scheme (D2): by vendor; XInput's pads are Xbox-class.
    pub fn family(&self) -> &'static str {
        match self.vendor {
            _ if self.user_index.is_some() => "xbox",
            0x045e => "xbox",
            0x054c => "playstation",
            0x057e => "nintendo",
            _ => "generic",
        }
    }
}

/// One connected pad, as the registry lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pad {
    /// Its slot.
    pub index: u8,
    /// Who it is.
    pub identity: Identity,
    /// Its latest standard reading.
    pub reading: Reading,
    /// Whether its device takes `dual-rumble`.
    pub rumble: bool,
}

struct Slot {
    identity: Identity,
    reading: Reading,
    /// Which connection this is: a pad that returns is a new one.
    connection: u64,
    output: Option<Box<dyn Output>>,
}

struct Claim {
    client: u64,
    connection: u64,
    index: u8,
    strong: f32,
    weak: f32,
    ms: u32,
    /// When it starts and ends, on the host's clock.
    start: f64,
    end: f64,
    started: bool,
    done: Option<Done>,
}

#[derive(Default)]
struct Registry {
    slots: Vec<Option<Slot>>,
    current: Option<u8>,
    claims: Vec<Claim>,
    watchers: Vec<(u64, Watcher)>,
    connections: u64,
    clients: u64,
    now: f64,
    lines: Vec<String>,
    /// Test pads that disconnect, and when.
    leaving: Vec<(f64, u8)>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let r = Mutex::new(Registry::default());
        if let Ok(pads) = std::env::var("EXACT_AGENT_PADS") {
            inject_into(&r, &pads);
        }
        r
    })
}

/// Run `f` on the registry, then tell the watchers and the ended claims
/// outside its lock, so either may call it again.
fn with<T>(f: impl FnOnce(&mut Registry, &mut Vec<(Done, &'static str)>) -> (T, bool)) -> T {
    let mut ended = Vec::new();
    let (out, changed, watchers) = {
        let mut r = registry().lock().unwrap_or_else(|e| e.into_inner());
        let (out, changed) = f(&mut r, &mut ended);
        let watchers: Vec<Watcher> = match changed {
            true => r.watchers.iter().map(|(_, w)| w.clone()).collect(),
            false => Vec::new(),
        };
        (out, changed, watchers)
    };
    for (done, how) in ended {
        done(how);
    }
    if changed {
        for w in watchers {
            w();
        }
    }
    out
}

/// A standard button is down past half its travel.
const DOWN: f32 = 0.5;

/// The host's reader: pad `index` connected, with `output` where its device
/// takes effects.
pub fn connect(index: u8, identity: Identity, output: Option<Box<dyn Output>>) {
    with(|r, ended| {
        r.end_device(index, ended);
        let i = index as usize;
        if r.slots.len() <= i {
            r.slots.resize_with(i + 1, || None);
        }
        r.connections += 1;
        r.slots[i] = Some(Slot {
            identity,
            reading: Reading::default(),
            connection: r.connections,
            output,
        });
        ((), true)
    })
}

/// The host's reader: pad `index` reads `reading`. A button that went down
/// makes it the pad in use; a stick's drift does not.
pub fn read(index: u8, reading: Reading) {
    with(|r, _| {
        let Some(Some(slot)) = r.slots.get_mut(index as usize) else {
            return ((), false);
        };
        let pressed = (reading.buttons.iter().zip(slot.reading.buttons))
            .any(|(now, was)| *now >= DOWN && was < DOWN);
        slot.reading = reading;
        let took = pressed && r.current != Some(index);
        if took {
            r.current = Some(index);
        }
        ((), took)
    })
}

/// The host's reader: pad `index` is gone, with every claim on it.
pub fn disconnect(index: u8) {
    with(|r, ended| ((), r.disconnect(index, ended)))
}

/// The connected pads by index, and the one in use.
pub fn list() -> (Vec<Pad>, Option<u8>) {
    let r = registry().lock().unwrap_or_else(|e| e.into_inner());
    let pads = (r.slots.iter().enumerate())
        .filter_map(|(i, s)| {
            let s = s.as_ref()?;
            Some(Pad {
                index: i as u8,
                identity: s.identity.clone(),
                reading: s.reading,
                rumble: s.output.is_some(),
            })
        })
        .collect();
    (pads, r.current)
}

/// A new client's id: a session's package instance.
pub fn client() -> u64 {
    with(|r, _| {
        r.clients += 1;
        (r.clients, false)
    })
}

/// `client` is told each change: a pad connected or went, or another pad
/// is in use. Any thread; never under the registry's lock.
pub fn watch(client: u64, f: impl Fn() + Send + Sync + 'static) {
    with(|r, _| {
        r.watchers.retain(|(c, _)| *c != client);
        r.watchers.push((client, Arc::new(f)));
        ((), false)
    })
}

/// `client` plays `dual-rumble` on pad `index` after `delay` ms, for `ms`:
/// it preempts the effect playing on that device. `done` hears how it
/// ended. `Err` with the code when there is no such pad or it has no motors.
#[allow(clippy::too_many_arguments)]
pub fn rumble(
    client: u64,
    index: u8,
    strong: f32,
    weak: f32,
    delay: f64,
    ms: u32,
    done: Done,
) -> Result<(), &'static str> {
    with(|r, ended| {
        let Some(Some(slot)) = r.slots.get(index as usize) else {
            return (Err("NotFoundError"), false);
        };
        if slot.output.is_none() {
            return (Err("NotSupportedError"), false);
        }
        let connection = slot.connection;
        r.preempt(index, ended);
        let start = r.now + delay.max(0.);
        r.claims.push(Claim {
            client,
            connection,
            index,
            strong: strong.clamp(0., 1.),
            weak: weak.clamp(0., 1.),
            ms,
            start,
            end: start + f64::from(ms),
            started: false,
            done: Some(done),
        });
        r.run(ended);
        (Ok(()), false)
    })
}

/// The effect playing on pad `index` ends, `preempted`; `Err` for no pad.
pub fn reset(index: u8) -> Result<(), &'static str> {
    with(|r, ended| {
        if !r.slots.get(index as usize).is_some_and(Option::is_some) {
            return (Err("NotFoundError"), false);
        }
        r.preempt(index, ended);
        (Ok(()), false)
    })
}

/// `client` is gone: its claims end, and it hears no more changes.
pub fn end(client: u64) {
    with(|r, ended| {
        r.watchers.retain(|(c, _)| *c != client);
        let mine: Vec<u8> = (r.claims.iter())
            .filter(|c| c.client == client)
            .map(|c| c.index)
            .collect();
        for index in mine {
            r.preempt(index, ended);
        }
        ((), false)
    })
}

/// The host's clock moved to `now_ms`: effects due start, and those past
/// their span complete.
pub fn tick(now_ms: f64) {
    with(|r, ended| {
        r.now = now_ms;
        r.run(ended);
        let (left, stay) = (r.leaving.iter()).partition(|(at, _)| *at <= now_ms);
        r.leaving = stay;
        let mut gone = false;
        for (_, index) in left {
            gone |= r.disconnect(index, ended);
        }
        ((), gone)
    })
}

/// When an effect next starts or ends, on the host's clock: the loop's
/// wake for it.
pub fn due() -> Option<f64> {
    let r = registry().lock().unwrap_or_else(|e| e.into_inner());
    (r.claims.iter())
        .map(|c| if c.started { c.end } else { c.start })
        .chain(r.leaving.iter().map(|(at, _)| *at))
        .reduce(f64::min)
}

/// A package instance's handle on the registry, as its context gives it:
/// its claims are its own, and end with the instance.
#[derive(Clone, Copy, Debug)]
pub struct Pads {
    client: u64,
}

impl Pads {
    pub(crate) fn new() -> Pads {
        Pads { client: client() }
    }
    pub(crate) fn client(&self) -> u64 {
        self.client
    }
    /// The connected pads by index, and the one in use.
    pub fn list(&self) -> (Vec<Pad>, Option<u8>) {
        list()
    }
    /// `f` is told each change: a pad came or went, or another is in use.
    pub fn watch(&self, f: impl Fn() + Send + Sync + 'static) {
        watch(self.client, f)
    }
    /// [`rumble`], as this instance's claim.
    pub fn rumble(
        &self,
        index: u8,
        strong: f32,
        weak: f32,
        delay: f64,
        ms: u32,
        done: Done,
    ) -> Result<(), &'static str> {
        rumble(self.client, index, strong, weak, delay, ms, done)
    }
    /// [`reset`].
    pub fn reset(&self, index: u8) -> Result<(), &'static str> {
        reset(index)
    }
}

/// What a test build's recorder heard since the last call, for the journal.
pub fn take_lines() -> Vec<String> {
    let mut r = registry().lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut r.lines)
}

impl Registry {
    /// End the effect on pad `index`'s device, if one plays, `preempted`.
    fn preempt(&mut self, index: u8, ended: &mut Vec<(Done, &'static str)>) {
        let mut stop = false;
        self.claims.retain_mut(|c| {
            if c.index != index {
                return true;
            }
            stop |= c.started;
            if let Some(done) = c.done.take() {
                ended.push((done, "preempted"));
            }
            false
        });
        if stop {
            if let Some(Some(slot)) = self.slots.get_mut(index as usize) {
                if let Some(out) = slot.output.as_mut() {
                    out.stop();
                }
            }
        }
    }

    /// Every claim on pad `index`'s device ends: it went, or another took the slot.
    fn end_device(&mut self, index: u8, ended: &mut Vec<(Done, &'static str)>) {
        let connection =
            (self.slots.get(index as usize).and_then(Option::as_ref)).map(|s| s.connection);
        let (gone, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.claims)
            .into_iter()
            .partition(|c| Some(c.connection) == connection);
        self.claims = keep;
        for mut c in gone {
            if let Some(done) = c.done.take() {
                ended.push((done, "preempted"));
            }
        }
    }

    /// Start the claims due, and complete those past their span.
    /// Pad `index` is gone, with every claim on it; whether it was there.
    fn disconnect(&mut self, index: u8, ended: &mut Vec<(Done, &'static str)>) -> bool {
        let gone = self.slots.get(index as usize).is_some_and(Option::is_some);
        self.end_device(index, ended);
        if let Some(slot) = self.slots.get_mut(index as usize) {
            *slot = None;
        }
        if self.current == Some(index) {
            self.current = None;
        }
        gone
    }

    fn run(&mut self, ended: &mut Vec<(Done, &'static str)>) {
        let now = self.now;
        for c in self
            .claims
            .iter_mut()
            .filter(|c| !c.started && c.start <= now)
        {
            c.started = true;
            if let Some(Some(slot)) = self.slots.get_mut(c.index as usize) {
                if let Some(out) = slot.output.as_mut() {
                    let remaining = (c.end - now).max(0.) as u32;
                    out.rumble(c.strong, c.weak, remaining.min(c.ms));
                }
            }
        }
        let (done, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.claims)
            .into_iter()
            .partition(|c| c.started && c.end <= now);
        self.claims = keep;
        for mut c in done {
            // XInput's motors run until told: its span ends here.
            if let Some(Some(slot)) = self.slots.get_mut(c.index as usize) {
                if let Some(out) = slot.output.as_mut() {
                    out.stop();
                }
            }
            if let Some(done) = c.done.take() {
                ended.push((done, "complete"));
            }
        }
    }
}

/// A test build's motors: every request journaled.
struct Recorder {
    index: u8,
}

impl Output for Recorder {
    fn rumble(&mut self, strong: f32, weak: f32, ms: u32) -> bool {
        let line = format!("pad {}: rumble {strong} {weak} for {ms} ms", self.index);
        // Called under the lock: the lines are the registry's own.
        LINES.with(|l| l.borrow_mut().push(line));
        true
    }
    fn stop(&mut self) {
        let line = format!("pad {}: motors stopped", self.index);
        LINES.with(|l| l.borrow_mut().push(line));
    }
}

thread_local!(static LINES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) });

/// The recorder's lines on this thread since the last call.
pub fn take_recorded() -> Vec<String> {
    let mut lines = take_lines();
    lines.extend(LINES.with(|l| std::mem::take(&mut *l.borrow_mut())));
    lines
}

/// `EXACT_AGENT_PADS`'s pads, connected (module doc).
fn inject_into(r: &Mutex<Registry>, json: &str) {
    let Ok(serde_json::Value::Array(pads)) = serde_json::from_str::<serde_json::Value>(json) else {
        r.lock()
            .unwrap_or_else(|e| e.into_inner())
            .lines
            .push(format!("EXACT_AGENT_PADS: not a JSON list: {json}"));
        return;
    };
    let mut r = r.lock().unwrap_or_else(|e| e.into_inner());
    for pad in pads {
        let index = pad["index"].as_u64().unwrap_or(0).min(15) as u8;
        let hex = |k: &str| {
            (pad[k].as_str())
                .and_then(|v| u16::from_str_radix(v, 16).ok())
                .or_else(|| pad[k].as_u64().map(|v| v as u16))
                .unwrap_or(0)
        };
        let identity = Identity {
            id: pad["id"].as_str().unwrap_or("Test pad").to_owned(),
            vendor: hex("vendor"),
            product: hex("product"),
            driver: pad["driver"].as_str().map(str::to_owned),
            user_index: None,
        };
        let i = index as usize;
        if r.slots.len() <= i {
            r.slots.resize_with(i + 1, || None);
        }
        r.connections += 1;
        let mut reading = Reading::default();
        for b in (pad["pressed"].as_array().into_iter().flatten()).filter_map(|b| b.as_u64()) {
            if let Some(v) = reading.buttons.get_mut(b as usize) {
                *v = 1.;
            }
        }
        if reading.buttons.iter().any(|b| *b >= DOWN) {
            r.current = Some(index);
        }
        let connection = r.connections;
        if let Some(ms) = pad["leaves"].as_f64() {
            let at = r.now + ms;
            r.leaving.push((at, index));
        }
        r.slots[i] = Some(Slot {
            identity,
            reading,
            connection,
            output: Some(Box::new(Recorder { index })),
        });
        r.lines
            .push(format!("pad {index}: a test build's pad connected"));
    }
}

#[cfg(test)]
#[path = "pads_tests.rs"]
mod tests;
