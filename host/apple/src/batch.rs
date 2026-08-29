//! The batch: what the presenter applies, as JSON built by hand (the shape
//! is nine op kinds and a handful of strings).
//!
//! @ref LLP 1008 §1

use std::fmt::Write as _;

/// A JSON writer for one batch.
#[derive(Debug, Default)]
pub struct Batch {
    ops: Vec<String>,
}

/// Quote a string as JSON.
pub fn quote(s: &str, out: &mut String) {
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

fn id_list(ids: &[u32], out: &mut String) {
    out.push('[');
    for (i, c) in ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{c}");
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

    /// `{"op":"create","id":…,"kind":…,"props":{…},"style":{…},"handlers":[…]}`;
    /// `style` is a JSON object (`style::style_json`).
    pub fn create(
        &mut self,
        id: u32,
        kind: &str,
        props: &[(&str, String)],
        style: &str,
        handlers: &[&str],
    ) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"create\",\"id\":{id},\"kind\":");
        quote(kind, &mut s);
        s.push_str(",\"props\":");
        string_map(props, &mut s);
        let _ = write!(s, ",\"style\":{style},\"handlers\":");
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

    /// `{"op":"style","id":…,"style":{…}}` — the whole dictionary.
    pub fn style(&mut self, id: u32, style: &str) {
        self.ops.push(format!(
            "{{\"op\":\"style\",\"id\":{id},\"style\":{style}}}"
        ));
    }

    /// `{"op":"children","id":…,"ids":[…]}`.
    pub fn children(&mut self, id: u32, ids: &[u32]) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"children\",\"id\":{id},\"ids\":");
        id_list(ids, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"surface","id":…,"name":…,"values":[…]}` — a canvas's inputs
    /// (LLP 1009 D2): plan values as JSON — numbers, strings, booleans,
    /// `null` for unit and `none`, lists, records as positional lists.
    pub fn surface(&mut self, id: u32, name: &str, values: &[exact_plan::Value]) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"surface\",\"id\":{id},\"name\":");
        quote(name, &mut s);
        s.push_str(",\"values\":[");
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            value_json(v, &mut s);
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
        let mut s = String::from("{\"op\":\"roots\",\"ids\":");
        id_list(ids, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"frame","id":…,"x":…,"y":…,"w":…,"h":…}` — the node's frame in
    /// its parent's coordinate space, in points.
    pub fn frame(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32) {
        self.ops.push(format!(
            "{{\"op\":\"frame\",\"id\":{id},\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
            crate::style::num(x),
            crate::style::num(y),
            crate::style::num(w),
            crate::style::num(h)
        ));
    }

    /// `{"op":"content","id":…,"w":…,"h":…}` — a scroll container's content size.
    pub fn content(&mut self, id: u32, w: f32, h: f32) {
        self.ops.push(format!(
            "{{\"op\":\"content\",\"id\":{id},\"w\":{},\"h\":{}}}",
            crate::style::num(w),
            crate::style::num(h)
        ));
    }

    /// `{"op":"present","id":…,"property":…,"x":…,"y":…}` — a motion
    /// property's presentation value this frame (`y` only for `translate`).
    pub fn present(&mut self, id: u32, property: &str, x: f64, y: f64) {
        self.ops.push(format!(
            "{{\"op\":\"present\",\"id\":{id},\"property\":\"{property}\",\"x\":{x},\"y\":{y}}}"
        ));
    }

    /// The batch as one JSON document:
    /// `{"ops":[…],"timers":bool,"motion":bool,"error":null|"…"}`.
    pub fn finish(self, timers: bool, motion: bool, clock_ms: f64, error: Option<&str>) -> String {
        let mut s = String::from("{\"ops\":[");
        s.push_str(&self.ops.join(","));
        let _ = write!(
            s,
            "],\"timers\":{timers},\"motion\":{motion},\"clock\":{clock_ms},\"error\":"
        );
        match error {
            Some(e) => quote(e, &mut s),
            None => s.push_str("null"),
        }
        s.push('}');
        s
    }
}

/// A plan value as JSON.
pub fn value_json(v: &exact_plan::Value, out: &mut String) {
    use exact_plan::Value;
    match v {
        Value::Number(n) if n.is_finite() => {
            let _ = write!(out, "{n}");
        }
        Value::Number(_) | Value::Unit | Value::Option(None) => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Str(s) => quote(s, out),
        Value::Option(Some(inner)) => value_json(inner, out),
        Value::List(items) | Value::Record(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                value_json(item, out);
            }
            out.push(']');
        }
    }
}
