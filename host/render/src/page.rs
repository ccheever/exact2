//! A rendered document as a page (LLP 1048.000 D3, D6, D7): the one place a
//! page is composed, for the build's `dist/` and the server alike.
use crate::Rendered;

/// The capture script (LLP 1048.000 D6, 1048.001 D5): host code, inline in
/// every document's head, that records presses — a click, or Enter or Space
/// on an element that hears `press` and isn't a native control — against
/// their elements from first parse, until the boot glue takes them over and
/// replays them once after adoption. Once the page has painted (its first
/// paint entry), a page that doesn't activate on interaction hands its wasm's
/// download, which the head's preload began, to the glue (`exact.runtime`).
/// A navigation that leaves the document stops that download, and removes the
/// preload, which would otherwise keep it going: the next document needs the
/// link. It holds no app logic. The
/// server's CSP admits it by hash; `scripts/boot.mjs` pins that hash and its
/// size.
pub fn capture() -> &'static str {
    include_str!("../../web/capture.js").trim_end()
}

/// The built shell with the renderer's `<head>` and the capture script in
/// place of its title and viewport meta, the document in `#exact-root`, and
/// its checkpoint, with the document's digest, before the glue. An idle
/// page keeps the shell's wasm and `navigation.js` preloads, so its runtime
/// downloads with the document, where a CDN would send them as a 103 (D3, as
/// built); an interaction page's are removed: it fetches nothing before a
/// handler's intent. Refuses a shell that no longer has those places.
pub fn page(shell: &str, rendered: &Rendered) -> Result<String, String> {
    let mut html = shell.to_string();
    // From `start` through the first `end` after it.
    let cut = |html: &mut String, start: &str, end: &str, with: &str| -> Result<(), String> {
        let lacks = || format!("the shell has no `{start}`…`{end}`");
        let at = html.find(start).ok_or_else(lacks)?;
        let stop = html[at..].find(end).ok_or_else(lacks)? + at + end.len();
        html.replace_range(at..stop, with);
        Ok(())
    };
    let lang = rendered
        .document
        .lang
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    cut(
        &mut html,
        "<html",
        ">",
        &format!("<html lang=\"{lang}\" dir=\"{}\">", rendered.document.dir),
    )?;
    // The shell's viewport meta goes before the head, which has its own.
    cut(&mut html, "<meta name=\"viewport\"", ">\n", "")?;
    cut(
        &mut html,
        "<title>",
        "</title>\n",
        &format!("{}\n<script>{}</script>\n", rendered.head, capture()),
    )?;
    cut(&mut html, "<!-- Fetched in parallel", "-->\n", "")?;
    let interaction = rendered.activate == exact_plan::ActivatePolicy::Interaction;
    // The wasm's URL names its build (`./app.wasm?v=…`, build.mjs). An
    // interaction page drops the preload, so its checkpoint names the build
    // for document-glue.js.
    let navigation = "<link rel=\"modulepreload\" href=\"./navigation.js\">\n";
    let preload = "<link rel=\"preload\" href=\"";
    let wasm = &format!("{preload}./app.wasm");
    let named = html.find(wasm).and_then(|at| {
        let from = at + preload.len();
        Some(&html[from..from + html[from..].find('"')?])
    });
    let build = match named {
        Some(href) if interaction => format!(" data-wasm=\"{href}\""),
        _ => String::new(),
    };
    if interaction {
        cut(&mut html, navigation, navigation, "")?;
        cut(&mut html, wasm, ">\n", "")?;
    }
    let root = "<div id=\"exact-root\"></div>";
    let document = format!("<div id=\"exact-root\">{}</div>", rendered.document.root);
    cut(&mut html, root, root, &document)?;
    let glue = "<script type=\"module\" src=\"./glue.js\"></script>";
    let entry = if interaction {
        "<script type=\"module\" src=\"./document-glue.js\"></script>"
    } else {
        glue
    };
    let checkpoint = format!(
        "<script type=\"application/vnd.exact.checkpoint\" data-digest=\"{}\" data-activate=\"{}\"{build}>{}</script>\n{entry}",
        rendered.digest, rendered.activate.name(), rendered.checkpoint
    );
    cut(&mut html, glue, glue, &checkpoint)?;
    let viewports = html.matches("<meta name=\"viewport\"").count();
    let preloads = ["app.wasm", "navigation.js", "glue.js"]
        .iter()
        .filter(|file| html.contains(&format!("preload\" href=\"./{file}")))
        .count();
    let expected = if interaction { 0 } else { 2 };
    if viewports != 1 || preloads != expected || !html.contains(&rendered.head) {
        return Err("the shell no longer has the places a document goes".into());
    }
    Ok(html)
}
