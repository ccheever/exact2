//! The heavy list (bench/heavy-list/SPEC.md) on Dioxus, row for row the
//! exact2 port's `app.contract`, with the same CSS (`style.css`).
//!
//! Dioxus has no virtualized list, so `Feed` is the windowed list a Dioxus
//! developer writes: a top spacer, the rows that intersect the viewport plus
//! one viewport of overscan each side, and a bottom spacer, re-windowed on
//! every scroll event. Unmeasured rows count 160 px (exact2's
//! `estimated-item-height`). Blitz fires `onmounted` before layout and has no
//! `onresize` (it panics), so each mounted row's handle is kept and its height
//! read at the next scroll event, before it can leave the window.

mod images;

use dioxus::prelude::*;
use futures_util::FutureExt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const CSS: &str = include_str!("style.css");
const JSON: &[u8] = include_bytes!("messages.json");
const ESTIMATE: f64 = 160.0;

#[derive(serde::Deserialize)]
struct Doc {
    messages: Vec<Msg>,
}

#[derive(serde::Deserialize, Clone)]
struct Run {
    t: String,
    #[serde(default)]
    s: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
struct Photo {
    src: String,
    w: f64,
    h: f64,
}

#[derive(serde::Deserialize, Clone)]
struct Link {
    thumb: String,
    title: String,
    description: String,
    site: String,
}

#[derive(serde::Deserialize, Clone)]
struct Quote {
    author: String,
    excerpt: String,
}

#[derive(serde::Deserialize, Clone)]
struct Reaction {
    emoji: String,
    count: u32,
}

#[derive(serde::Deserialize, Clone)]
struct Msg {
    id: String,
    author: String,
    avatar: String,
    #[serde(rename = "minutesAgo")]
    minutes_ago: f64,
    paragraphs: Vec<Vec<Run>>,
    #[serde(default)]
    photos: Vec<Photo>,
    #[serde(default)]
    link: Option<Link>,
    #[serde(default)]
    quote: Option<Quote>,
    #[serde(default)]
    reactions: Vec<Reaction>,
}

/// A message by identity: a row re-renders only when its record is replaced.
#[derive(Clone)]
struct M(Rc<Msg>);
impl PartialEq for M {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The rows' mounted handles, by message id, for measuring.
#[derive(Clone, Default)]
struct Mounted(Rc<RefCell<HashMap<String, Rc<MountedData>>>>);

fn ago(minutes: f64) -> String {
    if minutes < 60.0 {
        format!("{}m ago", minutes.floor())
    } else if minutes < 1440.0 {
        format!("{}h ago", (minutes / 60.0).floor())
    } else {
        format!("{}d ago", (minutes / 1440.0).floor())
    }
}

fn live_mode() -> bool {
    #[cfg(feature = "web")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|s| s.contains("live=1"))
    }
    #[cfg(not(feature = "web"))]
    {
        std::env::var("BENCH_LIVE").is_ok_and(|v| v == "1")
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut feed = use_signal(|| {
        let doc: Doc = serde_json::from_slice(JSON).expect("messages.json");
        doc.messages.into_iter().map(|m| M(Rc::new(m))).collect::<Vec<_>>()
    });
    // The rows' measured heights, by message id (stable under live inserts).
    let mut heights = use_signal(HashMap::<String, f64>::new);
    let mut secs = use_signal(|| 0.0f64);
    let live = use_hook(live_mode);
    use_context_provider(Mounted::default);

    // SPEC live mode: every 250 ms one message at the top and one reaction
    // bumped; once a second the relative times advance.
    use_future(move || async move {
        if !live {
            return;
        }
        let mut k = 0usize;
        loop {
            futures_timer::Delay::new(Duration::from_millis(250)).await;
            k += 1;
            let mut rows = feed.write();
            let n = rows.len();
            let mut copy = (*rows[(k * 37) % n].0).clone();
            copy.id = format!("live-{k}");
            copy.minutes_ago = 0.0;
            rows.insert(0, M(Rc::new(copy)));
            let mut i = (k * 101) % rows.len();
            while rows[i].0.reactions.is_empty() {
                i = (i + 1) % rows.len();
            }
            let mut bumped = (*rows[i].0).clone();
            bumped.reactions[0].count += 1;
            rows[i] = M(Rc::new(bumped));
            if k % 4 == 0 {
                secs += 1.0;
            }
        }
    });

    let react = move |(id, emoji): (String, String)| {
        let mut rows = feed.write();
        if let Some(i) = rows.iter().position(|m| m.0.id == id) {
            let mut next = (*rows[i].0).clone();
            if let Some(r) = next.reactions.iter_mut().find(|r| r.emoji == emoji) {
                r.count += 1;
            }
            rows[i] = M(Rc::new(next));
        }
    };

    rsx! {
        document::Style { {CSS} }
        div { class: "screen",
            div { class: "bar",
                span { class: "title", "Heavy list" }
                span { class: "live", if live { "Live: on" } else { "Live: off" } }
            }
            Feed { feed, heights, secs: secs(), react }
        }
    }
}

#[component]
fn Feed(
    feed: Signal<Vec<M>>,
    heights: Signal<HashMap<String, f64>>,
    secs: f64,
    react: EventHandler<(String, String)>,
) -> Element {
    let mut top = use_signal(|| 0.0f64);
    let mut viewport = use_signal(|| 1200.0f64);
    let mounted = use_context::<Mounted>();

    let rows = feed.read();
    let hs = heights.read();
    let h = |m: &M| hs.get(&m.0.id).copied().unwrap_or(ESTIMATE);
    let (t, v) = (top(), viewport());
    let mut y = 0.0;
    let mut first = rows.len();
    for (i, m) in rows.iter().enumerate() {
        let mh = h(m);
        if y + mh > t - v {
            first = i;
            break;
        }
        y += mh;
    }
    let pad_top = y;
    let mut last = first;
    while last < rows.len() && y < t + 2.0 * v {
        y += h(&rows[last]);
        last += 1;
    }
    let pad_bottom: f64 = rows[last..].iter().map(h).sum();
    let window: Vec<M> = rows[first..last].to_vec();
    drop(hs);
    drop(rows);

    let measure = move |e: ScrollEvent| {
        // Heights first: a row leaving the window takes its measured height
        // into the spacer, so the rows below it do not move.
        let mut changed = Vec::new();
        for (id, handle) in mounted.0.borrow().iter() {
            if let Some(Ok(rect)) = handle.get_client_rect().now_or_never() {
                let mh = rect.height();
                if mh > 0.0 && (heights.peek().get(id).copied().unwrap_or(ESTIMATE) - mh).abs() > 0.25 {
                    changed.push((id.clone(), mh));
                }
            }
        }
        if !changed.is_empty() {
            let mut hs = heights.write();
            for (id, mh) in changed {
                hs.insert(id, mh);
            }
        }
        top.set(e.scroll_top());
        viewport.set(e.client_height() as f64);
    };

    rsx! {
        div { class: "list", onscroll: measure,
            div { style: "height: {pad_top}px" }
            for m in window {
                Row { key: "{m.0.id}", m: m.clone(), secs, react }
            }
            div { style: "height: {pad_bottom}px" }
        }
    }
}

#[component]
fn Row(m: M, secs: f64, react: EventHandler<(String, String)>) -> Element {
    let mounted = use_context::<Mounted>();
    let id = m.0.id.clone();
    use_drop({
        let (mounted, id) = (mounted.clone(), id.clone());
        move || {
            mounted.0.borrow_mut().remove(&id);
        }
    });
    let msg = m.0.clone();
    let time = ago(msg.minutes_ago + secs / 60.0);
    rsx! {
        div {
            class: "msg",
            aria_label: "{msg.author}, {time}",
            onmounted: move |e: MountedEvent| {
                mounted.0.borrow_mut().insert(id.clone(), e.data());
            },
            div { class: "inner",
                img { class: "avatar", src: images::image(&msg.avatar), decoding: "async" }
                div { class: "content",
                    div { class: "head",
                        span { class: "author", "{msg.author}" }
                        span { class: "time", "{time}" }
                    }
                    if let Some(q) = &msg.quote {
                        div { class: "quote",
                            div { class: "qa", "{q.author}" }
                            div { class: "qe clamp", "{q.excerpt}" }
                        }
                    }
                    div { class: "paras",
                        for (pi, p) in msg.paragraphs.iter().enumerate() {
                            div { key: "{pi}", class: "p",
                                for (ri, r) in p.iter().enumerate() {
                                    span { key: "{ri}", class: r.s.as_deref().unwrap_or(""), "{r.t}" }
                                }
                            }
                        }
                    }
                    Photos { m: m.clone() }
                    if let Some(l) = &msg.link {
                        div { class: "card",
                            img { class: "thumb", src: images::image(&l.thumb), decoding: "async" }
                            div { class: "cbody",
                                div { class: "site", "{l.site}" }
                                div { class: "ct clamp", "{l.title}" }
                                div { class: "cd clamp", "{l.description}" }
                            }
                        }
                    }
                    if !msg.reactions.is_empty() {
                        div { class: "reacts",
                            for r in msg.reactions.iter() {
                                button {
                                    key: "{r.emoji}",
                                    class: "chip",
                                    aria_label: "{r.emoji} {r.count}",
                                    onclick: {
                                        let (id, emoji) = (msg.id.clone(), r.emoji.clone());
                                        move |_| react.call((id.clone(), emoji.clone()))
                                    },
                                    span { class: "emoji", "{r.emoji}" }
                                    span { class: "count", "{r.count}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "sep" }
        }
    }
}

/// 1: the image's own aspect, at most 320 tall; 2: two squares; 3: a 2/3-wide
/// square left and two stacked right; 4: a 2×2 grid.
#[component]
fn Photos(m: M) -> Element {
    let ph = &m.0.photos;
    let img = |p: &Photo, class: &'static str| rsx! { img { class, src: images::image(&p.src), decoding: "async" } };
    match ph.len() {
        0 => rsx! {},
        1 => {
            let ratio = ph[0].w / ph[0].h;
            rsx! {
                div { class: "photos",
                    img { class: "ph1", style: "aspect-ratio: {ratio}", src: images::image(&ph[0].src), decoding: "async" }
                }
            }
        }
        3 => rsx! {
            div { class: "photos",
                div { class: "prow3",
                    {img(&ph[0], "big")}
                    div { class: "right", {img(&ph[1], "half")} {img(&ph[2], "half")} }
                }
            }
        },
        n => rsx! {
            div { class: "photos",
                div { class: "pcol",
                    div { class: "prow", {img(&ph[0], "sq")} {img(&ph[1], "sq")} }
                    if n == 4 {
                        div { class: "prow", {img(&ph[2], "sq")} {img(&ph[3], "sq")} }
                    }
                }
            }
        },
    }
}
