//! Localized texts (LLP 1060): the tables `t(...)` reads, the chain that
//! picks a table for the viewer, and the one placeholder grammar the
//! compiler checks and the runner fills.

use crate::{LocalesRow, Plan, PlanError, TypeKind};

/// The `{name}` placeholders in `text`, in order. A name is what a Contract
/// named argument can spell; any other brace is literal text, so prose
/// needs no escapes.
pub fn placeholders(text: &str) -> impl Iterator<Item = &str> {
    pieces(text).filter_map(|piece| match piece {
        Piece::Name(name) => Some(name),
        Piece::Text(_) => None,
    })
}

/// `text` with each placeholder replaced by `value(name)`. A name without a
/// value stays as written: the compiler refuses that for the base table
/// and for every translation, so it only happens in a plan built by hand.
/// Refuses an expansion longer than `limit` bytes before allocating it.
pub fn fill<'v>(
    text: &str,
    value: impl Fn(&str) -> Option<&'v str>,
    limit: usize,
) -> Option<String> {
    let mut resolved = Vec::new();
    let mut len = 0usize;
    for piece in pieces(text) {
        let piece = match piece {
            Piece::Name(name) => value(name).map_or(Piece::Name(name), Piece::Text),
            piece => piece,
        };
        let bytes = match piece {
            Piece::Text(t) => t.len(),
            Piece::Name(name) => name.len().checked_add(2)?,
        };
        len = len.checked_add(bytes).filter(|len| *len <= limit)?;
        resolved.push(piece);
    }
    let mut out = String::with_capacity(len);
    for piece in resolved {
        match piece {
            Piece::Text(t) => out.push_str(t),
            Piece::Name(name) => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
    }
    Some(out)
}

enum Piece<'a> {
    Text(&'a str),
    Name(&'a str),
}

fn pieces(text: &str) -> impl Iterator<Item = Piece<'_>> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut from = 0;
        while let Some(open) = rest[from..].find('{').map(|i| from + i) {
            let inner = &rest[open + 1..];
            let name_len = inner
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .unwrap_or(inner.len());
            let name = &inner[..name_len];
            let is_name = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && inner[name_len..].starts_with('}');
            if !is_name {
                from = open + 1;
                continue;
            }
            if open > 0 {
                let text = &rest[..open];
                rest = &rest[open..];
                return Some(Piece::Text(text));
            }
            rest = &inner[name_len + 1..];
            return Some(Piece::Name(name));
        }
        let text = rest;
        rest = "";
        Some(Piece::Text(text))
    })
}

impl Plan {
    /// The table the viewer's `requested` locale (BCP 47) reads: the
    /// longest subtag prefix a table is named for, compared without case
    /// (RFC 4647 lookup: `zh-Hant-TW`, `zh-Hant`, `zh`), else the base.
    /// `None` when the plan has no strings.
    pub fn resolve_locale(&self, requested: &str) -> Option<&str> {
        let base = self.locales.first()?;
        let mut tag = requested;
        loop {
            if let Some(row) = self
                .locales
                .iter()
                .find(|row| self.str(row.name).eq_ignore_ascii_case(tag))
            {
                return Some(self.str(row.name));
            }
            match tag.rfind('-') {
                Some(end) => tag = &tag[..end],
                None => return Some(self.str(base.name)),
            }
        }
    }

    /// `key`'s text in the table named `locale`, or in the base when that
    /// table lacks it (a translation that is behind is still readable).
    pub fn localized(&self, locale: &str, key: &str) -> Option<&str> {
        let find = |row: &LocalesRow| {
            let texts = &self.texts[row.texts.start as usize..][..row.texts.len as usize];
            texts
                .binary_search_by(|t| self.str(t.key).cmp(key))
                .ok()
                .map(|i| self.str(texts[i].text))
        };
        self.locales
            .iter()
            .find(|row| self.str(row.name) == locale)
            .and_then(find)
            .or_else(|| find(self.locales.first()?))
    }

    /// Strings and their slot come together; each table is sorted by key
    /// (lookup is a binary search) and named once.
    pub(crate) fn validate_texts(&self) -> Result<(), PlanError> {
        let bad = |table, row: usize, field| PlanError::BadReference {
            table,
            row: row as u32,
            field,
        };
        match self.locale {
            Some(id) => {
                let slot = self.slot(id);
                if self.type_(slot.ty).kind != TypeKind::String
                    || slot.owner.is_some()
                    || self.locales.is_empty()
                {
                    return Err(bad("header", 0, "locale"));
                }
            }
            None if !self.locales.is_empty() => return Err(bad("locales", 0, "name")),
            None => {}
        }
        for (i, row) in self.locales.iter().enumerate() {
            let name = self.str(row.name);
            if name.is_empty()
                || self.locales[..i]
                    .iter()
                    .any(|other| self.str(other.name).eq_ignore_ascii_case(name))
            {
                return Err(bad("locales", i, "name"));
            }
            let texts = &self.texts[row.texts.start as usize..][..row.texts.len as usize];
            if texts
                .windows(2)
                .any(|pair| self.str(pair[0].key) >= self.str(pair[1].key))
            {
                return Err(bad("locales", i, "texts"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_names_in_braces_and_other_braces_are_text() {
        let text = "Hi {name}, {} {1x} {a b} {count}{_} {{x}}";
        assert_eq!(
            placeholders(text).collect::<Vec<_>>(),
            ["name", "count", "_", "x"]
        );
        let filled = fill(
            text,
            |n| match n {
                "name" => Some("Ada"),
                "count" => Some("3"),
                "x" => Some("X"),
                _ => None,
            },
            100,
        )
        .unwrap();
        assert_eq!(filled, "Hi Ada, {} {1x} {a b} 3{_} {X}");
    }

    #[test]
    fn expansion_counts_bytes_literals_and_unfilled_names_before_allocating() {
        assert_eq!(fill("{x}{x}", |_| Some("é"), 4).as_deref(), Some("éé"));
        assert_eq!(fill("{x}{x}", |_| Some("é"), 3), None);
        assert_eq!(fill("!{x}", |_| Some("abc"), 3), None);
        assert_eq!(fill("{missing}", |_| None, 8), None);
        assert_eq!(fill("{missing}", |_| None, 9).as_deref(), Some("{missing}"));
        assert_eq!(fill("literal", |_| None, 6), None);
        assert_eq!(fill("{x}", |_| Some(""), 0).as_deref(), Some(""));
    }
}
