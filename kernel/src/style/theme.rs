//! An app's `theme` (LLP 1095 D4): its own colour names, and the roles it
//! overrides, resolved where a colour is parsed. A plan's entries are bound
//! while its styles are parsed, as its colour profiles are
//! (`profiled::declarations`): the compiler binds them to check a row, the
//! runner to set one, so `color="brand"` and a string from data naming
//! `brand` read the same colour. A name is read as a whole component (a row,
//! a shadow's colour, a gradient stop, a palette entry), never inside a
//! `light-dark()` half or another function, as LLP 1095 D1 says; names
//! inside expressions are RFC 1121's synthetic tokens, built by the crate.

use std::cell::RefCell;
use std::sync::Arc;

/// A plan's theme as a parse reads it: each entry's name (a role's spelling
/// for an override) and the colour text it stands for, which names no entry.
/// Built once per plan; binding one is a pointer swap.
pub type Table = Arc<[(Box<str>, Box<str>)]>;

thread_local! {
    static ENTRIES: RefCell<Option<Table>> = const { RefCell::new(None) };
    /// Inside an entry's own parse: what it stands for names no entry.
    static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A parse scope's theme, restored on return or unwinding. The scope is
/// the thread's: every style of a plan is parsed on the thread that bound
/// it (a parse moved to a worker takes its table with it).
pub struct ThemeScope {
    previous: Option<Table>,
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// A table from `(name, colour text)` pairs.
pub fn table<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str)>) -> Table {
    entries
        .into_iter()
        .map(|(n, v)| (n.into(), v.into()))
        .collect()
}

/// Bind a plan's table while its styles are parsed.
pub fn bind(table: &Table) -> ThemeScope {
    ThemeScope {
        previous: ENTRIES.with(|e| e.replace(Some(table.clone()))),
        thread: std::marker::PhantomData,
    }
}

/// [`bind`] a table built from `entries`, for a caller that binds once.
pub fn declarations<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str)>) -> ThemeScope {
    bind(&table(entries))
}

impl Drop for ThemeScope {
    fn drop(&mut self) {
        ENTRIES.with(|e| {
            e.replace(self.previous.take());
        });
    }
}

/// `parse` of the colour text a theme name stands for, in the bound theme;
/// `None` when `text` names no entry. A role's spelling matches as roles do,
/// without case. The entry's own text is parsed with the theme unbound, so a
/// plan cannot make a name stand for itself.
pub(crate) fn resolved<T>(text: &str, parse: impl FnOnce(&str) -> T) -> Option<T> {
    let text = text.trim();
    if INSIDE.with(|i| i.get()) || text.is_empty() || text.contains(['(', ' ', '#']) {
        return None;
    }
    let table = ENTRIES.with(|e| e.borrow().clone())?;
    let value = &table
        .iter()
        .find(|(name, _)| {
            **name == *text
                || (name.starts_with('-') || name.starts_with(char::is_uppercase))
                    && name.eq_ignore_ascii_case(text)
        })?
        .1;
    INSIDE.with(|i| i.set(true));
    struct Out;
    impl Drop for Out {
        fn drop(&mut self) {
            INSIDE.with(|i| i.set(false));
        }
    }
    let _out = Out;
    Some(parse(value))
}
