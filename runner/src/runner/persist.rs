//! Persisted state (LLP 1116 D5): `state … persist` kept across launches.
//!
//! The value lives in the store (LLP 1018) under `exact.state.<name>`, a
//! runner-owned name beside the kept answers, as JSON text: a number as
//! JSON's (non-finite ones as `"NaN"`, `"Infinity"`, `"-Infinity"`), a
//! string, a bool, an option as `null` or its value, a list as an array.
//! At boot a stored value that decodes to the slot's type replaces the
//! initializer's; anything else keeps it and is journaled. After every
//! commit that stands — the boot's too — the store holds each persisted
//! slot's value, so a host persists exactly the writes the store makes.

use super::*;
use exact_plan::{TypeKind, TypesId};

/// The most a persisted value's text may hold.
pub const PERSIST_LIMIT: usize = 16 * 1024;

/// The store's name for persisted state `name`.
pub(crate) fn state_name(name: &str) -> String {
    let mut key = String::from(Store::STATE);
    key.push_str(name);
    key
}

impl<D: DataSource> Runner<D> {
    /// Slot `i`'s boot value: the store's, when slot `i` is persisted and the
    /// stored text decodes to its type; else `v`, the initializer's.
    pub(super) fn restore_persisted(&mut self, i: usize, v: Value) -> Value {
        let row = &self.plan.slots[i];
        if !row.persist {
            return v;
        }
        let name = self.plan.str(row.name);
        let Some(text) = self.store.kept(&state_name(name)) else {
            return v;
        };
        let stored = (text.len() <= PERSIST_LIMIT)
            .then(|| decode(&self.plan, row.ty, text))
            .flatten()
            .filter(|stored| stored.conforms(&self.plan, row.ty));
        match stored {
            Some(stored) => stored,
            None => {
                let line = format!(
                    "persisted state {name}: the stored value does not fit {}: the initial value stands",
                    type_name(&self.plan, row.ty)
                );
                self.log(line);
                v
            }
        }
    }

    /// Keep each persisted slot's value in the store, where it differs from
    /// what the store holds: after a commit stands.
    pub(super) fn persist_slots(&mut self) {
        for i in 0..self.plan.slots.len() {
            if !self.plan.slots[i].persist {
                continue;
            }
            let mut text = String::new();
            encode(&self.slots[i], &mut text);
            let name = self.plan.str(self.plan.slots[i].name);
            let key = state_name(name);
            if self.store.kept(&key) == Some(text.as_str()) {
                continue;
            }
            if text.len() > PERSIST_LIMIT {
                let line = format!(
                    "persisted state {name}: {} bytes is over the {PERSIST_LIMIT} a persisted \
                     value may hold: not kept (the store keeps its last value)",
                    text.len()
                );
                self.log(line);
                continue;
            }
            self.store.keep(&key, &text);
        }
    }

    /// [`Runner::persist_slots`] at boot, journaled.
    pub(super) fn persist_boot(&mut self) {
        let since = self.store.writes().len();
        self.persist_slots();
        self.log_store_writes(since);
    }

    /// What the store keeps for persisted state `name`, decoded: `None` when
    /// `name` is not a persisted root slot, or nothing decodable is kept.
    pub fn persisted(&self, name: &str) -> Option<Value> {
        let row = self
            .plan
            .slots
            .iter()
            .find(|s| s.persist && self.plan.str(s.name) == name)?;
        decode(&self.plan, row.ty, self.store.kept(&state_name(name))?)
    }
}

/// A persisted value as the store keeps it.
pub fn encode(v: &Value, out: &mut String) {
    match v {
        Value::Number(n) if n.is_nan() => out.push_str("\"NaN\""),
        Value::Number(n) if n.is_infinite() => out.push_str(if *n > 0.0 {
            "\"Infinity\""
        } else {
            "\"-Infinity\""
        }),
        Value::Number(n) => exact_num::push_text!(out, "{}", exact_num::Shortest(*n)),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        v @ exact_plan::str_value!() => crate::agent::quote(v.text(), out),
        Value::Option(None) | Value::Unit => out.push_str("null"),
        Value::Option(Some(v)) => encode(v, out),
        Value::List(items) | Value::Record(items) => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                encode(v, out);
            }
            out.push(']');
        }
    }
}

/// `text` as a value of type `ty`: `None` when it is not one.
pub fn decode(plan: &Plan, ty: TypesId, text: &str) -> Option<Value> {
    let mut r = Reader {
        b: text.as_bytes(),
        at: 0,
    };
    let v = r.value(plan, ty, 0)?;
    r.space();
    (r.at == r.b.len()).then_some(v)
}

/// A type as the journal names it.
fn type_name(plan: &Plan, ty: TypesId) -> String {
    let t = plan.type_(ty);
    let elem = || t.elem.map(|e| type_name(plan, e)).unwrap_or_default();
    match t.kind {
        TypeKind::Number => "number".into(),
        TypeKind::String => "string".into(),
        TypeKind::Bool => "bool".into(),
        TypeKind::Option => format!("option<{}>", elem()),
        TypeKind::List => format!("list<{}>", elem()),
        _ => "value of its type".into(),
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self.b.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, word: &str) -> bool {
        self.space();
        let hit = self.b[self.at..].starts_with(word.as_bytes());
        if hit {
            self.at += word.len();
        }
        hit
    }

    fn value(&mut self, plan: &Plan, ty: TypesId, depth: usize) -> Option<Value> {
        if depth > 2 {
            return None;
        }
        let t = plan.type_(ty);
        match t.kind {
            TypeKind::Number => match self.string() {
                Some(s) => match s.as_str() {
                    "NaN" => Some(Value::Number(f64::NAN)),
                    "Infinity" => Some(Value::Number(f64::INFINITY)),
                    "-Infinity" => Some(Value::Number(f64::NEG_INFINITY)),
                    _ => None,
                },
                None => self.number().map(Value::Number),
            },
            TypeKind::String => self.string().map(|s| Value::str(&s)),
            TypeKind::Bool if self.eat("true") => Some(Value::Bool(true)),
            TypeKind::Bool if self.eat("false") => Some(Value::Bool(false)),
            TypeKind::Option if self.eat("null") => Some(Value::Option(None)),
            TypeKind::Option => {
                let v = self.value(plan, t.elem?, depth + 1)?;
                Some(Value::Option(Some(std::rc::Rc::new(v))))
            }
            TypeKind::List => {
                let elem = t.elem?;
                if !self.eat("[") {
                    return None;
                }
                let mut items = Vec::new();
                if !self.eat("]") {
                    loop {
                        items.push(self.value(plan, elem, depth + 1)?);
                        if self.eat("]") {
                            break;
                        }
                        if !self.eat(",") {
                            return None;
                        }
                    }
                }
                Some(Value::list(items))
            }
            _ => None,
        }
    }

    fn number(&mut self) -> Option<f64> {
        self.space();
        let start = self.at;
        while self
            .b
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.b[start..self.at]).ok()?;
        let n = exact_num::parse_f64(text).ok()?;
        // JSON's numbers only: no `inf` or `nan` spellings, no bare `.5`.
        (n.is_finite()
            && text
                .trim_start_matches('-')
                .starts_with(|c: char| c.is_ascii_digit()))
        .then_some(n)
    }

    /// A JSON string, its escapes decoded; the cursor stays put when the
    /// next token is not one.
    fn string(&mut self) -> Option<String> {
        self.space();
        if self.b.get(self.at) != Some(&b'"') {
            return None;
        }
        let rest = std::str::from_utf8(&self.b[self.at + 1..]).ok()?;
        let mut out = String::new();
        let mut chars = rest.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    self.at += 1 + i + 1;
                    return Some(out);
                }
                '\\' => match chars.next()?.1 {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let mut unit = hex4(&mut chars)?;
                        if (0xD800..0xDC00).contains(&unit) {
                            if chars.next()?.1 != '\\' || chars.next()?.1 != 'u' {
                                return None;
                            }
                            let low = hex4(&mut chars)?;
                            if !(0xDC00..0xE000).contains(&low) {
                                return None;
                            }
                            unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                        }
                        out.push(char::from_u32(unit)?);
                    }
                    c @ ('"' | '\\' | '/') => out.push(c),
                    _ => return None,
                },
                c if (c as u32) < 0x20 => return None,
                c => out.push(c),
            }
        }
        None
    }
}

/// Four hex digits of a `\u` escape.
fn hex4(chars: &mut std::str::CharIndices<'_>) -> Option<u32> {
    let h: String = chars.by_ref().take(4).map(|(_, c)| c).collect();
    (h.len() == 4).then(|| u32::from_str_radix(&h, 16).ok())?
}
