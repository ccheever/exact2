//! The batch: what the presenter applies, as JSON built by hand (the shape
//! is nine op kinds and a handful of strings).
//!
//! @ref LLP 1008 §1

use std::fmt::Write as _;

use crate::style::push_int;

/// A JSON writer for one batch.
#[derive(Debug, Default)]
pub struct Batch {
    ops: Vec<String>,
    /// The views this batch creates: each starts at its presentation's
    /// identity (`Host::present`).
    created: std::collections::HashSet<u32>,
    /// What moves changes place or size (`Engine::spatial`): the display
    /// link asks for the panel's full rate (LLP 1061 D4).
    pub spatial: bool,
    /// A frame task wants each display frame (LLP 1073 D5): the display
    /// link runs and each tick is `exact_frame`.
    pub frames: bool,
    /// The kernel transactions this batch carries, `"seq":[first,last]`,
    /// when the runner measures (LLP 1079 D3): the frame sampler's join.
    pub seq: Option<(u64, u64)>,
    /// A 2D canvas asked for another frame (LLP 1056 D5).
    canvas: bool,
    /// A canvas draw is owed to a turn of its own (LLP 1072 §8.5).
    canvas_owed: bool,
    /// Image handles a 2D canvas asked for (LLP 1056 D9): the presenter
    /// decodes each and answers `exact_canvas_image`.
    images: Vec<String>,
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
        push_int(out, i64::from(*c));
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

    /// One paragraph's complete inline identity/style table, replacing its old runs.
    pub fn paragraph(&mut self, id: u32, runs: &str) {
        self.ops.push(format!(
            "{{\"op\":\"paragraph\",\"id\":{id},\"runs\":{runs}}}"
        ));
    }

    /// Encode a textual descendant without allocating a native view operation.
    pub(crate) fn inline_run(
        out: &mut String,
        id: u32,
        parent: u32,
        props: &std::collections::BTreeMap<String, String>,
        style: &str,
        handlers: &[&str],
        paints: bool,
    ) {
        let _ = write!(
            out,
            "{{\"id\":{id},\"parent\":{parent},\"paint\":{paints},\"props\":"
        );
        string_map(
            &props
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>(),
            out,
        );
        let _ = write!(out, ",\"style\":{style},\"handlers\":");
        string_list(handlers, out);
        out.push('}');
    }

    /// @ref LLP 1043.000 §3 D4 — empty geometry clears a former flow.
    pub fn flow(&mut self, id: u32, shapes: &[exact_kernel::FlowShape]) {
        let mut s = format!("{{\"op\":\"flow\",\"id\":{id},\"shapes\":[");
        for (i, shape) in shapes.iter().enumerate() {
            if i != 0 {
                s.push(',');
            }
            shape.write_json(&mut s);
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

    /// One Arrange contact's state (LLP 1041 §8.5): its reorder serial as a
    /// decimal string, the List and lifted wrapper views (0 once gone), and
    /// `active`, `settling`, `finished` or `refused`.
    pub(crate) fn reorder(&mut self, token: u64, ids: (u32, u32), phase: &str, dispatched: bool) {
        self.ops.push(format!("{{\"op\":\"reorder\",\"token\":\"{token}\",\"list\":{},\"wrapper\":{},\"phase\":\"{phase}\",\"dispatched\":{dispatched}}}", ids.0, ids.1));
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
        // A row's mount is a create per node: written by pushes, not `fmt`.
        let mut s = String::with_capacity(64 + kind.len() + style.len());
        s.push_str("{\"op\":\"create\",\"id\":");
        push_int(&mut s, i64::from(id));
        s.push_str(",\"kind\":");
        quote(kind, &mut s);
        s.push_str(",\"props\":");
        string_map(props, &mut s);
        s.push_str(",\"style\":");
        s.push_str(style);
        s.push_str(",\"handlers\":");
        string_list(handlers, &mut s);
        s.push('}');
        self.ops.push(s);
        self.created.insert(id);
    }

    /// Whether this batch creates `id`.
    pub fn creates(&self, id: u32) -> bool {
        self.created.contains(&id)
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
        let mut s = String::with_capacity(32 + style.len());
        s.push_str("{\"op\":\"style\",\"id\":");
        push_int(&mut s, i64::from(id));
        s.push_str(",\"style\":");
        s.push_str(style);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"children","id":…,"ids":[…]}`.
    pub fn children(&mut self, id: u32, ids: &[u32]) {
        let mut s = String::with_capacity(32 + 8 * ids.len());
        s.push_str("{\"op\":\"children\",\"id\":");
        push_int(&mut s, i64::from(id));
        s.push_str(",\"ids\":");
        id_list(ids, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// A 2D canvas's stamped lists (LLP 1056 D4), in order, for the Core
    /// Graphics replayer: `[address, length]` pairs the reader copies out
    /// as it decodes the batch (the host keeps them alive until then). `fresh` starts a new bitmap at `w`×`h`;
    /// `box` is the content box in the view's border box, where it shows.
    pub fn canvas2d(
        &mut self,
        c: &exact_runner::CanvasList,
        content: (f32, f32, f32, f32),
        radii: [(f32, f32); 4],
    ) {
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\"op\":\"canvas2d\",\"id\":{},\"lifetime\":{},\"generation\":{},\"seq\":{},\"fresh\":{},\"w\":{},\"h\":{},\"scale\":{},\"stretch\":{},\"animating\":{},\"box\":[{},{},{},{}],\"radii\":[{},{},{},{},{},{},{},{}],\"lists\":[",
            c.view, c.lifetime, c.generation, c.seq, c.fresh, c.pixel_width, c.pixel_height, c.scale, c.stretch, c.animating,
            content.0, content.1, content.2, content.3, radii[0].0, radii[0].1, radii[1].0, radii[1].1, radii[2].0, radii[2].1, radii[3].0, radii[3].1
        );
        for (i, l) in c.lists.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "[{},{}]", l.as_ptr() as usize, l.len());
        }
        s.push_str("]}");
        self.ops.push(s);
    }

    /// Whether a 2D canvas wants the next display frame (LLP 1056 D5).
    pub fn canvas_frames(&mut self, wants: bool) {
        self.canvas = wants;
    }

    /// Whether a deferred canvas draw is owed (`exact_canvas_draw`, LLP
    /// 1072 §8.5).
    pub fn canvas_owed(&mut self, owed: bool) {
        self.canvas_owed = owed;
    }

    /// Image handles for the presenter to decode (LLP 1056 D9).
    pub fn canvas_images(&mut self, srcs: Vec<String>) {
        self.images.extend(srcs);
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

    /// `{"op":"auth","ticket":N,"url":…,"callback":…,"ephemeral":…}`: open
    /// an authentication session (LLP 1069.006 D3). The `state` stays in
    /// Rust, which checks the completion.
    pub fn auth(&mut self, ticket: u64, session: &exact_runner::auth::Session) {
        let mut s = format!("{{\"op\":\"auth\",\"ticket\":{ticket},\"url\":");
        quote(&session.url, &mut s);
        s.push_str(",\"callback\":");
        quote(&session.callback, &mut s);
        s.push_str(&format!(",\"ephemeral\":{}}}", session.ephemeral));
        self.ops.push(s);
    }

    /// `{"op":"auth","ticket":N,"cancel":true}`: the runner let go of it.
    pub fn auth_cancel(&mut self, ticket: u64) {
        self.ops.push(format!(
            "{{\"op\":\"auth\",\"ticket\":{ticket},\"cancel\":true}}"
        ));
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
            s.push_str(&format!(",\"source\":{id}"));
        }
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"destroy","id":…}`.
    pub fn destroy(&mut self, id: u32) {
        self.ops.push(format!("{{\"op\":\"destroy\",\"id\":{id}}}"));
    }

    /// `{"op":"exit","id":…}` — the view leaves with its `exit-animation`
    /// (LLP 1063): the presenter keeps it and everything under it where they
    /// are, without input or accessibility, until a `destroy` names it.
    pub fn exit(&mut self, id: u32) {
        self.ops.push(format!("{{\"op\":\"exit\",\"id\":{id}}}"));
    }

    /// `{"op":"flight","id":…,"from":…}` — the view's shared-element name
    /// arrives from `from` (LLP 1013.000 D4). Sent before the batch's
    /// destroys: the presenter captures where `from` is shown, then, once
    /// the batch is applied, lifts `id` and flies it from there to its place
    /// by the `flight` progress it is presented.
    pub fn flight(&mut self, id: u32, from: u32) {
        self.ops
            .push(format!("{{\"op\":\"flight\",\"id\":{id},\"from\":{from}}}"));
    }

    /// `{"op":"land","id":…}` — the flight's curve settled: the view goes
    /// back to its place.
    pub fn land(&mut self, id: u32) {
        self.ops.push(format!("{{\"op\":\"land\",\"id\":{id}}}"));
    }

    /// `{"op":"roots","ids":[…]}`.
    pub fn roots(&mut self, ids: &[u32]) {
        let mut s = String::from("{\"op\":\"roots\",\"ids\":");
        id_list(ids, &mut s);
        s.push('}');
        self.ops.push(s);
    }

    /// `{"op":"title","title":…,"edited":…}`: the active head's title,
    /// `null` when no head sets one (LLP 1048.003 D1), and whether it says
    /// the document is `edited` (LLP 1069.010 D6). The app owning the window
    /// or scene shows them; an embedded view never claims that chrome.
    pub fn title(&mut self, title: Option<&str>, edited: bool) {
        let mut s = String::from("{\"op\":\"title\",\"title\":");
        match title {
            Some(title) => quote(title, &mut s),
            None => s.push_str("null"),
        }
        let _ = write!(s, ",\"edited\":{edited}}}");
        self.ops.push(s);
    }

    /// `{"op":"frame","id":…,"x":…,"y":…,"w":…,"h":…}` — the node's frame in
    /// its parent's coordinate space, in points.
    pub fn frame(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32) {
        // One string per op, the numbers written into it: a list row's
        // mount frames every node.
        let mut s = String::with_capacity(64);
        let _ = write!(s, "{{\"op\":\"frame\",\"id\":{id},\"x\":");
        crate::style::push_num(&mut s, x);
        s.push_str(",\"y\":");
        crate::style::push_num(&mut s, y);
        s.push_str(",\"w\":");
        crate::style::push_num(&mut s, w);
        s.push_str(",\"h\":");
        crate::style::push_num(&mut s, h);
        s.push('}');
        self.ops.push(s);
    }

    /// Twice the sibling paint rank, losslessly encoded (LLP 1083.000 D4).
    pub fn rank(&mut self, id: u32, rank: i64) {
        self.ops
            .push(format!("{{\"op\":\"rank\",\"id\":{id},\"rank\":{rank}}}"));
    }

    /// `{"op":"fragments","id":…,"fragments":[[x,y,w,h,first,end,dx,dy],…],
    /// "columns":[[x,y,w,h,holds],…]}` — a box's fragments in its own frame
    /// and a container's columns in its border box (LLP 1093 D7), or
    /// `{"op":"fragments","id":…}` when it has neither any more.
    pub fn fragments(&mut self, id: u32, record: &str) {
        self.ops
            .push(format!("{{\"op\":\"fragments\",\"id\":{id}{record}}}"));
    }

    /// `{"op":"sticky","id":…,"scroller":…,"natural":[…],"limit":[…],
    /// "port":[…],"insets":[…]}` — a sticky box's constraint (LLP 1083 D3),
    /// or `{"op":"sticky","id":…}` when it is no longer sticky.
    pub fn sticky(&mut self, id: u32, constraint: Option<&exact_kernel::StickyConstraint>) {
        let mut s = format!("{{\"op\":\"sticky\",\"id\":{id}");
        if let Some(c) = constraint {
            let _ = write!(s, ",\"scroller\":{}", c.scroller);
            for (name, rect) in [("natural", c.natural), ("limit", c.limit), ("port", c.port)] {
                let _ = write!(s, ",\"{name}\":[");
                for (i, v) in rect.into_iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    crate::style::push_num(&mut s, v);
                }
                s.push(']');
            }
            s.push_str(",\"insets\":[");
            for (i, v) in c.insets.into_iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                match v {
                    Some(v) => crate::style::push_num(&mut s, v),
                    None => s.push_str("null"),
                }
            }
            s.push(']');
        }
        s.push('}');
        self.ops.push(s);
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

    /// `{"op":"present","id":…,"property":"layout","x":…,"y":…,"w":…,"h":…}`
    /// — a layout transition's offset and scale of the laid-out box (LLP
    /// 1063).
    pub fn present4(&mut self, id: u32, property: &str, [x, y, w, h]: [f64; 4]) {
        self.ops.push(format!(
            "{{\"op\":\"present\",\"id\":{id},\"property\":\"{property}\",\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}"
        ));
    }

    /// `{"op":"svg","id":…,"scene":{…}}`: an `svg`'s whole scene (LLP 1055 D4).
    pub fn svg(&mut self, id: u32, scene: &str) {
        self.ops
            .push(format!("{{\"op\":\"svg\",\"id\":{id},\"scene\":{scene}}}"));
    }

    /// `{"op":"animations","id":…,"specs":[…]}`: a view's Core Animation
    /// specs for its CSS animations (LLP 1055 D7); `[]` removes them.
    pub fn animations(&mut self, id: u32, specs: &str) {
        self.ops.push(format!(
            "{{\"op\":\"animations\",\"id\":{id},\"specs\":{specs}}}"
        ));
    }

    /// @ref LLP 1043.000 §3 D8 — carry the runner deadline, not a poll interval.
    /// The batch as one JSON document:
    /// `{"ops":[…],"timers":bool,"motion":bool,"error":null|"…"}`, with
    /// `"spatial":true` before `timers` when what moves changes place or size.
    pub fn finish(
        self,
        timer_due_ms: Option<f64>,
        motion: bool,
        clock_ms: f64,
        error: Option<&str>,
    ) -> String {
        let timers = timer_due_ms.is_some();
        // One allocation for the ops, which a list row's mount makes by the
        // hundred: growing the string as it went copied it a dozen times.
        let ops: usize = self.ops.iter().map(|op| op.len() + 1).sum();
        let mut s = String::with_capacity(
            ops + 256 + self.images.iter().map(|i| i.len() + 3).sum::<usize>(),
        );
        s.push_str("{\"ops\":[");
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
        if self.spatial {
            s.push_str(",\"spatial\":true");
        }
        if self.frames {
            s.push_str(",\"frames\":true");
        }
        if self.canvas_owed {
            s.push_str(",\"canvasOwed\":true");
        }
        if !self.images.is_empty() {
            s.push_str(",\"canvasImages\":[");
            for (i, src) in self.images.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                quote(src, &mut s);
            }
            s.push(']');
        }
        if let Some((first, last)) = self.seq {
            let _ = write!(s, ",\"seq\":[{first},{last}]");
        }
        let _ = write!(
            s,
            ",\"timers\":{timers},\"motion\":{motion},\"canvas\":{},\"clock\":{clock_ms},\"error\":",
            self.canvas
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
        v @ exact_plan::str_value!() => quote(v.text(), out),
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
        assert!(!empty.contains("frames"), "{empty}");
        let mut frames = super::Batch::new();
        frames.frames = true;
        let frames = frames.finish(None, false, 0.0, None);
        assert!(frames.contains("\"frames\":true"), "{frames}");
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
            "],\"timers\":{timers},\"motion\":{motion},\"canvas\":false,\"clock\":{clock_ms},\"error\":"
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
                            batch.surface(&exact_runner::SurfaceUpdate {
                                view: 1,
                                name: "ordinary".into(),
                                mode: exact_plan::SurfaceArgsMode::Positional,
                                names: vec![],
                                values: vec![exact_plan::Value::str("日本語")],
                            });
                            batch.command("focus", &[exact_plan::Value::Number(1.)], None);
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
        let mut finished = Batch::new().finish(None, false, 0., None);
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
        let wire = refused.finish(None, false, 0., None);
        assert!(wire.contains(r#""refusal":"outside the app's grants""#));
        assert!(!wire.contains("body"));
    }
}

/// The fields of a `fragments` op ([`Batch::fragments`]), each with a leading comma.
pub fn fragments_json(
    fragments: Option<&[exact_kernel::fragment::Fragment]>,
    columns: Option<&exact_kernel::fragment::Columns>,
) -> String {
    let mut s = String::new();
    let list = |s: &mut String, name: &str, rows: Vec<Vec<f32>>| {
        let _ = write!(s, ",\"{name}\":[");
        for (i, row) in rows.iter().enumerate() {
            s.push_str(if i > 0 { ",[" } else { "[" });
            for (j, v) in row.iter().enumerate() {
                if j > 0 {
                    s.push(',');
                }
                crate::style::push_num(s, *v);
            }
            s.push(']');
        }
        s.push(']');
    };
    if let Some(frags) = fragments {
        let rows = frags
            .iter()
            .map(|g| {
                vec![
                    g.x,
                    g.y,
                    g.width,
                    g.height,
                    g.lines.0 as f32,
                    g.lines.1 as f32,
                    g.dx,
                    g.dy,
                ]
            })
            .collect();
        list(&mut s, "fragments", rows);
    }
    if let Some(c) = columns {
        let rows = c
            .columns
            .iter()
            .map(|c| vec![c.x, c.y, c.width, c.height, f32::from(u8::from(c.holds))])
            .collect();
        list(&mut s, "columns", rows);
    }
    s
}
