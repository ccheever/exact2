//! The differential check (LLP 1048.004 D6): a location rendered with a
//! kernel and without one, compared byte for byte — the document's root,
//! head fields, checkpoint and activation, and with a JavaScript shell the
//! page as the server streams it (its digest aside, which the kernel-free
//! page takes over the runtime's form of the root).

use crate::page::{body_js, Js};
use crate::{render_with, Ids, Projection, Rendered, Settled};
use exact_plan::Plan;
use exact_runner::DataSource;
use exact_web::document::{route_at, Site};
use std::time::Duration;

/// `location` rendered both ways: the JSON fields `<app>-render --compare`
/// prints after the location — `same`, and the first difference
/// (`differs`), or why the page has no kernel-free render (`direct`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn location<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    shell: Option<&str>,
) -> String {
    let render = |projection| {
        render_with(
            plan,
            data,
            viewport,
            location,
            site,
            deadline,
            Ids::Any,
            projection,
        )
    };
    let kernel = match render(Projection::Kernel) {
        Ok(kernel) => kernel,
        Err(e) => return format!(",\"same\":null,\"error\":{}", json(&e)),
    };
    let direct = match render(Projection::Direct) {
        Ok(direct) => direct,
        Err(why) => return format!(",\"same\":null,\"direct\":{}", json(&why)),
    };
    if let Some(differs) = documents(&kernel, &direct) {
        return format!(",\"same\":false,\"differs\":{}", json(&differs));
    }
    let js = shell.and_then(|shell| crate::page::is_js(shell).then_some(shell));
    if let Some(shell) = js {
        match streamed(
            plan, data, viewport, location, site, deadline, shell, &kernel,
        ) {
            Ok(None) => {}
            Ok(Some(differs)) => return format!(",\"same\":false,\"differs\":{}", json(&differs)),
            Err(e) => return format!(",\"same\":false,\"differs\":{}", json(&e)),
        }
    }
    let settled = kernel.settled == Settled::Complete && direct.settled == Settled::Complete;
    format!(",\"same\":true,\"settled\":{settled}")
}

/// The first difference between two renders' documents.
fn documents(kernel: &Rendered, direct: &Rendered) -> Option<String> {
    let (k, d) = (&kernel.document, &direct.document);
    // View ids follow the order answers landed in, which two renders need
    // not share; nobody keeps them on these pages ([`Ids::Any`]).
    let (kr, dr) = (without_ids(&k.root), without_ids(&d.root));
    // The clock the page read, which two renders read at different times.
    let (kc, dc) = (
        without_clock(&kernel.checkpoint),
        without_clock(&direct.checkpoint),
    );
    let fields: [(&str, &str, &str); 6] = [
        ("root", &kr, &dr),
        ("head", &kernel.head, &direct.head),
        ("checkpoint", &kc, &dc),
        ("keyframes", &k.keyframes, &d.keyframes),
        ("lang", &k.lang, &d.lang),
        ("dir", &k.dir, &d.dir),
    ];
    for (field, a, b) in fields {
        if let Some(at) = first_difference(a, b) {
            return Some(format!("{field} at byte {at}: {}", around(a, b, at)));
        }
    }
    if k.scroll_document != d.scroll_document {
        return Some(format!(
            "scroll_document: {} against {}",
            k.scroll_document, d.scroll_document
        ));
    }
    if kernel.activate != direct.activate {
        return Some(format!(
            "activate: {} against {}",
            kernel.activate.name(),
            direct.activate.name()
        ));
    }
    (k.viewport_fit != d.viewport_fit || k.interactive_widget != d.interactive_widget)
        .then(|| "the first root's viewport policies".to_string())
}

/// The page after a flushed head, as the server streams it without a
/// kernel, against the kernel render's (`kernel`), digests aside: `None`
/// when the same, or when the page isn't streamed.
#[allow(clippy::too_many_arguments)]
fn streamed<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    shell: &str,
    kernel: &Rendered,
) -> Result<Option<String>, String> {
    let js = Js::of(shell)?;
    let preload = route_at(plan, location)
        .is_none_or(|route| route.activate != exact_plan::ActivatePolicy::Interaction);
    let late = |activate: exact_plan::ActivatePolicy| {
        !preload && crate::page::activate_js(activate) != "interaction"
    };
    let mut sent = Vec::new();
    let mut send = |bytes: &[u8]| sent.extend_from_slice(bytes);
    let Some((_, body)) = crate::direct::render_js(
        &std::sync::Arc::new(plan.clone()),
        data,
        viewport,
        location,
        site,
        deadline,
        shell,
        &js,
        late,
        usize::MAX,
        &mut send,
    )?
    else {
        return Ok(None);
    };
    if sent != body.as_bytes() {
        return Ok(Some("the streamed runs are not the page".into()));
    }
    let expected = body_js(shell, &js, kernel, late(kernel.activate), false);
    let (a, b) = (
        without_clock(&without_digest(&expected)),
        without_clock(&without_digest(&body)),
    );
    Ok(first_difference(&a, &b)
        .map(|at| format!("streamed page at byte {at}: {}", around(&a, &b, at))))
}

/// `text` with the checkpoint's clock (`"time":<ms>`) blanked.
fn without_clock(text: &str) -> String {
    const TIME: &str = "\"time\":";
    match text.find(TIME) {
        Some(at) => {
            let from = at + TIME.len();
            let to = text[from..]
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
                .map_or(text.len(), |end| from + end);
            format!("{}{}", &text[..from], &text[to..])
        }
        None => text.to_string(),
    }
}

/// `root` with its view ids blanked.
fn without_ids(root: &str) -> String {
    const VIEW: &str = "data-view=\"";
    let mut out = String::with_capacity(root.len());
    let mut rest = root;
    while let Some(at) = rest.find(VIEW) {
        out.push_str(&rest[..at + VIEW.len()]);
        rest = &rest[at + VIEW.len()..];
        let end = rest.find('"').unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `page` with its checkpoint's digest blanked.
fn without_digest(page: &str) -> String {
    const DIGEST: &str = "data-digest=\"";
    match page.find(DIGEST) {
        Some(at) => {
            let from = at + DIGEST.len();
            let to = page[from..].find('"').map_or(page.len(), |end| from + end);
            format!("{}{}", &page[..from], &page[to..])
        }
        None => page.to_string(),
    }
}

fn first_difference(a: &str, b: &str) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(
        a.bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len())),
    )
}

/// The text around byte `at` in each.
fn around(a: &str, b: &str, at: usize) -> String {
    let cut = |s: &str| {
        let mut from = at.saturating_sub(60).min(s.len());
        while !s.is_char_boundary(from) {
            from -= 1;
        }
        let mut to = (at + 60).min(s.len());
        while !s.is_char_boundary(to) {
            to += 1;
        }
        s[from..to].to_string()
    };
    format!("kernel «{}» direct «{}»", cut(a), cut(b))
}

fn json(text: &str) -> String {
    serde_json::Value::String(text.into()).to_string()
}
