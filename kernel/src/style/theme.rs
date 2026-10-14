//! An app's `theme` (LLP 1095 D4): its own colour names, and the roles it
//! overrides, resolved where a colour is parsed. A plan's entries are bound
//! while its styles are parsed, as its colour profiles are
//! (`profiled::declarations`): the compiler binds them to check a row, the
//! runner to set one, so `color="brand"` and a string from data naming
//! `brand` read the same colour.

use std::cell::RefCell;

thread_local! {
    static ENTRIES: RefCell<Vec<(Box<str>, Box<str>)>> = const { RefCell::new(Vec::new()) };
    /// Inside an entry's own parse: what it stands for names no entry.
    static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A parse scope's theme, restored on return or unwinding.
pub struct ThemeScope {
    previous: Vec<(Box<str>, Box<str>)>,
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Bind a plan's theme while its styles are parsed: each entry's name (a
/// role's spellings for an override) and the colour text it stands for,
/// which names no entry.
pub fn declarations<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str)>) -> ThemeScope {
    let next = entries
        .into_iter()
        .map(|(n, v)| (n.into(), v.into()))
        .collect();
    ThemeScope {
        previous: ENTRIES.with(|e| e.replace(next)),
        thread: std::marker::PhantomData,
    }
}

impl Drop for ThemeScope {
    fn drop(&mut self) {
        ENTRIES.with(|e| {
            e.replace(std::mem::take(&mut self.previous));
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
    let value = ENTRIES.with(|e| {
        e.borrow()
            .iter()
            .find(|(name, _)| {
                **name == *text
                    || (name.starts_with('-') || name.starts_with(char::is_uppercase))
                        && name.eq_ignore_ascii_case(text)
            })
            .map(|(_, value)| value.clone())
    })?;
    INSIDE.with(|i| i.set(true));
    struct Out;
    impl Drop for Out {
        fn drop(&mut self) {
            INSIDE.with(|i| i.set(false));
        }
    }
    let _out = Out;
    Some(parse(&value))
}
