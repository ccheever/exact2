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
    if shell.contains(JS_ENTRY) {
        return page_js(shell, rendered);
    }
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

/// The JavaScript runtime's entry in its shell (`host/web3/build.mjs`).
const JS_ENTRY: &str = "<script type=\"module\" src=\"./app.js\"></script>";

/// The JavaScript runtime's capture script (LLP 1048.001 D5, as
/// `capture()`): presses and edits before activation, replayed after
/// adoption. It also starts the runtime, by the page's policy
/// ([`activate_js`]): an `eager` page's after its first paint, once the
/// document is parsed; an `idle` page's when the browser is idle after
/// `load`; an `interaction` page's at the first press or edit, as every
/// page's at a press that comes first.
pub fn capture_js() -> &'static str {
    include_str!("../../web3/capture.js").trim_end()
}

/// A route's activation on the JavaScript runtime (LLP 1071 D6): what it
/// declared, with an undeclared one `eager`, the default (preloaded from
/// the head, run after first paint).
fn activate_js(activate: exact_plan::ActivatePolicy) -> &'static str {
    match activate {
        exact_plan::ActivatePolicy::Inferred => "eager",
        declared => declared.name(),
    }
}

/// [`page`] over the JavaScript runtime's shell (the exact3 web target,
/// LLP 1071): the same document, head and checkpoint; the shell's title and
/// viewport give way to the head and its capture script, which imports the
/// entry when the page's policy says (the checkpoint takes the entry's
/// place). The shell's `modulepreload`s for the entry and its static
/// imports stay, so the runtime downloads while the document streams; an
/// `interaction` page drops them: it fetches nothing before intent.
fn page_js(shell: &str, rendered: &Rendered) -> Result<String, String> {
    let mut html = shell.to_string();
    let lacks = |what: &str| format!("the JavaScript shell has no `{what}`");
    let lang = rendered
        .document
        .lang
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    let open = html.find("<html").ok_or_else(|| lacks("<html"))?;
    let end = html[open..].find('>').ok_or_else(|| lacks("<html>"))? + open + 1;
    html.replace_range(
        open..end,
        &format!("<html lang=\"{lang}\" dir=\"{}\">", rendered.document.dir),
    );
    let title = html.find("<title>").ok_or_else(|| lacks("<title>"))?;
    let meta = html[title..]
        .find("<meta name=\"viewport\"")
        .ok_or_else(|| lacks("viewport"))?
        + title;
    let stop = html[meta..].find(">\n").ok_or_else(|| lacks("viewport"))? + meta + 2;
    html.replace_range(
        title..stop,
        &format!("{}\n<script>{}</script>\n", rendered.head, capture_js()),
    );
    let activate = activate_js(rendered.activate);
    if activate == "interaction" {
        while let Some(at) = html.find(MODULE_PRELOAD) {
            let stop = html[at..]
                .find(">\n")
                .ok_or_else(|| lacks(MODULE_PRELOAD))?
                + at
                + 2;
            html.replace_range(at..stop, "");
        }
    }
    let root = "<div id=\"exact-root\"></div>";
    let at = html.find(root).ok_or_else(|| lacks(root))?;
    html.replace_range(
        at..at + root.len(),
        &format!("<div id=\"exact-root\">{}</div>", rendered.document.root),
    );
    let at = html.find(JS_ENTRY).ok_or_else(|| lacks(JS_ENTRY))?;
    html.replace_range(
        at..at + JS_ENTRY.len(),
        &format!(
            "<script type=\"application/vnd.exact.checkpoint\" data-digest=\"{}\" data-activate=\"{activate}\">{}</script>",
            rendered.digest, rendered.checkpoint
        ),
    );
    Ok(html)
}

/// A `modulepreload` in the JavaScript shell's head (`host/web3/build.mjs`).
const MODULE_PRELOAD: &str = "<link rel=\"modulepreload\" href=\"";
