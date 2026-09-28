//! The two shared heap forms a [`Value`] holds: text ([`Str`]) and a list's
//! or a record's items ([`Items`]).
//!
//! Both are one thin pointer, the length kept in the allocation beside the
//! count, so a `Value` is 16 bytes: an 8-byte payload and its tag. A fat
//! `Rc<str>` or `Rc<[Value]>` carries its length in the pointer and makes
//! every value, and so every record field and list item, 24 bytes.
//!
//! The unsafe code is the dependencies' (`triomphe` for items, `arcstr` for
//! text: triomphe has no thin `str`); this crate keeps its ban. Both keep
//! one count and the length before the data, 16 bytes, where a thin `Rc`
//! (the `slice-dst` family) keeps a weak count too: 24, which malloc's
//! 16-byte quantum makes 32 on every record, giving back half the saving
//! (perf/value16: the heavy list's baked values 21.1 MB as `Rc<[Value]>`,
//! 16.1 thin here, 18.8 through a thin `Rc`). Their counts are atomic;
//! against the same two crates with plain counts, decode, boot, scroll,
//! live ticks and the VM's map/join measured the same.

use crate::Value;
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

/// Shared UTF-8 text: one thin pointer, cloned by count.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Str(arcstr::ArcStr);

impl Str {
    /// Whether two are one allocation (equal text, then, but not only).
    #[inline]
    pub fn ptr_eq(a: &Str, b: &Str) -> bool {
        arcstr::ArcStr::ptr_eq(&a.0, &b.0)
    }

    /// The text.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// How many hold this allocation.
    pub fn strong_count(this: &Str) -> usize {
        arcstr::ArcStr::strong_count(&this.0).unwrap_or(usize::MAX)
    }
}

impl exact_num::Piece for Str {
    fn push_to(&self, out: &mut String) {
        out.push_str(self);
    }
}

impl Deref for Str {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Str {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Str {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl From<&str> for Str {
    #[inline]
    fn from(s: &str) -> Str {
        Str(arcstr::ArcStr::from(s))
    }
}

impl From<String> for Str {
    fn from(s: String) -> Str {
        Str::from(s.as_str())
    }
}

impl PartialEq<str> for Str {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// Text longer than [`InlineStr::CAP`] bytes inside a [`Value`]: opaque
/// outside this crate, so text is read only through [`Value::as_str`]
/// (Charlie, 2026-09-28; LLP 1017.003 §"The value's text").
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HeapStr(pub(crate) Str);

impl fmt::Debug for HeapStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_str(), f)
    }
}

/// Text of up to [`InlineStr::CAP`] bytes held inside a [`Value`], no
/// allocation: the length and the bytes. Opaque outside this crate, as
/// [`HeapStr`] is. Safe code: the bytes are UTF-8 by construction, and
/// reading them checks it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InlineStr {
    len: u8,
    bytes: [u8; InlineStr::CAP],
}

impl InlineStr {
    /// The most bytes held inline: a `Value`'s 16 less its tag and this length.
    pub const CAP: usize = 14;

    /// `s` held inline, when it fits.
    #[inline]
    pub(crate) fn new(s: &str) -> Option<InlineStr> {
        let b = s.as_bytes();
        if b.len() > Self::CAP {
            return None;
        }
        let mut bytes = [0u8; Self::CAP];
        bytes[..b.len()].copy_from_slice(b);
        Some(InlineStr {
            len: b.len() as u8,
            bytes,
        })
    }

    /// The text.
    #[inline]
    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("inline text is UTF-8 by construction")
    }
}

impl fmt::Debug for InlineStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// A list's or a record's items, shared: one thin pointer, cloned by count.
#[derive(Clone)]
pub struct Items(triomphe::ThinArc<(), Value>);

impl Items {
    /// Whether two are one allocation.
    #[inline]
    pub fn ptr_eq(a: &Items, b: &Items) -> bool {
        a.0.ptr() == b.0.ptr()
    }

    /// The allocation's address: an identity, for hashing what `ptr_eq`
    /// compares.
    #[inline]
    pub fn addr(&self) -> usize {
        self.0.ptr() as usize
    }

    /// How many hold this allocation.
    pub fn strong_count(this: &Items) -> usize {
        triomphe::ThinArc::strong_count(&this.0)
    }

    /// The items.
    #[inline]
    pub fn as_slice(&self) -> &[Value] {
        &self.0.slice
    }
}

impl Str {
    /// The allocation's address: an identity, for hashing what `ptr_eq`
    /// compares.
    #[inline]
    pub fn addr(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

impl Deref for Items {
    type Target = [Value];
    #[inline]
    fn deref(&self) -> &[Value] {
        &self.0.slice
    }
}

impl AsRef<[Value]> for Items {
    #[inline]
    fn as_ref(&self) -> &[Value] {
        self
    }
}

impl Borrow<[Value]> for Items {
    #[inline]
    fn borrow(&self) -> &[Value] {
        self
    }
}

impl Default for Items {
    fn default() -> Items {
        Items::from(Vec::new())
    }
}

impl PartialEq for Items {
    #[inline]
    fn eq(&self, other: &Items) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl fmt::Debug for Items {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl From<Vec<Value>> for Items {
    #[inline]
    fn from(items: Vec<Value>) -> Items {
        Items(triomphe::ThinArc::from_header_and_iter(
            (),
            items.into_iter(),
        ))
    }
}

impl From<&[Value]> for Items {
    fn from(items: &[Value]) -> Items {
        Items(triomphe::ThinArc::from_header_and_iter(
            (),
            items.iter().cloned(),
        ))
    }
}

impl FromIterator<Value> for Items {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Items {
        Items::from(iter.into_iter().collect::<Vec<Value>>())
    }
}

impl<'a> IntoIterator for &'a Items {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}
