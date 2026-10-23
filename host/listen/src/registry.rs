//! D2 within one process: listeners on one resolved path follow the rule
//! between processes. Without `takeover` a second one waits until the
//! holder lets go; with it the newest takes the address, and the one it
//! displaced keeps its connections and takes the address back when the
//! newer lets go while it is still held.
use super::server::{Poke, Shared};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// A listener's place on its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// It holds the address (or is binding it).
    Holder,
    /// A newer listener took the address over.
    Displaced,
    /// Another listener holds the address; this one has not bound.
    Waiting,
}

struct Entry {
    path: PathBuf,
    id: u64,
    role: Role,
    shared: Arc<Shared>,
}

static ENTRIES: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// What a listener joining its path does.
pub(crate) enum Joined {
    /// Bind.
    Bind,
    /// Wait to be poked ([`Poke::Bind`]).
    Wait,
}

/// Join `path` as listener `id`.
pub(crate) fn join(path: &Path, id: u64, takeover: bool, shared: Arc<Shared>) -> Joined {
    let mut entries = ENTRIES.lock().unwrap();
    let holder = (entries.iter_mut()).find(|e| e.path == path && e.role == Role::Holder);
    let (role, joined) = match holder {
        None => (Role::Holder, Joined::Bind),
        Some(holder) if takeover => {
            holder.role = Role::Displaced;
            holder.shared.poke(Poke::Displaced);
            (Role::Holder, Joined::Bind)
        }
        Some(_) => (Role::Waiting, Joined::Wait),
    };
    entries.push(Entry {
        path: path.to_path_buf(),
        id,
        role,
        shared,
    });
    joined
}

/// Listener `id`'s role, while it has joined.
pub(crate) fn role(id: u64) -> Option<Role> {
    let entries = ENTRIES.lock().unwrap();
    entries.iter().find(|e| e.id == id).map(|e| e.role)
}

/// Listener `id` leaves its path. When it held the address, the newest
/// listener it displaced takes it back, or else the first waiting one
/// binds. The holder removes its socket file before it leaves, so the one
/// poked finds the path free.
pub(crate) fn leave(id: u64) {
    let mut entries = ENTRIES.lock().unwrap();
    let Some(at) = entries.iter().position(|e| e.id == id) else {
        return;
    };
    let left = entries.remove(at);
    if left.role != Role::Holder {
        return;
    }
    let on_path = |e: &&mut Entry| e.path == left.path;
    let next = match (entries.iter_mut().filter(on_path)).rfind(|e| e.role == Role::Displaced) {
        Some(displaced) => Some(displaced),
        None => (entries.iter_mut().filter(on_path)).find(|e| e.role == Role::Waiting),
    };
    if let Some(next) = next {
        next.role = Role::Holder;
        next.shared.poke(Poke::Bind);
    }
}
