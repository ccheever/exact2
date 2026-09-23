//! What this binary knows about its own delivery (LLP 1030 D4, D7).
//!
//! Two facts are the binary's and never change while it runs — `L`, whether
//! an update store is linked at all, and `E`, which executors it links —
//! and the rest is what the store has to say: the stream this client
//! belongs to, the entry it is running, the entry it shipped with, whether
//! a newer one is staged, a sunset notice, and which of the app's sources
//! are interpreted rather than native.
//!
//! A client with no update store (`L = 0`) answers the embedded entry and
//! nothing staged, which is the honest statement that it cannot be told
//! anything — and that is exactly [`Delivery::default`], so a plan baked
//! with no host in sight compiles the right first frame.
//!
//! The app reads these through one resource, `exactDelivery`, which the
//! runner answers itself before the data seam; the agent reads the same
//! facts through `state.delivery` (LLP 1012's eight operations, unchanged).

use exact_plan::Value;

/// The source name a Contract app declares to read these facts:
/// `resource delivery = exactDelivery()`. LLP 1030 D7 spells the resource
/// `delivery` and its commands `delivery.check`/`delivery.activate`; a
/// Contract identifier holds no dot, so the source is `exactDelivery` and
/// the commands are `deliveryCheck` and `deliveryActivate`.
pub const SOURCE: &str = "exactDelivery";

/// Refuse a bake produced only for compatibility analysis (LLP 1030 D3a).
/// Its receipt remains readable by tooling, but no host may launch it.
/// Only the binary's immediate `embedded.analysis: true` marks this policy;
/// a nested publisher receipt or ordinary release metadata does not.
pub fn refuse_analysis(json: &str) -> Result<(), &'static str> {
    if member(json, "embedded").and_then(|embedded| member(embedded, "analysis")) == Some("true") {
        Err("this is a compatibility analysis artifact; publish and rebake before launch")
    } else {
        Ok(())
    }
}

/// Every field the runner can fill, by the name a declared shape gives it.
/// A shape that names anything else is refused at bake (`bake-delivery-field`).
pub const FIELDS: [&str; 7] = [
    "stream",
    "seq",
    "embeddedSeq",
    "staged",
    "sunset",
    "interpreted",
    "compatibilityId",
];

/// The delivery facts, as the runner holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The cohort this binary belongs to (LLP 1030 D3a): bake's digest of
    /// everything a bundle may depend on and cannot replace. Empty until a
    /// host hands the runner its `compat.json`.
    pub compatibility_id: String,
    /// `L` (LLP 1030 D4): `'0'` — no update store is linked; `'A'` — the
    /// store, selection, signing, anti-rollback, crash fallback.
    pub store: char,
    /// `E` (LLP 1030 D4): the executors this binary links, sorted.
    pub executors: Vec<String>,
    /// The stream this client belongs to: `"embedded"` until a host knows
    /// better, else `"<channel>/<compatibilityId>"`.
    pub stream: String,
    /// The sequence running now, including the binary's baked sequence.
    pub seq: u64,
    /// The entry this binary shipped with.
    pub embedded_seq: u64,
    /// Whether a newer entry is downloaded and waiting for `deliveryActivate`.
    pub staged: bool,
    /// The sunset card, when the stream has one.
    pub sunset: Option<String>,
    /// Which of the app's data sources run on an interpreter rather than as
    /// native code (LLP 1029 D2), sorted.
    pub interpreted: Vec<String>,
}

impl Default for Delivery {
    /// The embedded answer: this binary, its own entry, nothing staged.
    fn default() -> Delivery {
        Delivery {
            compatibility_id: String::new(),
            store: 'A',
            executors: vec!["native".to_string()],
            stream: "embedded".to_string(),
            seq: 0,
            embedded_seq: 0,
            staged: false,
            sunset: None,
            interpreted: Vec::new(),
        }
    }
}

impl Delivery {
    /// One declared field's value, by the name the shape gives it; `None`
    /// when the runner does not know that name.
    pub fn field(&self, name: &str) -> Option<Value> {
        Some(match name {
            "stream" => Value::str(&self.stream),
            "seq" => Value::Number(self.seq as f64),
            "embeddedSeq" => Value::Number(self.embedded_seq as f64),
            "staged" => Value::Bool(self.staged),
            // A resource has no absence (LLP 1004 D3): no sunset is "".
            "sunset" => Value::str(self.sunset.as_deref().unwrap_or("")),
            "interpreted" => Value::list(self.interpreted.iter().map(|s| Value::str(s)).collect()),
            "compatibilityId" => Value::str(&self.compatibility_id),
            _ => return None,
        })
    }

    /// The binary's id, `store.L`, executor set, and embedded sequence from
    /// its archive's `compat.json`, as
    /// `exact_bake::Compat::to_json` writes them. Everything the store has to
    /// say is left as it was; a field the text does not carry is left as it
    /// was too, so a truncated or foreign file degrades to the embedded
    /// answer rather than refusing a boot.
    pub fn with_compat(&self, json: &str) -> Delivery {
        let mut out = self.clone();
        if let Some(id) = member(json, "id").and_then(|rest| string_at(rest).map(|(s, _)| s)) {
            out.compatibility_id = id;
        }
        if let Some(l) = member(json, "inputs")
            .and_then(|inputs| member(inputs, "store"))
            .and_then(|rest| member(rest, "L"))
            .and_then(|rest| string_at(rest).map(|(s, _)| s))
            .and_then(|s| s.chars().next())
        {
            out.store = l;
        }
        if let Some(executors) = member(json, "inputs")
            .and_then(|inputs| member(inputs, "executors"))
            .and_then(strings_at)
        {
            out.executors = executors;
        }
        if let Some(seq) = member(json, "embedded")
            .and_then(|embedded| member(embedded, "seq"))
            .and_then(|seq| seq.trim().parse::<u64>().ok())
        {
            out.embedded_seq = seq;
            if out.stream == "embedded" {
                out.seq = seq;
            }
        }
        out
    }
}

/// One immediate JSON object member, bounded to its value. Nested publisher
/// receipts and asset objects cannot supply the binary's own sequence.
fn member<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let mut rest = json.trim().strip_prefix('{')?;
    loop {
        rest = rest.trim_start();
        let (name, used) = string_at(rest)?;
        rest = rest[used..].trim_start().strip_prefix(':')?.trim_start();
        let mut depth = 0u32;
        let mut quoted = false;
        let mut escaped = false;
        let mut end = rest.len();
        for (i, byte) in rest.bytes().enumerate() {
            if quoted {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    quoted = false;
                }
                continue;
            }
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' if depth > 0 => depth -= 1,
                b',' | b'}' if depth == 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        if quoted || depth != 0 || end == rest.len() {
            return None;
        }
        if name == key {
            return Some(rest[..end].trim_end());
        }
        rest = rest[end..].strip_prefix(',')?;
    }
}

/// One JSON string at the head of `s`: its value, and how many bytes it took.
fn string_at(s: &str) -> Option<(String, usize)> {
    let body = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, i + 2)),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let hex: String = (0..4)
                        .filter_map(|_| chars.next().map(|(_, c)| c))
                        .collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

/// A JSON array of strings at the head of `s`. Anything else is `None`.
fn strings_at(s: &str) -> Option<Vec<String>> {
    let mut rest = s.strip_prefix('[')?;
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.starts_with(']') {
            return Some(out);
        }
        let (value, used) = string_at(rest)?;
        out.push(value);
        rest = rest[used..].trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_binary_analysis_marker_refuses_launch() {
        assert!(refuse_analysis(r#"{"embedded":{"seq":null,"analysis":true}}"#).is_err());
        for json in [
            r#"{"embedded":{"seq":41}}"#,
            r#"{"embedded":{"seq":0,"genesis":true,"analysis":false}}"#,
            r#"{"embedded":{"analysis":"true"}}"#,
            r#"{"embedded":{"assets":[{"analysis":true}],"seq":41}}"#,
            r#"{"receipt":{"embedded":{"analysis":true}},"embedded":{"seq":41}}"#,
            r#"{"analysis":true,"embedded":{"seq":41}}"#,
            r#"{"embedded":{},"analysis":true}"#,
            "{}",
        ] {
            assert_eq!(refuse_analysis(json), Ok(()), "{json}");
        }
    }

    /// A real `compat.json` line, shortened: the id first, then the sorted
    /// inputs with the executor set and the store's `L` among them.
    const COMPAT: &str = concat!(
        r#"{"id":"9f1c0a2b3d4e5f60718293a4b5c6d7e8","inputs":{"app":"io.exact.caltrain","#,
        r#""executors":["native"],"grantCeiling":"net.fetch https://a/\"x\"","#,
        r#""store":{"L":"A","acceptedKinds":["assets","plan"]},"storeCodec":1}}"#,
        "\n"
    );

    #[test]
    fn the_default_is_the_embedded_answer() {
        let d = Delivery::default();
        assert_eq!(d.stream, "embedded");
        assert_eq!((d.seq, d.embedded_seq, d.staged), (0, 0, false));
        assert_eq!(d.sunset, None);
        assert!(d.interpreted.is_empty());
        assert_eq!(d.store, 'A');
        assert_eq!(d.executors, ["native"]);
        assert_eq!(d.field("compatibilityId"), Some(Value::str("")));
        assert_eq!(d.field("sunset"), Some(Value::str("")));
        assert_eq!(d.field("nope"), None);
    }

    #[test]
    fn a_compat_file_gives_up_the_id_the_store_and_the_executors() {
        let d = Delivery::default().with_compat(COMPAT);
        assert_eq!(d.compatibility_id, "9f1c0a2b3d4e5f60718293a4b5c6d7e8");
        assert_eq!(d.store, 'A');
        assert_eq!(d.executors, ["native"]);
        // Everything the store has to say is untouched.
        assert_eq!(d.stream, "embedded");
        assert!(!d.staged);
    }

    #[test]
    fn a_stripped_build_and_two_executors_read_as_themselves() {
        let json = r#"{"id":"abc","inputs":{"executors":["hermes","native"],"store":{"L":"0"}}}"#;
        let d = Delivery::default().with_compat(json);
        assert_eq!(d.store, '0');
        assert_eq!(d.executors, ["hermes", "native"]);
        assert_eq!(d.compatibility_id, "abc");
    }

    #[test]
    fn the_baked_sequence_is_visible_without_a_store_and_preserves_a_selection() {
        let json = r#"{"id":"abc","inputs":{"store":{"L":"0"},"executors":[]},"embedded":{"assets":[{"seq":999}],"seq":41},"artifacts":[{"embedded":{"seq":888}}]}"#;
        let baked = Delivery::default().with_compat(json);
        assert_eq!((baked.seq, baked.embedded_seq, baked.store), (41, 41, '0'));
        let selected = Delivery {
            stream: "prod/abc".into(),
            seq: 73,
            ..Delivery::default()
        }
        .with_compat(json);
        assert_eq!((selected.seq, selected.embedded_seq), (73, 41));
        for bad in ["null", "-1", "41.5", "18446744073709551616"] {
            let invalid = format!(r#"{{"embedded":{{"seq":{bad}}},"artifacts":[{{"seq":999}}]}}"#);
            assert_eq!(selected.with_compat(&invalid), selected);
        }
    }

    #[test]
    fn a_file_that_says_nothing_leaves_every_fact_as_it_was() {
        let before = Delivery {
            stream: "prod/abc".into(),
            seq: 12,
            ..Delivery::default()
        };
        assert_eq!(before.with_compat(""), before);
        assert_eq!(before.with_compat("{\"id\":"), before);
        assert_eq!(before.with_compat("not json at all"), before);
    }
}
