//! Each session's role overrides (LLP 1121 stage 5): its own table, bound
//! by a host around the work it does for it, or installed as its thread's.

use super::{reported_generation, role_of};
use crate::style::ColorValue;
use std::sync::atomic::{AtomicU64, Ordering};

/// A session's role overrides (LLP 1095 D4): the roles its design replaces,
/// and with what (an existing form), with the wide colours they hold kept
/// from the table's sweep while the session holds them. Each session has
/// its own (LLP 1121 stage 5): a runner builds one from the plan it boots.
#[derive(Default)]
pub struct Roles {
    pub(super) table: Vec<(u8, ColorValue)>,
    /// Held for its life, never read.
    #[allow(dead_code)]
    kept: crate::style::wide::Keep,
    pub(super) generation: u64,
}

/// Every [`Roles`] made, numbered: what a host compares to re-present.
static ROLES_MADE: AtomicU64 = AtomicU64::new(1);

impl Roles {
    /// A session's overrides from `next`: an unknown role, and a role
    /// replaced by itself, are left out.
    pub fn new(next: Vec<(u8, ColorValue)>) -> std::rc::Rc<Roles> {
        let table: Vec<(u8, ColorValue)> = next
            .into_iter()
            .filter(|(id, c)| {
                role_of(*id).is_some() && !matches!(c, ColorValue::Role(r) if r == id)
            })
            .collect();
        let kept = crate::style::wide::Keep::of(|m| {
            for (_, c) in &table {
                m.color(*c);
            }
        });
        std::rc::Rc::new(Roles {
            table,
            kept,
            generation: ROLES_MADE.fetch_add(1, Ordering::AcqRel),
        })
    }

    /// The overridden roles as set.
    pub fn overrides(&self) -> &[(u8, ColorValue)] {
        &self.table
    }

    /// These overrides with each of `changes` set in place (a live edit,
    /// LLP 1121 stage 5): a new table, so a host sees a new generation.
    pub fn with(&self, changes: &[(u8, ColorValue)]) -> std::rc::Rc<Roles> {
        let mut next = self.table.clone();
        for &(id, c) in changes {
            match next.iter_mut().find(|(r, _)| *r == id) {
                Some(entry) => entry.1 = c,
                None => next.push((id, c)),
            }
        }
        Roles::new(next)
    }
}

thread_local! {
    /// The session whose roles resolve now: bound by a host around what it
    /// does for one session ([`bind`]), else the one the thread installed
    /// last ([`install`]) that still lives, then the one before.
    static BOUND: std::cell::RefCell<Option<std::rc::Rc<Roles>>> = const { std::cell::RefCell::new(None) };
    static INSTALLED: std::cell::RefCell<Vec<std::rc::Weak<Roles>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The roles that resolve now ([`BOUND`]); none when no session is.
///
/// Unbound while two sessions live on the thread, which one is meant is
/// a guess: a host path that forgot to bind. Said once per call site, and
/// a debug build asserts.
#[track_caller]
pub fn bound() -> std::rc::Rc<Roles> {
    if let Some(roles) = BOUND.with(|b| b.borrow().clone()) {
        return roles;
    }
    let (live, newest) = INSTALLED.with(|i| {
        let i = i.borrow();
        let live = i.iter().filter(|w| w.strong_count() > 0).count();
        (live, i.iter().rev().find_map(std::rc::Weak::upgrade))
    });
    if live > 1 {
        unbound(std::panic::Location::caller());
    }
    newest.unwrap_or_default()
}

/// The thread's newest installed roles that still live, bound or not, with
/// no guess said: a host or test asking which session is installed.
pub fn installed() -> Option<std::rc::Rc<Roles>> {
    INSTALLED.with(|i| i.borrow().iter().rev().find_map(std::rc::Weak::upgrade))
}

/// A resolve with no session bound while several live on its thread.
fn unbound(at: &'static std::panic::Location<'static>) {
    static SEEN: std::sync::Mutex<Vec<(&str, u32)>> = std::sync::Mutex::new(Vec::new());
    let first = SEEN.lock().is_ok_and(|mut seen| {
        let first = !seen.contains(&(at.file(), at.line()));
        if first {
            seen.push((at.file(), at.line()));
        }
        first
    });
    if first {
        eprintln!(
            "exact: a role resolved at {at} with no session bound while several live on this thread: the newest installed one's (LLP 1121 stage 5)"
        );
    }
    debug_assert!(
        false,
        "a role resolved at {at} with no session bound while several live on this thread"
    );
}

/// A host's scope for one session: its roles resolve until the scope ends,
/// the ones before after (a host driving several sessions on one thread
/// binds each around what it does for it).
pub struct RolesScope {
    previous: Option<std::rc::Rc<Roles>>,
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Bind `roles` for the scope.
pub fn bind(roles: &std::rc::Rc<Roles>) -> RolesScope {
    RolesScope {
        previous: BOUND.with(|b| b.replace(Some(roles.clone()))),
        thread: std::marker::PhantomData,
    }
}

impl Drop for RolesScope {
    fn drop(&mut self) {
        let _ = BOUND.try_with(|b| {
            b.replace(self.previous.take());
        });
    }
}

/// Make `roles` the thread's outside any [`bind`] (a build that emits one
/// plan), while its holder keeps it alive; the ones installed before resolve
/// again once it ends. A scope a host bound stays its own: a session that
/// boots inside one binds its roles itself.
pub fn install(roles: &std::rc::Rc<Roles>) {
    INSTALLED.with(|i| {
        let mut i = i.borrow_mut();
        i.retain(|w| w.strong_count() > 0 && !std::ptr::eq(w.as_ptr(), std::rc::Rc::as_ptr(roles)));
        i.push(std::rc::Rc::downgrade(roles));
    });
}

/// A session's roles, installed as its thread's once it has booted
/// ([`Installed::install`]), and no longer once it ends: a candidate its
/// host refused leaves the live session's resolving, not none, whatever
/// order candidates end in.
pub struct Installed {
    roles: std::rc::Rc<Roles>,
}

impl Installed {
    /// `roles`, not yet installed.
    pub fn new(roles: std::rc::Rc<Roles>) -> Installed {
        Installed { roles }
    }

    /// The roles.
    pub fn roles(&self) -> &std::rc::Rc<Roles> {
        &self.roles
    }

    /// Install them on this thread, over the ones before (a session that
    /// moved here installs again).
    pub fn install(&mut self) {
        install(&self.roles);
    }

    /// The session's roles now `next` (a live edit): where its old ones are
    /// installed or bound on this thread, these are.
    pub fn replace(&mut self, next: std::rc::Rc<Roles>) {
        let (old, new) = (
            std::rc::Rc::as_ptr(&self.roles),
            std::rc::Rc::downgrade(&next),
        );
        INSTALLED.with(|i| {
            for w in i
                .borrow_mut()
                .iter_mut()
                .filter(|w| std::ptr::eq(w.as_ptr(), old))
            {
                *w = new.clone();
            }
        });
        BOUND.with(|b| {
            if let Some(bound) = b
                .borrow_mut()
                .as_mut()
                .filter(|r| std::ptr::eq(std::rc::Rc::as_ptr(r), old))
            {
                *bound = next.clone();
            }
        });
        self.roles = next;
    }

    /// No longer this thread's: the ones installed before resolve again (a
    /// session that leaves the thread, or ends).
    pub fn uninstall(&mut self) {
        let roles = std::rc::Rc::as_ptr(&self.roles);
        // Nothing to take back on a thread that is ending (a session its
        // thread-locals drop).
        let _ = INSTALLED.try_with(|i| i.borrow_mut().retain(|w| !std::ptr::eq(w.as_ptr(), roles)));
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        self.uninstall();
    }
}

/// The overridden roles that resolve now, for a caller that puts them back.
#[track_caller]
pub fn overrides() -> Vec<(u8, ColorValue)> {
    bound().overrides().to_vec()
}

/// What a host compares to know it must re-present what it resolved from
/// roles: moves when the reported table changes, and differs for another
/// set of the session's overrides.
#[track_caller]
pub fn generation() -> u64 {
    reported_generation().wrapping_add(bound().generation.wrapping_mul(1 << 20))
}

/// Install `next` as the thread's overrides, held by the thread (tests and
/// one-plan tools); `true` when a different, non-empty set is replaced.
pub fn set_overrides(next: Vec<(u8, ColorValue)>) -> bool {
    thread_local! {
        static HELD: std::cell::RefCell<std::rc::Rc<Roles>> = std::cell::RefCell::new(std::rc::Rc::default());
    }
    let roles = Roles::new(next);
    let before = bound();
    let replaced = !before.table.is_empty() && before.table != roles.table;
    install(&roles);
    HELD.with(|h| *h.borrow_mut() = roles);
    replaced
}
