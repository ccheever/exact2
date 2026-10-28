//! @exact/device on the Linux and Windows hosts (LLP 1133.031 D1–D5, D14),
//! composed by the desktop package builder (LLP 1133.022 D10 item 5).
//!
//! - Linux: the battery from UPower's display device, else the system
//!   battery in sysfs; `type` from the link of the route rtnetlink names for
//!   a global destination; low power from the Power Profile Monitor portal,
//!   else power-profiles-daemon. `saveData` is false: no desktop has such a
//!   setting.
//! - Windows: the battery and Energy Saver from the power management
//!   functions, `type` from the internet connection profile.
//! - Neither has a thermal state: `thermalState` is none.
//! - Elsewhere (the Linux host run headless on a Mac) the battery answers
//!   the specification's no-battery values and `type` `unknown`.
//!
//! One watcher per source per process, shared by every session's instance
//! (D5 item 1): the first read starts it, and it stops when no session reads
//! the source. A source announces to its readers only when an answered
//! field changes; the foreground reads every watched source again.

use exact_linux::package::{json, Context, Json, Notifier, Package, Reply};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod readers;
#[cfg(target_os = "linux")]
mod system;
#[cfg(windows)]
#[path = "windows.rs"]
mod win;

const SOURCES: [&str; 3] = ["getBattery", "connection", "powerState"];

/// What ends a watcher, run when its source has no reader left.
struct Watcher(Vec<Box<dyn FnOnce() + Send>>);

impl Drop for Watcher {
    fn drop(&mut self) {
        for stop in self.0.drain(..) {
            stop();
        }
    }
}

/// One source: its readers by session, its last answer, the reads waiting
/// for the first, and its watcher, once its first reader has started one.
#[derive(Default)]
struct Source {
    readers: HashMap<String, Notifier>,
    last: Option<Json>,
    waiting: Vec<Reply>,
    started: bool,
    watcher: Option<Watcher>,
}

static HUB: Mutex<Option<HashMap<&'static str, Source>>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut HashMap<&'static str, Source>) -> R) -> R {
    let mut hub = HUB.lock().unwrap_or_else(|e| e.into_inner());
    f(hub.get_or_insert_with(HashMap::new))
}

/// A new answer for `name`, from any thread: the reads waiting for one are
/// answered, and the readers told when it differs from the last.
fn heard(name: &'static str, answer: Json) {
    let (replies, tell) = with(|hub| {
        let Some(source) = hub.get_mut(name) else {
            return (Vec::new(), Vec::new());
        };
        let changed = source.last.as_ref().is_some_and(|last| *last != answer);
        source.last = Some(answer.clone());
        let tell: Vec<Notifier> = if changed {
            source.readers.values().cloned().collect()
        } else {
            Vec::new()
        };
        (std::mem::take(&mut source.waiting), tell)
    });
    for reply in replies {
        reply.send(answer.clone());
    }
    for notifier in tell {
        notifier.changed(&format!("device.{name}"));
    }
}

/// Read `name` again on a thread of its own, since a reading may wait on a
/// bus, and hear it.
fn refresh(name: &'static str) {
    std::thread::spawn(move || heard(name, reading(name)));
}

/// `session` has no reader of `name` left (`None`: of any source). A source
/// no session reads is dropped, which stops its watcher.
fn leave(name: Option<&str>, session: &str) {
    let gone: Vec<Source> = with(|hub| {
        let names: Vec<&'static str> = hub.keys().copied().filter(|n| name.is_none_or(|m| m == *n)).collect();
        let mut gone = Vec::new();
        for n in names {
            let Some(source) = hub.get_mut(n) else { continue };
            source.readers.remove(session);
            if source.readers.is_empty() {
                gone.extend(hub.remove(n));
            }
        }
        gone
    });
    for source in gone {
        for reply in source.waiting {
            reply.error("AbortError", None);
        }
    }
}

/// The answer now for `name`.
fn reading(name: &str) -> Json {
    match name {
        "getBattery" => {
            #[cfg(target_os = "linux")]
            let battery = system::battery();
            #[cfg(windows)]
            let battery = win::battery();
            #[cfg(not(any(target_os = "linux", windows)))]
            let battery = readers::NO_BATTERY;
            json!({ "level": battery.level, "charging": battery.charging })
        }
        "connection" => {
            #[cfg(target_os = "linux")]
            let answer = json!({
                "type": system::network_type(),
                "saveData": false,
                "linux": system::network_words()
                    .map(|(metered, connectivity)| json!({ "metered": metered, "connectivity": connectivity })),
            });
            #[cfg(windows)]
            let answer = {
                let (kind, words) = win::network();
                json!({ "type": kind, "saveData": false, "windows": words })
            };
            #[cfg(not(any(target_os = "linux", windows)))]
            let answer = json!({ "type": "unknown", "saveData": false });
            answer
        }
        _ => {
            #[cfg(target_os = "linux")]
            let low = matches!(
                system::low_power(),
                system::LowPower::Portal(true) | system::LowPower::Profiles(_, true)
            );
            #[cfg(windows)]
            let low = win::low_power();
            #[cfg(not(any(target_os = "linux", windows)))]
            let low = false;
            json!({ "lowPowerMode": low, "thermalState": null })
        }
    }
}

/// Start watching `name`: what calls [`refresh`] at each change.
fn watch(name: &'static str) -> Watcher {
    let mut stops: Vec<Box<dyn FnOnce() + Send>> = Vec::new();
    #[cfg(target_os = "linux")]
    linux_watch(name, &mut stops);
    #[cfg(windows)]
    match name {
        "connection" => {
            if let Some(token) = win::watch_network(Box::new(move || refresh(name))) {
                stops.push(Box::new(move || win::unwatch_network(token)));
            }
        }
        _ => {
            let registration = win::watch_power(name == "getBattery", Box::new(move || refresh(name)));
            stops.push(Box::new(move || drop(registration)));
        }
    }
    let _ = name;
    Watcher(stops)
}

/// A thread hearing `bus`'s signals for `rule`, each a refresh of `name`,
/// until the watcher shuts its socket.
#[cfg(target_os = "linux")]
fn hear(name: &'static str, mut bus: system::Bus, rule: &str, stops: &mut Vec<Box<dyn FnOnce() + Send>>) -> bool {
    let Some(closer) = bus.closer() else { return false };
    if !bus.subscribe(rule) {
        return false;
    }
    bus.wait_forever();
    std::thread::spawn(move || {
        while bus.signal().is_some() {
            refresh(name);
        }
    });
    stops.push(Box::new(move || {
        let _ = closer.shutdown(std::net::Shutdown::Both);
    }));
    true
}

#[cfg(target_os = "linux")]
fn linux_watch(name: &'static str, stops: &mut Vec<Box<dyn FnOnce() + Send>>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let changed = |path: &str, interface: &str| {
        format!("type='signal',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='{path}',arg0='{interface}'")
    };
    match name {
        "getBattery" => {
            if let Some(mut bus) = system::Bus::system() {
                let rule = changed(system::DISPLAY_DEVICE, "org.freedesktop.UPower.Device");
                if system::upower(&mut bus).is_some() && hear(name, bus, &rule, stops) {
                    return;
                }
            }
            // Without UPower the supply files are read once a minute while a reader exists (D5).
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || {
                while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
                    rx.recv_timeout(std::time::Duration::from_secs(60))
                {
                    refresh(name);
                }
            });
            stops.push(Box::new(move || drop(tx)));
        }
        "connection" => {
            let stop = Arc::new(AtomicBool::new(false));
            if let Some(socket) = system::Netlink::open(system::ROUTE_GROUPS, 2) {
                let stopped = stop.clone();
                std::thread::spawn(move || {
                    let mut buf = vec![0u8; 16384];
                    while !stopped.load(Ordering::Relaxed) {
                        if socket.read(&mut buf).is_some() {
                            refresh(name);
                        }
                    }
                });
            }
            stops.push(Box::new(move || stop.store(true, Ordering::Relaxed)));
            if let Some(bus) = system::Bus::session() {
                hear(
                    name,
                    bus,
                    "type='signal',interface='org.freedesktop.portal.NetworkMonitor',member='changed'",
                    stops,
                );
            }
        }
        _ => match system::low_power() {
            system::LowPower::Portal(_) => {
                if let Some(bus) = system::Bus::session() {
                    hear(name, bus, &changed(system::PORTAL_PATH, system::POWER_PORTAL), stops);
                }
            }
            system::LowPower::Profiles(i, _) => {
                let (bus_name, path) = system::PROFILES[i];
                if let Some(bus) = system::Bus::system() {
                    hear(name, bus, &changed(path, bus_name), stops);
                }
            }
            system::LowPower::Neither => {}
        },
    }
}

/// The session's instance: it reads through the process's sources.
pub struct Device {
    context: Context,
}

impl Package for Device {
    fn new(context: Context) -> Self {
        Device { context }
    }

    fn later(&mut self, name: &str, _args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let Some(name) = SOURCES.iter().copied().find(|s| *s == name) else {
            return reply.error("NotSupportedError", None);
        };
        let session = self.context.session().to_owned();
        let notifier = self.context.notifier();
        // One lock decides: the last answer, or a wait for the next, which
        // the first reader of a source also starts.
        let mut waiting = Some(reply);
        let (answer, start) = with(|hub| {
            let source = hub.entry(name).or_default();
            source.readers.insert(session, notifier);
            if let Some(last) = &source.last {
                return (Some(last.clone()), false);
            }
            source.waiting.extend(waiting.take());
            (None, !std::mem::replace(&mut source.started, true))
        });
        if let (Some(answer), Some(reply)) = (answer, waiting) {
            return reply.send(answer);
        }
        if start {
            let watcher = watch(name);
            // A source left meanwhile is gone, and its watcher with it.
            with(|hub| hub.get_mut(name).map(|source| source.watcher = Some(watcher)));
            refresh(name);
        }
    }

    fn release(&mut self, name: &str, _args: Option<&Json>) {
        self.context.log(format!("release device.{name}"));
        leave(Some(name), self.context.session());
    }

    fn foreground(&mut self, shown: bool) {
        if !shown {
            return;
        }
        let watched: Vec<&'static str> = with(|hub| hub.keys().copied().collect());
        for name in watched {
            refresh(name);
        }
    }

    fn destroy(&mut self) {
        leave(None, self.context.session());
    }
}
