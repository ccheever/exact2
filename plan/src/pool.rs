//! The plan's interned strings. A decoded plan reads them where its bytes
//! hold them: a plan linked into the binary lends its static bytes, any
//! other keeps one copy of the pool; a plan a compiler builds, or a string
//! interned after the decode, is its own `String`. A `Vec<String>` cost a
//! euxy boot 1,331 allocations and some 70 KB for its life (2026-10-19).

use crate::{bytes::Reader, PlanError};
use std::borrow::Cow;

/// A plan's strings by id: the pool its bytes decoded to, then those interned since.
#[derive(Clone, Default)]
pub struct Strings {
    /// The encoded records, `[len: u32 LE][UTF-8 bytes]`, each validated
    /// when decoded and never written after.
    pool: Cow<'static, [u8]>,
    /// Each pooled string's text, as an offset into `pool`.
    at: Vec<u32>,
    /// Strings interned after the decode; their ids follow the pool's.
    extra: Vec<String>,
}

impl Strings {
    /// Decode the pool at `r`, which reads `whole`; `lent` is `whole` when it
    /// lives as long as the program, so the pool stays in it.
    pub(crate) fn decode(
        r: &mut Reader<'_>,
        whole: &[u8],
        lent: Option<&'static [u8]>,
    ) -> Result<Self, PlanError> {
        let n = r.count()?;
        let start = whole.len() - r.remaining();
        let mut at = Vec::with_capacity(r.reserve(n, 4));
        for _ in 0..n {
            let text = r.str()?;
            let end = whole.len() - r.remaining();
            at.push((end - text.len() - start) as u32);
        }
        let end = whole.len() - r.remaining();
        let pool = match lent {
            Some(lent) => Cow::Borrowed(&lent[start..end]),
            None => Cow::Owned(whole[start..end].to_vec()),
        };
        Ok(Strings {
            pool,
            at,
            extra: Vec::new(),
        })
    }

    /// How many strings have ids.
    pub fn len(&self) -> usize {
        self.at.len() + self.extra.len()
    }

    /// Whether none has.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The string with id `id`, if there is one.
    pub fn get(&self, id: usize) -> Option<&str> {
        match self.at.get(id) {
            Some(&at) => {
                let at = at as usize;
                let len = u32::from_le_bytes(self.pool[at - 4..at].try_into().ok()?) as usize;
                std::str::from_utf8(self.pool.get(at..at + len)?).ok()
            }
            None => self.extra.get(id - self.at.len()).map(String::as_str),
        }
    }

    /// Every string, by id.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> + DoubleEndedIterator + '_ {
        (0..self.len()).map(|id| &self[id])
    }

    /// Intern `s` with the next id, `len()` before the call.
    pub fn push(&mut self, s: impl Into<String>) {
        self.extra.push(s.into());
    }

    /// Intern each, with the ids from `len()` on.
    pub fn extend<I: IntoIterator<Item = String>>(&mut self, strings: I) {
        self.extra.extend(strings);
    }

    /// Replace the string with id `id`.
    pub fn set(&mut self, id: usize, s: impl Into<String>) {
        if id < self.at.len() {
            // A pooled string moves out of the pool: the ids after it stay.
            let pooled: Vec<String> = self.iter().map(str::to_owned).collect();
            self.pool = Cow::Borrowed(&[]);
            self.at = Vec::new();
            self.extra = pooled;
        }
        self.extra[id - self.at.len()] = s.into();
    }

    /// Which plan's strings these are, for a cache keyed by the plan: the
    /// address of the buffer that holds them.
    pub fn identity(&self) -> usize {
        if self.at.is_empty() {
            self.extra.as_ptr() as usize
        } else {
            self.pool.as_ptr() as usize
        }
    }
}

impl std::ops::Index<usize> for Strings {
    type Output = str;
    fn index(&self, id: usize) -> &str {
        self.get(id)
            .unwrap_or_else(|| panic!("string id {id} of {}", self.len()))
    }
}

impl<'a> IntoIterator for &'a Strings {
    type Item = &'a str;
    type IntoIter = Box<dyn Iterator<Item = &'a str> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl std::fmt::Debug for Strings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for Strings {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytes::Writer;

    fn encoded(strings: &[&str]) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(strings.len() as u32);
        for s in strings {
            w.string(s);
        }
        w.u32(7);
        w.into_vec()
    }

    #[test]
    fn decoded_strings_read_in_place_and_extend() {
        let bytes = encoded(&["", "a", "héllo", "b"]);
        let mut r = Reader::new(&bytes);
        let mut strings = Strings::decode(&mut r, &bytes, None).unwrap();
        assert_eq!(r.remaining(), 4, "the pool ends where its last string does");
        assert_eq!(strings.len(), 4);
        assert_eq!(strings.get(2), Some("héllo"));
        assert_eq!(&strings[0], "");
        assert_eq!(strings.get(4), None);
        strings.push("after");
        assert_eq!(strings.get(4), Some("after"));
        assert_eq!(
            strings.iter().collect::<Vec<_>>(),
            ["", "a", "héllo", "b", "after"]
        );
        assert_eq!(strings.iter().position(|s| s == "b"), Some(3));
        assert_eq!(
            format!("{strings:?}"),
            r#"["", "a", "héllo", "b", "after"]"#
        );
        strings.set(1, "A");
        assert_eq!(
            strings.iter().collect::<Vec<_>>(),
            ["", "A", "héllo", "b", "after"]
        );
    }

    #[test]
    fn a_static_plan_lends_its_bytes() {
        let bytes: &'static [u8] = Box::leak(encoded(&["x", "yz"]).into_boxed_slice());
        let mut r = Reader::new(bytes);
        let strings = Strings::decode(&mut r, bytes, Some(bytes)).unwrap();
        let text = strings.get(1).unwrap();
        let range = bytes.as_ptr_range();
        assert!(
            range.contains(&text.as_ptr()),
            "the text is read from the static bytes"
        );
        assert!(matches!(strings.pool, Cow::Borrowed(_)));
    }

    #[test]
    fn bad_utf8_is_refused_at_decode() {
        let mut w = Writer::default();
        w.u32(1);
        w.u32(2);
        w.bytes(&[0xff, 0xfe]);
        let bytes = w.into_vec();
        let mut r = Reader::new(&bytes);
        assert!(matches!(
            Strings::decode(&mut r, &bytes, None),
            Err(PlanError::BadUtf8)
        ));
    }

    #[test]
    fn equal_by_content_whatever_holds_them() {
        let bytes = encoded(&["p", "q"]);
        let decoded = Strings::decode(&mut Reader::new(&bytes), &bytes, None).unwrap();
        let mut built = Strings::default();
        built.extend(["p".to_string(), "q".to_string()]);
        assert_eq!(decoded, built);
        built.push("r");
        assert_ne!(decoded, built);
    }
}
