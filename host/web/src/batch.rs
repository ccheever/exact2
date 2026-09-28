//! The batch: what the glue applies, as JSON built by hand (no serde in the
//! wasm; the shape is six op kinds and a handful of strings). The ops a
//! page's boot and first press send are written piece by piece
//! (`exact_num::text!`, `format!`'s `{}` without `core::fmt`); the drag
//! ops still format.

use exact_num::{push_text, text, Piece, Shortest};

/// A JSON writer for one batch.
#[derive(Debug, Default)]
pub struct Batch {
    ops: Vec<String>,
    collection_accepted: bool,
}

/// Validate before token translation or storage/continuation serialization.
pub(crate) fn request_refusal(request: &exact_runner::Request) -> Option<&'static str> {
    if let exact_runner::HttpScheduling::Independent { max_response_bytes } = request.http {
        if request.storage.is_some() || request.continuation.is_some() {
            return Some("only HTTP may opt into independent transport");
        }
        if max_response_bytes == 0 || max_response_bytes > 64 * 1024 * 1024 {
            return Some("independent HTTP response limit must be 1..=64 MiB");
        }
    }
    None
}

pub(crate) fn quote(s: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('"');
    // Every byte that needs an escape is ASCII, so the runs between them
    // end on character boundaries and are copied whole.
    let mut run = 0;
    for (at, b) in s.bytes().enumerate() {
        let escape = match b {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0..0x20 => "",
            _ => continue,
        };
        out.push_str(&s[run..at]);
        if escape.is_empty() {
            out.push_str("\\u00");
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 15)]));
        } else {
            out.push_str(escape);
        }
        run = at + 1;
    }
    out.push_str(&s[run..]);
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
    /// The resolved strings table owns the document's language and direction.
    pub fn language(&mut self, lang: &str, dir: &str) {
        let mut s = String::from("{\"op\":\"language\",\"lang\":");
        quote(lang, &mut s);
        s.push_str(",\"dir\":");
        quote(dir, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    pub(crate) fn reorder_drag(
        &mut self,
        view: u32,
        runtime: u64,
        handle: exact_kernel::NodeKey,
        binding: Option<exact_runner::ReorderBinding>,
        kernel: &exact_kernel::Kernel,
    ) {
        use exact_kernel::motion::motion_node;
        let keys = binding.map(|b| [b.list, b.wrapper, b.root]);
        let id = |i: usize| {
            keys.and_then(|k| kernel.node_by_key(k[i]))
                .map_or("null".into(), |n| n.id.to_string())
        };
        let key = |i: usize| keys.map_or("null".into(), |k| format!("\"{}\"", motion_node(k[i])));
        self.ops.push(format!("{{\"op\":\"reorder-drag\",\"id\":{view},\"runtime\":\"{runtime}\",\"handleKey\":\"{}\",\"list\":{},\"listKey\":{},\"wrapper\":{},\"wrapperKey\":{},\"rootKey\":{},\"rowEpoch\":\"{}\"}}",motion_node(handle),id(0),key(0),id(1),key(1),key(2),binding.map_or(0,|b|b.row_epoch)));
    }
    pub(crate) fn reorder_state(
        &mut self,
        runtime: u64,
        token: u64,
        terminal: bool,
        released: bool,
        frame: &str,
    ) {
        self.ops.push(format!("{{\"op\":\"reorder-state\",\"runtime\":\"{runtime}\",\"token\":\"{token}\",\"terminal\":{terminal},\"released\":{released},\"frame\":{frame}}}"));
    }

    /// Geometry committed, even if its subsequent edge action refused.
    pub(crate) fn accept_collection(&mut self) {
        self.collection_accepted = true;
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

    /// A resolved authored handle. Packed generational keys remain decimal
    /// strings; JavaScript Numbers cannot preserve all NodeKey bits.
    pub fn height_drag(
        &mut self,
        view: u32,
        handle: exact_kernel::NodeKey,
        target: Option<(exact_kernel::NodeKey, u32)>,
    ) {
        use exact_kernel::motion::motion_node;
        self.ops.push(format!(
            "{{\"op\":\"height-drag\",\"id\":{view},\"target\":{},\"handleKey\":\"{}\",\"targetKey\":{}}}",
            target.map_or("null".into(), |(_, view)| view.to_string()),
            motion_node(handle),
            target.map_or("null".into(), |(key, _)| format!("\"{}\"", motion_node(key))),
        ));
    }

    /// Empty.
    pub fn new() -> Batch {
        Batch::default()
    }

    /// @ref LLP 1038 D7 — one coalesced router change beside commands.
    pub fn router(&mut self, change: &exact_runner::RouterChange) {
        let mut s = text!("{{\"op\":\"router\",\"top\":{},\"url\":", change.top);
        quote(&change.url, &mut s);
        s.push_str(",\"removed\":[");
        for (i, id) in change.removed.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            id.push_to(&mut s);
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// Full live collection metadata, serialized by the common runner seam.
    pub(crate) fn collections(&mut self, snapshots: &str) {
        self.ops
            .push(text!("{{\"op\":\"collections\",\"items\":{}}}", snapshots));
    }

    /// A terminal admission refusal, delivered after the enclosing DOM batch.
    pub(crate) fn refuse(&mut self, ticket: u64, message: &str) {
        let mut out = text!("{{\"op\":\"refuse\",\"ticket\":{},\"message\":", ticket);
        quote(message, &mut out);
        out.push('}');
        self.ops.push(out);
    }

    /// Wrapping contexts and their paragraph/exclusion identities; geometry is the DOM's.
    /// @ref LLP 1043.000 §3 D2, D7
    pub(crate) fn textflow(&mut self, contexts: &str) {
        self.ops
            .push(text!("{{\"op\":\"textflow\",\"contexts\":{}}}", contexts));
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// `{"op":"create","id":…,"tag":…,"props":{…},"css":…,"handlers":[…]}`,
    /// plus `"ns":"http://www.w3.org/2000/svg"` for an SVG element (LLP 1055 D4).
    pub fn create(
        &mut self,
        id: u32,
        tag: &str,
        props: &[(&str, String)],
        css: &str,
        handlers: &[&str],
    ) {
        let mut s = text!("{{\"op\":\"create\",\"id\":{},\"tag\":", id);
        quote(tag, &mut s);
        if matches!(
            tag,
            "svg"
                | "g"
                | "path"
                | "polyline"
                | "polygon"
                | "circle"
                | "ellipse"
                | "line"
                | "rect"
                | "defs"
                | "linearGradient"
                | "radialGradient"
                | "stop"
                | "use"
                | "symbol"
                | "clipPath"
                | "marker"
                | "mask"
                | "pattern"
                | "foreignObject"
                | "filter"
                | "text"
                | "tspan"
        ) || tag.starts_with("fe")
        {
            s.push_str(",\"ns\":\"http://www.w3.org/2000/svg\"");
        }
        s.push_str(",\"props\":");
        string_map(props, &mut s);
        s.push_str(",\"css\":");
        quote(css, &mut s);
        s.push_str(",\"handlers\":");
        string_list(handlers, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"head","title":…,"description":…,"image":…,"canonical":…,
    /// "robots":…,"status":…}`: the active head's fields, `null` where none
    /// is set (LLP 1048.003 D1).
    pub fn head(&mut self, head: &exact_runner::Head) {
        let mut s = String::from("{\"op\":\"head\"");
        for (name, value) in head.fields() {
            push_text!(&mut s, ",\"{}\":", name);
            match value {
                Some(value) => quote(value, &mut s),
                None => s.push_str("null"),
            }
        }
        match head.status {
            Some(code) => {
                push_text!(&mut s, ",\"status\":{}", code);
            }
            None => s.push_str(",\"status\":null"),
        }
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"adopt","adopted":…}`: whether the page's document is this
    /// runtime's own first tree (LLP 1048.000 D6), so the glue binds it
    /// rather than replacing it.
    pub fn adopt(&mut self, adopted: bool) {
        self.ops
            .push(text!("{{\"op\":\"adopt\",\"adopted\":{}}}", adopted));
    }

    /// `{"op":"props","id":…,"set":{…},"clear":[…]}`.
    pub fn props(&mut self, id: u32, set: &[(&str, String)], clear: &[&str]) {
        let mut s = text!("{{\"op\":\"props\",\"id\":{},\"set\":", id);
        string_map(set, &mut s);
        s.push_str(",\"clear\":");
        string_list(clear, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"keyframes","name":…,"css":…}`: one `@keyframes` rule for the
    /// page's stylesheet, sent once per name before a node names it (LLP 1055 D7).
    pub fn keyframes(&mut self, name: &str, css: &str) {
        let mut s = String::from("{\"op\":\"keyframes\",\"name\":");
        quote(name, &mut s);
        s.push_str(",\"css\":");
        quote(css, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"style","id":…,"css":…}` — the whole `cssText`.
    pub fn style(&mut self, id: u32, css: &str) {
        let mut s = text!("{{\"op\":\"style\",\"id\":{},\"css\":", id);
        quote(css, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"children","id":…,"ids":[…]}`.
    pub fn children(&mut self, id: u32, ids: &[u32]) {
        let mut s = text!("{{\"op\":\"children\",\"id\":{},\"ids\":[", id);
        for (i, c) in ids.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            c.push_to(&mut s);
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
        let mut s = text!("{{\"op\":\"animate\",\"id\":{},\"property\":", id);
        quote(property, &mut s);
        let (delay_ms, duration_ms) = (Shortest(delay_ms), Shortest(duration_ms));
        push_text!(
            &mut s,
            ",\"delay\":{},\"duration\":{},\"values\":[",
            delay_ms,
            duration_ms
        );
        for (i, (x, y)) in values.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let (x, y) = (Shortest(*x), Shortest(*y));
            if pair {
                push_text!(&mut s, "[{},{}]", x, y);
            } else {
                x.push_to(&mut s);
            }
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// End a property's ownership, including a held presentation override.
    pub fn retire_motion(&mut self, id: u32, property: &str) {
        let mut s = text!("{{\"op\":\"retire-motion\",\"id\":{},\"property\":", id);
        quote(property, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// Watch a 2D canvas's box (LLP 1056 D4): the page reports its geometry.
    pub fn canvas2d_watch(&mut self, view: u32) {
        self.ops.push(text!(
            "{{\"op\":\"canvas2d\",\"id\":{},\"watch\":true}}",
            view
        ));
    }

    /// Image handles for the page to load for Canvas 2D (LLP 1056 D9).
    pub fn canvas2d_images(&mut self, srcs: &[String]) {
        let mut s = String::from("{\"op\":\"canvas2d\",\"images\":[");
        for (i, src) in srcs.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(src, &mut s);
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// Whether a 2D canvas wants the page's animation frames (LLP 1056 D5).
    pub fn canvas2d_frames(&mut self, frames: bool) {
        self.ops
            .push(text!("{{\"op\":\"canvas2d\",\"frames\":{}}}", frames));
    }

    /// A 2D canvas's stamped lists (LLP 1056 D4), base64, in order; `fresh`
    /// starts a new bitmap at `w`×`h`.
    pub fn canvas2d(&mut self, c: &exact_runner::CanvasList) {
        let mut s = text!(
            "{{\"op\":\"canvas2d\",\"id\":{},\"lifetime\":{},\"generation\":{},\"seq\":{},\"fresh\":{},\"w\":{},\"h\":{},\"scale\":{},\"stretch\":{},\"lists\":[",
            c.view,
            Shortest(c.lifetime as f64),
            c.generation,
            Shortest(c.seq as f64),
            c.fresh,
            c.pixel_width,
            c.pixel_height,
            Shortest(c.scale),
            c.stretch
        );
        for (i, l) in c.lists.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push('"');
            s.push_str(&exact_runner::agent::base64(l));
            s.push('"');
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// A canvas binding, preserving positional values or authored argument names.
    pub fn surface(&mut self, update: &exact_runner::SurfaceUpdate) {
        let mut s = text!("{{\"op\":\"surface\",\"id\":{},\"name\":", update.view);
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
        if let Some(message) = request_refusal(&r.request) {
            self.refuse(r.ticket, message);
            return;
        } else if let Some(work) = r.request.surface.as_deref() {
            let (mode, name, bytes) = match work {
                exact_runner::SurfaceRequest::Capture { name } => ("capture", name, None),
                exact_runner::SurfaceRequest::Restore { name, bytes } => {
                    ("restore", name, Some(bytes.as_slice()))
                }
            };
            let mixed = r.request.continuation.is_some()
                || r.request.storage.is_some()
                || r.request.method != "GET"
                || !r.request.url.is_empty()
                || !r.request.headers.is_empty()
                || !r.request.body.is_empty();
            let oversized =
                bytes.is_some_and(|bytes| bytes.len() > exact_runner::MAX_HOST_WORK_BYTES);
            let mut s = text!(
                "{{\"op\":\"surfaceWork\",\"ticket\":{},\"mode\":\"{}\",\"name\":",
                r.ticket,
                mode
            );
            quote(name, &mut s);
            s.push_str(",\"scope\":");
            if let Some(scope) = &r.request.grants {
                quote(scope, &mut s)
            } else {
                s.push_str("null")
            }
            if let Some(bytes) = bytes.filter(|_| !mixed && !oversized) {
                s.push_str(",\"body\":\"");
                s.push_str(&exact_runner::agent::base64(bytes));
                s.push('"');
            }
            if oversized {
                s.push_str(",\"refusal\":\"surface restore exceeds 16 MiB\"");
            } else if mixed {
                s.push_str(",\"refusal\":\"surface request combines multiple host-work kinds\"");
            }
            s.push('}');
            self.ops.push(s);
            return;
        }
        if let Some(token) = r.request.continuation {
            self.ops.push(text!(
                "{{\"op\":\"continue\",\"ticket\":{},\"token\":{}}}",
                r.ticket,
                token
            ));
            return;
        }
        if let Some(payload) = &r.request.storage {
            let mut s = text!("{{\"op\":\"storage\",\"ticket\":{},\"payload\":", r.ticket);
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
        let mut s = text!("{{\"op\":\"request\",\"ticket\":{},\"target\":", r.ticket);
        quote(&r.target, &mut s);
        s.push_str(",\"scope\":");
        if let Some(scope) = &r.request.grants {
            quote(scope, &mut s)
        } else {
            s.push_str("null")
        }
        if let exact_runner::HttpScheduling::Independent { max_response_bytes } = r.request.http {
            push_text!(
                &mut s,
                ",\"nativeHttp\":\"independent\",\"maxResponseBytes\":{}",
                max_response_bytes
            );
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
    pub fn command(&mut self, name: &str, args: &[exact_plan::Value], source: Option<u32>) {
        let mut s = String::from("{\"op\":\"command\",\"name\":");
        quote(name, &mut s);
        s.push_str(",\"args\":[");
        for (i, v) in args.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            value_json(v, &mut s);
        }
        s.push(']');
        if let Some(id) = source {
            s.push_str(&text!(",\"source\":{}", id));
        }
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"destroy","id":…}`.
    pub fn destroy(&mut self, id: u32) {
        self.ops.push(text!("{{\"op\":\"destroy\",\"id\":{}}}", id));
    }

    /// `{"op":"exit","id":…,"css":…}` — the view leaves with `css`, the CSS
    /// `animation` list of its `exit-animation` (LLP 1063): the page keeps
    /// it, inert, where it was until the exit ends. Its `destroy` ops follow
    /// as usual.
    pub fn exit(&mut self, id: u32, css: &str) {
        let mut s = text!("{{\"op\":\"exit\",\"id\":{},\"css\":", id);
        quote(css, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"at","ms":…}` — the clock at which the ops that follow were
    /// committed (a timer's due time inside one `advance`), so a page that
    /// owns time can attribute the transitions they start to that instant
    /// (LLP 1012: one seek and sixty give the same bits).
    pub fn at(&mut self, ms: f64) {
        self.ops
            .push(text!("{{\"op\":\"at\",\"ms\":{}}}", Shortest(ms)));
    }

    /// `{"op":"roots","ids":[…]}`.
    pub fn roots(&mut self, ids: &[u32]) {
        let mut s = String::from("{\"op\":\"roots\",\"ids\":[");
        for (i, c) in ids.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            c.push_to(&mut s);
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// @ref LLP 1043.000 §3 D8 — carry the runner deadline, not a poll interval.
    /// The batch as one JSON document:
    /// `{"ops":[…],"timers":bool,"clock":ms,"error":null|"…"}` — `clock` is
    /// the runner's clock after the call (an advance a timer refused stops at
    /// that timer's due time).
    pub fn finish(self, timer_due_ms: Option<f64>, clock_ms: f64, error: Option<&str>) -> String {
        let mut s = String::from("{\"ops\":[");
        for (i, op) in self.ops.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(op);
        }
        s.push(']');
        let timers = timer_due_ms.is_some();
        if let Some(due) = timer_due_ms {
            push_text!(&mut s, ",\"timer_due_ms\":{}", Shortest(due));
        }
        if self.collection_accepted {
            s.push_str(",\"accepted\":true");
        }
        push_text!(
            &mut s,
            ",\"timers\":{},\"clock\":{},\"error\":",
            timers,
            Shortest(clock_ms)
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
        Value::Number(n) if n.is_finite() => Shortest(*n).push_to(out),
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
        let next = super::Batch::new().finish(Some(16.0), 0.0, None);
        assert!(next.contains("\"timer_due_ms\":16,"), "{next}");
        assert!(next.contains("\"timers\":true"), "{next}");
        let empty = super::Batch::new().finish(None, 0.0, None);
        assert!(!empty.contains("timer_due_ms"), "{empty}");
        assert!(empty.contains("\"timers\":false"), "{empty}");
    }
}

#[cfg(test)]
mod quote_tests {
    #[test]
    fn strings_are_json_with_every_control_escaped() {
        let mut out = String::new();
        super::quote("a\"b\\c\nd\re\tf\u{1}g\u{1f}h\u{7f}é€😀", &mut out);
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\re\\tf\\u0001g\\u001fh\u{7f}é€😀\"");
    }
}
