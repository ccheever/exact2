//! Extra Heavy feed data (../../SPEC.md): the 3,000 rows of `feed.json` (a byte copy of
//! `../../data/feed.json`, bundled into the binary, never regenerated), the launch config, the
//! app state keyed by row id (a thread's expansion and its comment draft), and the canvas row's
//! drawing (Canvas 2D, LLP 1056: `canvas surface=card(c)` in app.contract calls `draw_2d`).
//!
//! - `feed()` answers every row (baked into the plan at build);
//! - `config()` answers `{ live, freeze, startKey, kinds17 }` from the launch environment (`BENCH_LIVE`,
//!   `BENCH_SCENARIO=rest`, `BENCH_FREEZE`, `BENCH_START_INDEX`);
//! - `draft(id, text)` and `toggle(id, epoch)` change one thread row and answer the feed.
//!
//! Every answer is a fresh list whose items are the previous answer's `Rc` records for every row
//! that did not change (the heavy bench's record sharing), so the list's identity checks hold.
#![forbid(unsafe_code)]



use exact_plan::Value;
use exact_runner::{DataError, DataSource};

mod canvas;
mod md;
mod raw;

use raw::{Doc, Raw};

/// The benchmark's data, as the build found it.
pub const JSON: &[u8] = include_bytes!("../feed.json");

/// The four Lottie files' colours (`../../data/anim/motion-0N.json`, layers ball / square / ring).
pub const LOTTIES: [&[u8]; 4] = [
    include_bytes!("../motion-00.json"),
    include_bytes!("../motion-01.json"),
    include_bytes!("../motion-02.json"),
    include_bytes!("../motion-03.json"),
];

fn s(v: &str) -> Value {
    Value::str(v)
}
fn n(v: f64) -> Value {
    Value::Number(v)
}
fn list<T>(xs: &[T], f: impl Fn(usize, &T) -> Value) -> Value {
    Value::list(xs.iter().enumerate().map(|(i, x)| f(i, x)).collect())
}
fn opt(v: Option<Value>) -> Value {
    match v {
        Some(v) => Value::some(v),
        None => Value::NONE,
    }
}

/// `#rrggbb` of a Lottie colour `[r, g, b, a]` in 0..1.
fn hex(c: &[f64]) -> String {
    let b = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

/// A Lottie file's three shape colours: the ball's fill, the square's fill, the ring's stroke.
fn lottie_colors(json: &[u8]) -> [String; 3] {
    let v: serde_json::Value = serde_json::from_slice(json).expect("lottie json");
    let colour = |layer: usize, ty: &str| -> String {
        let items = &v["layers"][layer]["shapes"][0]["it"];
        let it = items.as_array().unwrap().iter().find(|i| i["ty"] == ty).expect("paint");
        hex(&it["c"]["k"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>())
    };
    [colour(0, "fl"), colour(1, "fl"), colour(2, "st")]
}

/// The declared family (app.contract `font` lines) of a `fonts` key.
fn family(key: &str) -> &'static str {
    match key {
        "bebas" => "Bebas Neue",
        "pacifico" => "Pacifico",
        "dmserif" => "DM Serif Display",
        "abril" => "Abril Fatface",
        "spacemono" => "Space Mono",
        "lobster" => "Lobster",
        "elite" => "Special Elite",
        "marker" => "Permanent Marker",
        "crimson" => "Crimson Text Italic",
        "inter" => "Inter",
        other => panic!("unknown font {other}"),
    }
}

/// `shape Row`'s thread field: 7 common fields, then the kinds in order (thread is the 16th).
const THREAD: usize = 22;

/// One row as `shape Row` (app.contract): the common fields, then one `option` per kind, in
/// `feed.json`'s `kinds` order; exactly one is `some`.
fn row(r: &Raw, fonts: &raw::Fonts, lotties: &[[String; 3]; 4], thread: &ThreadState) -> Value {
    let k = r.kind.as_str();
    let photo = |p: &raw::Photo| (s(&format!("assets/{}", p.src)), n(p.w), n(p.h));
    let mut kinds: Vec<Value> = Vec::with_capacity(19);
    // 1 photo
    kinds.push(opt((k == "photo").then(|| {
        let (src, w, h) = photo(r.photo.as_ref().unwrap());
        Value::record(vec![s(&r.caption), src, w, h])
    })));
    // 2 thumbs
    kinds.push(opt((k == "thumbs").then(|| {
        Value::record(vec![
            s(&r.title),
            // Three rows of four (SPEC 2's 4 × 3 grid).
            list(&r.thumbs.chunks(4).collect::<Vec<_>>(), |i, row| {
                Value::record(vec![
                    s(&i.to_string()),
                    list(row, |j, t| Value::record(vec![s(&j.to_string()), s(&format!("assets/{t}"))])),
                ])
            }),
        ])
    })));
    // 3 shader
    kinds.push(opt((k == "shader").then(|| {
        // The surface asks for the photo by its name under `assets/`.
        let p = r.photo.as_ref().unwrap();
        Value::record(vec![s(&r.caption), s(&p.src), n(p.w), n(p.h), s(&r.duo_a), s(&r.duo_b)])
    })));
    // 4 canvas
    kinds.push(opt((k == "canvas").then(|| {
        Value::record(vec![
            s(&r.title),
            s(&format!("assets/{}", r.image)),
            s(&r.bg),
            s(&r.band[0]),
            s(&r.band[1]),
            list(&r.strokes, |_, st| Value::record(vec![s(&st.color), n(st.width), list(&st.p, |_, x| n(*x))])),
            list(&r.dots, |_, d| Value::record(vec![n(d.x), n(d.y), n(d.r), s(&d.color)])),
        ])
    })));
    // 5 svg
    kinds.push(opt((k == "svg").then(|| {
        Value::record(vec![
            list(&r.icons, |i, name| Value::record(vec![s(&i.to_string()), s(name)])),
            s(&r.chart),
            s(&r.art),
        ])
    })));
    // 6 video
    kinds.push(opt((k == "video").then(|| Value::record(vec![s(&r.caption), s(&format!("assets/{}", r.video))]))));
    // 7 map
    kinds.push(opt((k == "map").then(|| Value::record(vec![s(&r.place), s(&r.address), n(r.lat), n(r.lon)]))));
    // 8 markdown
    kinds.push(opt((k == "markdown").then(|| Value::record(vec![md::blocks(&r.md)]))));
    // 9 code
    kinds.push(opt((k == "code").then(|| {
        let lang = match r.lang.as_str() {
            "ts" => "TS",
            "rs" => "RUST",
            "py" => "PYTHON",
            other => other,
        };
        Value::record(vec![
            s(&r.file),
            s(lang),
            list(&r.lines, |i, toks| {
                Value::record(vec![
                    s(&(i + 1).to_string()),
                    list(toks, |j, t| Value::record(vec![s(&j.to_string()), s(&t.t), s(&t.k)])),
                ])
            }),
        ])
    })));
    // 10 intl
    kinds.push(opt((k == "intl").then(|| {
        Value::record(vec![list(&r.blocks, |i, b| {
            Value::record(vec![s(&i.to_string()), s(&b.text), Value::Bool(b.dir == "rtl")])
        })])
    })));
    // 11 typeface
    kinds.push(opt((k == "typeface").then(|| {
        let spec = &fonts[&r.font];
        Value::record(vec![s(&r.quote), s(&r.by), s(family(&r.font)), s(&spec.family), n(spec.size), s(&r.bg)])
    })));
    // 12 carousel
    kinds.push(opt((k == "carousel").then(|| {
        Value::record(vec![
            s(&r.title),
            list(&r.cards, |i, c| {
                Value::record(vec![s(&i.to_string()), s(&format!("assets/{}", c.image)), s(&c.title), s(&c.meta)])
            }),
        ])
    })));
    // 13 motion
    kinds.push(opt((k == "motion").then(|| {
        let li: usize = r.lottie.trim_start_matches("motion-").trim_end_matches(".json").parse().unwrap();
        let c = &lotties[li];
        Value::record(vec![
            s(&r.caption),
            s(&format!("assets/{}", r.gif)),
            s(&format!("assets/{}", r.webp)),
            // Frame 0 as a still, for BENCH_FREEZE (gen-assets.py).
            s(&format!("assets/still/{}.png", r.gif.trim_end_matches(".gif"))),
            s(&format!("assets/still/{}.png", r.webp.trim_end_matches(".webp"))),
            s(&c[0]),
            s(&c[1]),
            s(&c[2]),
        ])
    })));
    // 14 glass
    kinds.push(opt((k == "glass").then(|| {
        let (src, _, _) = photo(r.photo.as_ref().unwrap());
        Value::record(vec![src, s(&r.title), s(&r.subtitle), s(&r.rating)])
    })));
    // 15 live
    kinds.push(opt((k == "live").then(|| {
        const RING: [&str; 3] = ["#FF3B30", "#34C759", "#007AFF"];
        Value::record(vec![
            s(&r.title),
            n(r.ends_in_sec),
            n(r.updated_sec),
            list(&r.rings, |i, v| Value::record(vec![s(&i.to_string()), n(i as f64), n(*v), s(RING[i])])),
            list(&r.wave, |j, h| Value::record(vec![s(&j.to_string()), n(j as f64), n(*h)])),
        ])
    })));
    // 16 thread: the text, then app state keyed by row id
    kinds.push(opt((k == "thread").then(|| thread.value(&r.text))));
    // 17 webview
    kinds.push(opt((k == "webview").then(|| {
        // The page, filled in per row by gen-assets.py (exact2's `iframe` has no `srcdoc`):
        // `assets/embed/<id>.html`, and `<id>-paused.html` for BENCH_FREEZE.
        Value::record(vec![s(&r.caption), s(&format!("assets/embed/{}", r.id))])
    })));
    // 18 filmstrip, 19 inbox: the nested virtualized lists (LLP 1070, not built yet). The row
    // carries what the inner list will need; the app draws the header, title and the box.
    kinds.push(opt((k == "filmstrip").then(|| {
        Value::record(vec![s(&r.title), n(r.count), n(r.img0), n(r.img_step), n(r.num0)])
    })));
    kinds.push(opt((k == "inbox").then(|| {
        Value::record(vec![s(&r.title), n(r.count), n(r.m0), n(r.m_step), n(r.clock0)])
    })));
    let mut f = vec![
        s(&r.id),
        n(r.index as f64),
        s(&r.kind),
        s(&r.author),
        s(&r.handle),
        s(&format!("assets/{}", r.avatar)),
        n(r.minutes_ago),
    ];
    f.extend(kinds);
    debug_assert_eq!(f.len(), 26);
    Value::record(f)
}

/// A thread row's app state (SPEC 16): expanded, the flip epoch of the last tap (−1: never),
/// and the comment draft.
#[derive(Clone, Default)]
struct ThreadState {
    expanded: bool,
    at: f64,
    draft: String,
}

impl ThreadState {
    fn fresh() -> ThreadState {
        ThreadState { expanded: false, at: -1.0, draft: String::new() }
    }
    fn value(&self, text: &str) -> Value {
        Value::record(vec![s(text), Value::Bool(self.expanded), n(self.at), s(&self.draft)])
    }
}

struct Feed {
    /// What a row's `Value` does not answer cheaply: its id and kind, and a thread row's text
    /// (to rebuild its state). The parsed `Raw`s are dropped once the `Value`s exist, so the source
    /// keeps one copy of the feed, as SwiftUI's app keeps its `[Row]`.
    meta: Vec<Meta>,
    rows: Vec<Value>,
    threads: std::collections::HashMap<usize, ThreadState>,
    /// What every filmstrip / inbox row's nested list iterates (SPEC 18, 19): the item indices
    /// 0‥count−1, one shared list each, and the inbox's message pool. A row computes its item's
    /// image, caption, message and time from its own parameters.
    strip: Value,
    mail: Value,
    pool: Value,
}

fn indices(count: usize) -> Value {
    Value::list((0..count).map(|j| Value::record(vec![s(&j.to_string()), n(j as f64)])).collect())
}

impl Feed {
    fn load(doc: Doc) -> Feed {
        let lotties = LOTTIES.map(lottie_colors);
        let fresh = ThreadState::fresh();
        let rows = doc.rows.iter().map(|r| row(r, &doc.fonts, &lotties, &fresh)).collect();
        let pool = Value::list(
            doc.messages
                .iter()
                .enumerate()
                .map(|(i, m)| Value::record(vec![n(i as f64), s(&m.name), s(&format!("assets/{}", m.avatar)), s(&m.text)]))
                .collect(),
        );
        let meta = doc
            .rows
            .into_iter()
            .map(|r| Meta { text: if r.kind == "thread" { r.text } else { String::new() }, id: r.id, kind: r.kind })
            .collect();
        Feed { meta, rows, threads: Default::default(), strip: indices(2000), mail: indices(1000), pool }
    }

    fn value(&self) -> Value {
        let rows: Vec<Value> = match bench_kinds().as_deref() {
            None => self.rows.clone(),
            // BENCH_KINDS=17: without the nested-list kinds (filmstrip, inbox), as every stack does.
            Some("17") => self.rows.iter().zip(&self.meta).filter(|(_, r)| !nested(r)).map(|(v, _)| v.clone()).collect(),
            // BENCH_KINDS=<kind>[,<kind>…] (the per-kind breakdown, as the SwiftUI and Expo apps do):
            // only those kinds, the 17-kind feed's row count, their rows in feed order cycled (a
            // repeat's id gets `-<cycle>`).
            Some(k) => {
                let want: Vec<&str> = k.split(',').map(str::trim).collect();
                let pick: Vec<usize> = (0..self.meta.len()).filter(|&i| want.contains(&self.meta[i].kind.as_str())).collect();
                let total = self.meta.iter().filter(|r| !nested(r)).count();
                if pick.is_empty() {
                    Vec::new()
                } else {
                    (0..total)
                        .map(|i| {
                            let (j, cycle) = (pick[i % pick.len()], i / pick.len());
                            if cycle == 0 {
                                return self.rows[j].clone();
                            }
                            let Value::Record(fields) = &self.rows[j] else { unreachable!() };
                            let mut fields = fields.to_vec();
                            fields[0] = s(&format!("{}-{cycle}", self.meta[j].id));
                            Value::record(fields)
                        })
                        .collect()
                }
            }
        };
        Value::record(vec![Value::list(rows), self.strip.clone(), self.mail.clone(), self.pool.clone()])
    }

    /// The key of the row at list position n (BENCH_START_INDEX), counted as the list shows it.
    fn key_at(&self, at: usize) -> String {
        let k17 = kinds17();
        self.meta.iter().filter(|r| !k17 || !nested(r)).nth(at).map(|r| r.id.clone()).unwrap_or_default()
    }

    fn at(&self, id: &str) -> Option<usize> {
        let i: usize = id.strip_prefix('r')?.parse().ok()?;
        (i < self.meta.len() && self.meta[i].id == id && self.meta[i].kind == "thread").then_some(i)
    }

    fn edit(&mut self, id: &str, f: impl FnOnce(&mut ThreadState)) {
        let Some(i) = self.at(id) else { return };
        let st = self.threads.entry(i).or_insert_with(ThreadState::fresh);
        f(st);
        let v = st.value(&self.meta[i].text);
        let Value::Record(fields) = &self.rows[i] else { unreachable!() };
        let mut fields = fields.to_vec();
        fields[THREAD] = Value::some(v);
        self.rows[i] = Value::record(fields);
    }
}

struct Meta {
    id: String,
    kind: String,
    text: String,
}

fn nested(r: &Meta) -> bool {
    r.kind == "filmstrip" || r.kind == "inbox"
}

/// The source. The list first shows the plan's baked value; the source's own copy (what a draft,
/// a tap edits) is parsed on a background thread started at launch.
pub struct XHeavy {
    #[cfg(not(target_arch = "wasm32"))]
    parsed: Option<std::thread::JoinHandle<Doc>>,
    /// The web build (../web): wasm32-unknown-unknown has no threads, so the feed is
    /// parsed when it is first asked for, on the page's thread.
    #[cfg(target_arch = "wasm32")]
    parsed: Option<()>,
    feed: Option<Feed>,
    /// Built for the bake: not ready, so no feed is compiled into the plan and the launch asks for
    /// it (filtered by BENCH_KINDS) before its first frame, as SwiftUI's app filters before its own.
    baking: bool,
}

impl Default for XHeavy {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let parsed = std::thread::spawn(|| serde_json::from_slice::<Doc>(JSON).expect("feed.json"));
        #[cfg(target_arch = "wasm32")]
        let parsed = ();
        XHeavy { parsed: Some(parsed), feed: None, baking: false }
    }
}

impl XHeavy {
    /// The source `apple/build.rs` bakes with: answers nothing at build.
    pub fn for_bake() -> Self {
        XHeavy { parsed: None, feed: None, baking: true }
    }
}

impl XHeavy {
    fn feed(&mut self) -> &mut Feed {
        if self.feed.is_none() {
            #[cfg(not(target_arch = "wasm32"))]
            let doc = self.parsed.take().expect("parsed once").join().expect("parse thread");
            #[cfg(target_arch = "wasm32")]
            let doc = {
                self.parsed.take().expect("parsed once");
                serde_json::from_slice::<Doc>(JSON).expect("feed.json")
            };
            self.feed = Some(Feed::load(doc));
        }
        self.feed.as_mut().expect("loaded")
    }
}

/// BENCH_KINDS in the launch environment (`17`, or a kind list), when set.
fn bench_kinds() -> Option<String> {
    std::env::var("BENCH_KINDS").ok().filter(|v| !v.is_empty())
}

/// Any BENCH_KINDS: the feed is filtered (the source answers it at launch);
/// `17` also leaves the nested kinds out of BENCH_START_INDEX's count.
fn kinds17() -> bool {
    bench_kinds().is_some()
}

fn env_is(name: &str, value: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == value)
}

fn arg_str<'a>(args: &'a [Value], i: usize) -> Result<&'a str, DataError> {
    args.get(i).and_then(Value::as_str).ok_or_else(|| DataError::BadArguments(format!("argument {i}")))
}

impl DataSource for XHeavy {
    fn app_id(&self) -> &str {
        "dev.exact.xheavy.exact2"
    }

    fn ready(&self) -> bool {
        !self.baking
    }

    /// The canvas row (SPEC 4) is a Canvas 2D surface.
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        vec![("card".into(), 1)]
    }

    fn draw_2d(
        &mut self,
        surface: &str,
        args: &[Value],
        ctx: &exact_runner::exact_canvas::Context2d,
        frame: &exact_runner::exact_canvas::Frame,
    ) -> Result<bool, exact_runner::exact_canvas::DrawError> {
        match surface {
            "card" => canvas::draw(args, ctx, frame),
            other => Err(format!("no 2D surface `{other}`").into()),
        }
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("feed", []) => Ok(self.feed().value()),
            ("config", []) => {
                let freeze = env_is("BENCH_FREEZE", "1");
                let live = !freeze && (env_is("BENCH_LIVE", "1") || env_is("BENCH_SCENARIO", "rest"));
                let start = std::env::var("BENCH_START_INDEX").ok().and_then(|v| v.parse::<usize>().ok());
                let key = match start {
                    Some(i) if i > 0 => self.feed().key_at(i),
                    _ => String::new(),
                };
                Ok(Value::record(vec![Value::Bool(live), Value::Bool(freeze), s(&key), Value::Bool(kinds17())]))
            }
            ("draft", [_, _]) => {
                let (id, text) = (arg_str(args, 0)?.to_string(), arg_str(args, 1)?.to_string());
                let f = self.feed();
                f.edit(&id, |st| st.draft = text);
                Ok(f.value())
            }
            ("toggle", [_, Value::Number(epoch), Value::Bool(now)]) => {
                let id = arg_str(args, 0)?.to_string();
                let (epoch, now) = (*epoch, *now);
                let f = self.feed();
                f.edit(&id, |st| {
                    st.expanded = !now;
                    st.at = epoch;
                });
                Ok(f.value())
            }
            ("feed" | "config", _) => Err(DataError::BadArguments(format!("{source}()"))),
            ("draft", _) => Err(DataError::BadArguments("draft(id: string, text: string)".into())),
            ("toggle", _) => Err(DataError::BadArguments("toggle(id: string, epoch: number, expanded: bool)".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &Value) -> Vec<Value> {
        match v {
            Value::Record(f) => match &f[0] {
                Value::List(l) => l.to_vec(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn feed_and_state() {
        let mut x = XHeavy::default();
        let a = rows(&x.query("feed", &[]).unwrap());
        assert_eq!(a.len(), 3000);
        let b = rows(&x.query("draft", &[Value::str("r18"), Value::str("hi")]).unwrap());
        let changed = a.iter().zip(&b).filter(|(p, q)| !exact_runner::compare::same(p, q)).count();
        assert_eq!(changed, 1);
        assert_eq!(x.feed().key_at(500), "r500");
        assert_eq!(lottie_colors(LOTTIES[0])[0].len(), 7);
    }
}
