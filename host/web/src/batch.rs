//! The batch: what the glue applies, as JSON built by hand (no serde in the
//! wasm; the shape is six op kinds and a handful of strings).

use std::fmt::Write as _;

/// A JSON writer for one batch.
#[derive(Debug, Default)]
pub struct Batch {
    ops: Vec<String>,
}

pub(crate) fn quote(s: &str, out: &mut String) {
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

    /// `{"op":"request","ticket":N,"target":…,"method":…,"url":…,"headers":[[k,v]…],"body":"<base64>","cache":"default"|"reload"}`
    /// — a request the runner handed the host to run (LLP 1016 D2); the
    /// reply comes back through `exact_fulfill`.
    pub fn request(&mut self, r: &exact_runner::RequestOut) {
        if let Some(token) = r.request.continuation {
            self.ops.push(format!(
                "{{\"op\":\"continue\",\"ticket\":{},\"token\":{token}}}",
                r.ticket
            ));
            return;
        }
        if let Some(payload) = &r.request.storage {
            let mut s = format!("{{\"op\":\"storage\",\"ticket\":{},\"payload\":", r.ticket);
            // Empty text is invalid JSON and refuses before effects; never repair
            // malformed bytes into a different, executable storage request.
            quote(std::str::from_utf8(payload).unwrap_or(""), &mut s);
            s.push_str(",\"scope\":");
            if let Some(scope) = &r.request.grants {
                quote(scope, &mut s)
            } else {
                s.push_str("null")
            }
            s.push('}');
            self.ops.push(s);
            return;
        }
        let mut s = format!("{{\"op\":\"request\",\"ticket\":{},\"target\":", r.ticket);
        quote(&r.target, &mut s);
        s.push_str(",\"scope\":");
        if let Some(scope) = &r.request.grants {
            quote(scope, &mut s)
        } else {
            s.push_str("null")
        }
        s.push_str(",\"method\":");
        quote(&r.request.method, &mut s);
        s.push_str(",\"url\":");
        quote(&r.request.url, &mut s);
        s.push_str(",\"headers\":[");
        for (i, (k, v)) in r.request.headers.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push('[');
            quote(k, &mut s);
            s.push(',');
            quote(v, &mut s);
            s.push(']');
        }
        s.push_str("],\"body\":\"");
        s.push_str(&exact_runner::agent::base64(&r.request.body));
        s.push_str("\",\"cache\":\"");
        s.push_str(if r.forced { "reload" } else { "default" });
        s.push_str("\"}");
        self.ops.push(s);
    }

    /// `{"op":"store","tier":"secret","name":…,"value":…|null}` — a secret
    /// the app kept or forgot (LLP 1018 D1), for the page to persist after
    /// the commit (`localStorage` under `exact.secret.<name>`).
    pub fn store(&mut self, w: &exact_runner::StoreWrite) {
        let mut s = String::from("{\"op\":\"store\",\"tier\":\"secret\",\"name\":");
        quote(&w.name, &mut s);
        s.push_str(",\"value\":");
        match &w.value {
            Some(v) => quote(v, &mut s),
            None => s.push_str("null"),
        }
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"grants","lines":[…]}` — the hosts the app may reach (LLP 1016
    /// D6), once at boot; the page refuses a request outside them itself.
    pub fn grants(&mut self, grants: &str) {
        let mut s = String::from("{\"op\":\"grants\",\"lines\":[");
        for (i, line) in grants
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .enumerate()
        {
            if i > 0 {
                s.push(',');
            }
            quote(line, &mut s);
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// `{"op":"command","name":…,"args":[…]}` — a capability an action
    /// called (LLP 1005 §3), for the glue to execute after the commit.
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

    /// `{"op":"at","ms":…}` — the clock at which the ops that follow were
    /// committed (a timer's due time inside one `advance`), so a page that
    /// owns time can attribute the transitions they start to that instant
    /// (LLP 1012: one seek and sixty give the same bits).
    pub fn at(&mut self, ms: f64) {
        self.ops.push(format!("{{\"op\":\"at\",\"ms\":{ms}}}"));
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

    /// The batch as one JSON document:
    /// `{"ops":[…],"timers":bool,"clock":ms,"error":null|"…"}` — `clock` is
    /// the runner's clock after the call (an advance a timer refused stops at
    /// that timer's due time).
    pub fn finish(self, timers: bool, clock_ms: f64, error: Option<&str>) -> String {
        let mut s = String::from("{\"ops\":[");
        s.push_str(&self.ops.join(","));
        s.push(']');
        let _ = write!(s, ",\"timers\":{timers},\"clock\":{clock_ms},\"error\":");
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
