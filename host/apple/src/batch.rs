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

pub use exact_runner::agent::quote;

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
    /// @ref LLP 1043.000 §3 D4 — empty geometry clears a former flow.
    pub fn flow(&mut self, id: u32, shapes: &[exact_kernel::FlowShape]) {
        let mut s = format!("{{\"op\":\"flow\",\"id\":{id},\"shapes\":[");
        for (i, shape) in shapes.iter().enumerate() {
            if i != 0 {
                s.push(',');
            }
            exact_runner::agent::flow_shape_json(shape, &mut s);
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// The caller has preflighted source/structural/wire bounds. Allocate the
    /// staging vector before encoding; appending moves complete ops only.
    pub(crate) fn staging(ops: usize) -> Result<Self, String> {
        let mut out = Self::new();
        out.ops
            .try_reserve_exact(ops)
            .map_err(|_| "native diff allocation")?;
        Ok(out)
    }
    pub(crate) fn append_checked(&mut self, other: Self, max: usize) -> Result<(), String> {
        let mut bytes = 0usize;
        for op in &other.ops {
            bytes = bytes
                .checked_add(op.len())
                .and_then(|n| n.checked_add(1))
                .ok_or("native diff byte overflow")?;
        }
        if bytes > max {
            return Err("native encoded diff capacity".into());
        }
        self.ops
            .try_reserve_exact(other.ops.len())
            .map_err(|_| "native append allocation")?;
        self.ops.extend(other.ops);
        Ok(())
    }
    pub(crate) fn region(&mut self, json: &str) {
        self.ops.push(json.into());
    }
    pub(crate) fn transform_drag(
        &mut self,
        view: u32,
        runtime: u64,
        handle: exact_kernel::NodeKey,
        binding: Option<[(exact_kernel::NodeKey, u32); 2]>,
    ) {
        use exact_kernel::motion::motion_node;
        let id = |i: usize| binding.map_or("null".into(), |b| b[i].1.to_string());
        let key =
            |i: usize| binding.map_or("null".into(), |b| format!("\"{}\"", motion_node(b[i].0)));
        self.ops.push(format!("{{\"op\":\"transform-drag\",\"id\":{view},\"runtime\":\"{runtime}\",\"handleKey\":\"{}\",\"target\":{},\"targetKey\":{},\"clip\":{},\"clipKey\":{}}}",motion_node(handle),id(0),key(0),id(1),key(1)));
    }

    pub(crate) fn retire_transform_token(
        &mut self,
        view: u32,
        runtime: u64,
        token: exact_motion::HoldToken,
    ) {
        self.ops.push(format!("{{\"op\":\"retire-motion\",\"id\":{view},\"property\":\"{}\",\"runtime\":\"{runtime}\",\"token\":\"{}\"}}",token.property().name(),token.serial()));
    }

    /// A new native presentation hold. Its serial is a decimal string, never a JSON float.
    pub fn hold(&mut self, token: u64, x: f64, y: f64) {
        self.ops.push(format!(
            "{{\"op\":\"hold\",\"token\":\"{token}\",\"x\":{x},\"y\":{y}}}"
        ));
    }

    /// An authored header's resolved binding, with exact generational keys.
    pub fn height_drag(&mut self, id: u32, handle_key: u64, target: Option<(u32, u64)>) {
        let (target, target_key) = target.map_or_else(
            || ("null".into(), "null".into()),
            |(id, key)| (id.to_string(), format!("\"{key}\"")),
        );
        self.ops.push(format!("{{\"op\":\"height-drag\",\"id\":{id},\"target\":{target},\"handleKey\":\"{handle_key}\",\"targetKey\":{target_key}}}"));
    }

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

    /// Mounted collection metadata from the runner's common JSON array writer.
    /// An empty array clears previously published collections on the presenter.
    pub fn collections(&mut self, items: &str) {
        self.ops
            .push(format!("{{\"op\":\"collections\",\"items\":{items}}}"));
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

    /// @ref LLP 1043.000 §3 D8 — carry the runner deadline, not a poll interval.
    /// The batch as one JSON document:
    /// `{"ops":[…],"timers":bool,"motion":bool,"error":null|"…"}`.
    pub fn finish(
        self,
        timer_due_ms: Option<f64>,
        motion: bool,
        clock_ms: f64,
        error: Option<&str>,
    ) -> String {
        let timers = timer_due_ms.is_some();
        let mut s = String::from("{\"ops\":[");
        for (i, op) in self.ops.iter().enumerate() {
            if i != 0 {
                s.push(',');
            }
            s.push_str(op);
        }
        s.push(']');
        if let Some(due) = timer_due_ms {
            let _ = write!(s, ",\"timer_due_ms\":{due}");
        }
        let _ = write!(
            s,
            ",\"timers\":{timers},\"motion\":{motion},\"clock\":{clock_ms},\"error\":"
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
mod timer_tests {
    #[test]
    fn batches_carry_deadlines_and_omit_them_without_timers() {
        let next = super::Batch::new().finish(Some(16.0), false, 0.0, None);
        assert!(next.contains("\"timer_due_ms\":16,"), "{next}");
        assert!(next.contains("\"timers\":true"), "{next}");
        let empty = super::Batch::new().finish(None, false, 0.0, None);
        assert!(!empty.contains("timer_due_ms"), "{empty}");
        assert!(empty.contains("\"timers\":false"), "{empty}");
    }
}

#[cfg(test)]
mod finish_bytes_tests {
    use super::*;

    // Exact pre-change finish writer. Compare bytes, not parsed JSON equivalence.
    fn old_finish(
        batch: &Batch,
        timers: bool,
        motion: bool,
        clock_ms: f64,
        error: Option<&str>,
    ) -> String {
        let mut s = String::from("{\"ops\":[");
        s.push_str(&batch.ops.join(","));
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
    #[test]
    fn native_publication_ordinary_finish_matches_original_join_bytes() {
        for count in [0, 1, 12] {
            for timers in [false, true] {
                for motion in [false, true] {
                    for clock in [0., -0., 123.25] {
                        for error in [None, Some("refused α\n\"\\\u{0001}")] {
                            let mut batch = Batch::new();
                            batch.create(
                                1,
                                "view",
                                &[("text", "α 👩‍🚀 e\u{301}\n\"\\\u{0001}".into())],
                                "{}",
                                &["press"],
                            );
                            batch.props(1, &[("title", "updated".into())], &["text"]);
                            batch.style(1, "{\"opacity\":0.5}");
                            batch.children(1, &[2, 3]);
                            batch.frame(1, -0., 0.1, 200., 400.);
                            batch.content(1, 200., 800.);
                            batch.present(1, "translate", -0., 0.125);
                            batch.surface(
                                1,
                                "ordinary",
                                &[exact_plan::Value::Str("日本語".into())],
                            );
                            batch.command("focus", &[exact_plan::Value::Number(1.)]);
                            batch.destroy(3);
                            batch.roots(&[1]);
                            batch.collections("[]");
                            batch.ops.truncate(count);
                            let mut expected = old_finish(&batch, timers, motion, clock, error);
                            let deadline = timers.then_some(16.0);
                            if timers {
                                expected = expected.replacen(
                                    ",\"timers\":",
                                    ",\"timer_due_ms\":16,\"timers\":",
                                    1,
                                );
                            }
                            assert_eq!(
                                batch.finish(deadline, motion, clock, error).as_bytes(),
                                expected.as_bytes()
                            );
                        }
                    }
                }
            }
        }
    }
}
