//! The batch: what the glue applies, as JSON built by hand (no serde in the
//! wasm; the shape is six op kinds and a handful of strings).

use std::fmt::Write as _;

/// A JSON writer for one batch.
#[derive(Debug, Default)]
pub struct Batch {
    ops: Vec<String>,
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn string_map(pairs: &[(&str, String)], out: &mut String) {
    out.push('{');
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote(k, out);
        out.push(':');
        quote(v, out);
    }
    out.push('}');
}

fn string_list(items: &[&str], out: &mut String) {
    out.push('[');
    for (i, s) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote(s, out);
    }
    out.push(']');
}

impl Batch {
    /// Empty.
    pub fn new() -> Batch {
        Batch::default()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// `{"op":"create","id":…,"tag":…,"props":{…},"css":…,"handlers":[…]}`.
    pub fn create(
        &mut self,
        id: u32,
        tag: &str,
        props: &[(&str, String)],
        css: &str,
        handlers: &[&str],
    ) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"create\",\"id\":{id},\"tag\":");
        quote(tag, &mut s);
        s.push_str(",\"props\":");
        string_map(props, &mut s);
        s.push_str(",\"css\":");
        quote(css, &mut s);
        s.push_str(",\"handlers\":");
        string_list(handlers, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"props","id":…,"set":{…},"clear":[…]}`.
    pub fn props(&mut self, id: u32, set: &[(&str, String)], clear: &[&str]) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"props\",\"id\":{id},\"set\":");
        string_map(set, &mut s);
        s.push_str(",\"clear\":");
        string_list(clear, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"style","id":…,"css":…}` — the whole `cssText`.
    pub fn style(&mut self, id: u32, css: &str) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"style\",\"id\":{id},\"css\":");
        quote(css, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"children","id":…,"ids":[…]}`.
    pub fn children(&mut self, id: u32, ids: &[u32]) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"children\",\"id\":{id},\"ids\":[");
        for (i, c) in ids.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "{c}");
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// `{"op":"animate","id":…,"property":…,"delay":ms,"duration":ms,"values":[…]}`
    /// — a spring's frames, evenly spaced; `translate` values are `[x,y]`
    /// pairs, the rest numbers. No values means stop playing the property.
    pub fn animate(
        &mut self,
        id: u32,
        property: &str,
        delay_ms: f64,
        duration_ms: f64,
        values: &[(f64, f64)],
        pair: bool,
    ) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"animate\",\"id\":{id},\"property\":");
        quote(property, &mut s);
        let _ = write!(
            s,
            ",\"delay\":{delay_ms},\"duration\":{duration_ms},\"values\":["
        );
        for (i, (x, y)) in values.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            if pair {
                let _ = write!(s, "[{x},{y}]");
            } else {
                let _ = write!(s, "{x}");
            }
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// `{"op":"destroy","id":…}`.
    pub fn destroy(&mut self, id: u32) {
        self.ops.push(format!("{{\"op\":\"destroy\",\"id\":{id}}}"));
    }

    /// `{"op":"roots","ids":[…]}`.
    pub fn roots(&mut self, ids: &[u32]) {
        let mut s = String::from("{\"op\":\"roots\",\"ids\":[");
        for (i, c) in ids.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "{c}");
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// The batch as one JSON document: `{"ops":[…],"timers":bool,"error":null|"…"}`.
    pub fn finish(self, timers: bool, error: Option<&str>) -> String {
        let mut s = String::from("{\"ops\":[");
        s.push_str(&self.ops.join(","));
        let _ = write!(s, "],\"timers\":{timers},\"error\":");
        match error {
            Some(e) => quote(e, &mut s),
            None => s.push_str("null"),
        }
        s.push('}');
        s
    }
}
