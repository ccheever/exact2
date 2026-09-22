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

    /// @ref LLP 1038 D7 — one coalesced router change beside commands.
    pub fn router(&mut self, change: &exact_runner::RouterChange) {
        let mut s = format!("{{\"op\":\"router\",\"top\":{},\"url\":", change.top);
        quote(&change.url, &mut s);
        s.push_str(",\"removed\":[");
        for (i, id) in change.removed.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "{id}");
        }
        s.push_str("]}");
        self.ops.push(s);
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

    /// A canvas binding, preserving positional values or authored argument names.
    pub fn surface(&mut self, update: &exact_runner::SurfaceUpdate) {
        let mut s = String::new();
        let _ = write!(s, "{{\"op\":\"surface\",\"id\":{},\"name\":", update.view);
        quote(&update.name, &mut s);
        s.push_str(",\"values\":");
        s.push_str(&update.arguments_json());
        s.push('}');
        self.ops.push(s);
    }

    /// Presenter-owned capture or restore work for one named surface.
    pub fn surface_work(&mut self, request: &exact_runner::RequestOut, refusal: Option<&str>) {
        let Some(work) = request.request.surface.as_deref() else {
            return;
        };
        let (mode, name, bytes) = match work {
            exact_runner::SurfaceRequest::Capture { name } => ("capture", name, None),
            exact_runner::SurfaceRequest::Restore { name, bytes } => {
                ("restore", name, Some(bytes.as_slice()))
            }
        };
        let mut s = format!(
            "{{\"op\":\"surfaceWork\",\"ticket\":{},\"mode\":\"{mode}\",\"name\":",
            request.ticket
        );
        quote(name, &mut s);
        if let Some(bytes) = bytes.filter(|_| refusal.is_none()) {
            s.push_str(",\"body\":\"");
            s.push_str(&exact_runner::agent::base64(bytes));
            s.push('"');
        }
        if let Some(refusal) = refusal {
            s.push_str(",\"refusal\":");
            quote(refusal, &mut s);
        }
        s.push('}');
        self.ops.push(s);
    }

    /// Prepend these ops to an already finished batch from the same writer.
    pub fn prepend_to(self, finished: &mut String) {
        if self.ops.is_empty() {
            return;
        }
        let at = "{\"ops\":[".len();
        debug_assert!(finished.starts_with("{\"ops\":["));
        let mut text = self.ops.join(",");
        if finished.as_bytes().get(at) != Some(&b']') {
            text.push(',');
        }
        finished.insert_str(at, &text);
    }

    /// `{"op":"command","name":…,"args":[…]}` — a capability an action
    /// called (LLP 1005 §3), for the presenter to execute after the commit.
    pub fn command(&mut self, name: &str, args: &[exact_plan::Value]) {
        let mut s = String::from("{\"op\":\"command\",\"name\":");
        quote(name, &mut s);
        s.push_str(",\"args\":[");
        for (i, v) in args.iter().enumerate() {
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

    /// `{"op":"content","id":…,"w":…,"h":…}` — natural scrollable extent;
    /// the platform presenter applies its client-size minimum.
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

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::{Request, RequestOut};

    #[test]
    fn surface_work_prepends_typed_bytes_or_a_refusal() {
        let mut extra = Batch::new();
        extra.surface_work(
            &RequestOut {
                ticket: 7,
                target: "continue".into(),
                request: Request::restore_surface("world", vec![0, 128, 255]),
                forced: false,
            },
            None,
        );
        let mut finished = Batch::new().finish(false, false, 0., None);
        extra.prepend_to(&mut finished);
        assert!(finished.contains(r#""mode":"restore","name":"world","body":"AID/""#));

        let mut refused = Batch::new();
        refused.surface_work(
            &RequestOut {
                ticket: 8,
                target: "save".into(),
                request: Request::restore_surface("world", vec![1, 2, 3]),
                forced: false,
            },
            Some("outside the app's grants"),
        );
        let wire = refused.finish(false, false, 0., None);
        assert!(wire.contains(r#""refusal":"outside the app's grants""#));
        assert!(!wire.contains("body"));
    }
}
