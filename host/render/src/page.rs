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
        &format!(
            "<html lang=\"{lang}\" dir=\"{}\"{}>",
            rendered.document.dir,
            scroll_attr(rendered.document.scroll_document)
        ),
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

/// The JavaScript runtime's entry in its shell (`host/web-js/build.mjs`).
const JS_ENTRY: &str = "<script type=\"module\" src=\"./app.js\"></script>";

/// The JavaScript runtime's capture script (LLP 1048.001 D5, as
/// `capture()`): presses and edits before activation, replayed after
/// adoption. It also starts the runtime, by the page's policy
/// ([`activate_js`]): an `eager` page's after its first paint, once the
/// document is parsed; an `idle` page's when the browser is idle after
/// `load`; an `interaction` page's at the first press or edit, as every
/// page's at a press that comes first.
pub fn capture_js() -> &'static str {
    include_str!("../../web-js/capture.js").trim_end()
}

/// `<html data-scrolldocument>` when an element is the page's scroller
/// (LLP 1048.003 D4): the shell's rule reads the root, not a `:has()` over
/// the whole tree at every style recalculation.
fn scroll_attr(on: bool) -> &'static str {
    if on {
        " data-scrolldocument"
    } else {
        ""
    }
}

/// The mark a streamed page sets once its render finds the page's scroller,
/// its `<html>` having gone before (the only other inline script; the CSP
/// admits its bytes).
pub fn scroll_document_js() -> &'static str {
    "document.documentElement.setAttribute(\"data-scrolldocument\",\"\")"
}

/// A route's activation on the JavaScript runtime (LLP 1071 D6): what it
/// declared, with an undeclared one `eager`, the default (preloaded from
/// the head, run after first paint).
pub(crate) fn activate_js(activate: exact_plan::ActivatePolicy) -> &'static str {
    match activate {
        exact_plan::ActivatePolicy::Inferred => "eager",
        declared => declared.name(),
    }
}

/// [`page`] over the JavaScript runtime's shell (exact2's JS web target,
/// LLP 1071): [`head_js`], then [`body_js`] — the page a server streams in
/// those two parts is these bytes.
fn page_js(shell: &str, rendered: &Rendered) -> Result<String, String> {
    let preload = activate_js(rendered.activate) != "interaction";
    let document = &rendered.document;
    let scroll = document.scroll_document;
    Ok(
        head_js(shell, &document.lang, &document.dir, preload, scroll)?
            + &body_js(shell, rendered, false, scroll)?,
    )
}

/// Whether `shell` is the JavaScript runtime's, whose pages a server can
/// send in two parts ([`head_js`], [`body_js`]).
pub(crate) fn is_js(shell: &str) -> bool {
    shell.contains(JS_ENTRY)
}

/// The shell's places, in its order: `<html…>`, `<title>` through the
/// viewport meta's line, the root, the entry.
struct Places {
    html: (usize, usize),
    title: (usize, usize),
    root: usize,
    entry: usize,
}

fn places(shell: &str) -> Result<Places, String> {
    let lacks = |what: &str| format!("the JavaScript shell has no `{what}`");
    let open = shell.find("<html").ok_or_else(|| lacks("<html"))?;
    let end = shell[open..].find('>').ok_or_else(|| lacks("<html>"))? + open + 1;
    let title = shell.find("<title>").ok_or_else(|| lacks("<title>"))?;
    let meta = shell[title..]
        .find("<meta name=\"viewport\"")
        .ok_or_else(|| lacks("viewport"))?
        + title;
    let stop = shell[meta..].find(">\n").ok_or_else(|| lacks("viewport"))? + meta + 2;
    let root = shell.find(ROOT).ok_or_else(|| lacks(ROOT))?;
    let entry = shell.find(JS_ENTRY).ok_or_else(|| lacks(JS_ENTRY))?;
    if !(end <= title && stop <= root && root + ROOT.len() <= entry) {
        return Err("the JavaScript shell's places are out of order".into());
    }
    Ok(Places {
        html: (open, end),
        title: (title, stop),
        root,
        entry,
    })
}

/// The shell's `modulepreload` lines between the viewport meta and the root.
fn preloads(shell: &str, places: &Places) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = places.title.1;
    while let Some(at) = shell[from..places.root].find(MODULE_PRELOAD) {
        let at = at + from;
        let Some(stop) = shell[at..places.root].find(">\n") else {
            break;
        };
        found.push((at, at + stop + 2));
        from = at + stop + 2;
    }
    found
}

/// What of a JavaScript page comes before anything its render decides: the
/// doctype and `<html lang dir>`, the charset and base, the capture script,
/// the entry's `modulepreload`s when `preload` (an `interaction` page has
/// none: it fetches nothing before intent) and the stylesheet. A server
/// sends it as a request arrives (LLP 1071 D6's early flush), while the
/// page's data is still being asked; the head stays open, so what the
/// render decides — the title, the metas, a late preload — still lands in
/// `<head>`.
pub(crate) fn head_js(
    shell: &str,
    lang: &str,
    dir: &str,
    preload: bool,
    scroll: bool,
) -> Result<String, String> {
    let at = places(shell)?;
    let lang = lang
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    let mut out = String::with_capacity(at.root + 1024);
    out.push_str(&shell[..at.html.0]);
    let _ = std::fmt::Write::write_fmt(
        &mut out,
        format_args!(
            "<html lang=\"{lang}\" dir=\"{dir}\"{}>",
            scroll_attr(scroll)
        ),
    );
    out.push_str(&shell[at.html.1..at.title.0]);
    out.push_str("<script>");
    out.push_str(capture_js());
    out.push_str("</script>\n");
    let mut from = at.title.1;
    for (start, stop) in preloads(shell, &at) {
        out.push_str(&shell[from..start]);
        if preload {
            out.push_str(&shell[start..stop]);
        }
        from = stop;
    }
    out.push_str(&shell[from..at.root]);
    Ok(out)
}

/// The rest of a JavaScript page, once its render is done: the head's
/// fields, the entry's `modulepreload`s when `late` (a page sent before its
/// render turned an `interaction` route `eager`), the document in
/// `#exact-root`, and its checkpoint in the entry's place, which the capture
/// script finds. The document goes as the runtime adopts it
/// ([`for_runtime`]).
pub(crate) fn body_js(
    shell: &str,
    rendered: &Rendered,
    late: bool,
    marked: bool,
) -> Result<String, String> {
    let at = places(shell)?;
    let mut out =
        String::with_capacity(rendered.document.root.len() + rendered.checkpoint.len() + 1024);
    out.push_str(&without_fonts(shell, &rendered.head));
    out.push('\n');
    // A head sent before the render could not mark `<html>` (`scroll_attr`).
    if rendered.document.scroll_document && !marked {
        out.push_str("<script>");
        out.push_str(scroll_document_js());
        out.push_str("</script>\n");
    }
    if late {
        for (start, stop) in preloads(shell, &at) {
            out.push_str(&shell[start..stop]);
        }
    }
    out.push_str("<div id=\"exact-root\">");
    out.push_str(&for_runtime(&rendered.document.root, &classes(shell)));
    out.push_str("</div>");
    out.push_str(&shell[at.root + ROOT.len()..at.entry]);
    let _ = std::fmt::Write::write_fmt(
        &mut out,
        format_args!(
            "<script type=\"application/vnd.exact.checkpoint\" data-digest=\"{}\" data-activate=\"{}\">{}</script>",
            rendered.digest,
            activate_js(rendered.activate),
            rendered.checkpoint
        ),
    );
    out.push_str(&shell[at.entry + JS_ENTRY.len()..]);
    Ok(out)
}

/// The head's fields without the plan's fonts when the shell declares them
/// (`host/web-js/build.mjs`: its stylesheet's faces, with their
/// `font-display`, and their preloads early in the head): a second rule for
/// a face would replace the shell's.
fn without_fonts(shell: &str, head: &str) -> String {
    const PRELOAD: &str = "<link rel=\"preload\" href=\"";
    if !shell.contains("as=\"font\"") {
        return head.to_string();
    }
    let mut out = head.to_string();
    if let Some(at) = out.find("<style>@font-face") {
        if let Some(end) = out[at..].find("</style>") {
            out.replace_range(at..at + end + "</style>".len(), "");
        }
    }
    let mut from = 0;
    while let Some(at) = out[from..].find(PRELOAD).map(|at| at + from) {
        let Some(end) = out[at..].find('>').map(|end| at + end + 1) else {
            break;
        };
        if out[at..end].contains("as=\"font\"") {
            out.replace_range(at..end, "");
            from = at;
        } else {
            from = end;
        }
    }
    out
}

/// The document's element in the shell.
const ROOT: &str = "<div id=\"exact-root\"></div>";

/// The shell stylesheet's static classes (`host/web-js/build.mjs`: `.c<n>{…}`
/// inside `#exact-root#exact-root{…}`), by their CSS text.
fn classes(shell: &str) -> std::collections::HashMap<&str, &str> {
    let mut found = std::collections::HashMap::new();
    let Some(start) = shell.find("#exact-root#exact-root{") else {
        return found;
    };
    let mut rest = &shell[start + "#exact-root#exact-root{".len()..];
    while let Some(tail) = rest.strip_prefix(".c") {
        let Some(open) = tail.find('{') else { break };
        let Some(close) = tail[open..].find('}') else {
            break;
        };
        let (name, css) = (&tail[..open], &tail[open + 1..open + close]);
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        // `c<n>`, from the `.c<n>{` at the head of `rest`.
        found.entry(css).or_insert(&rest[1..2 + open]);
        rest = &tail[open + close + 1..];
    }
    found
}

/// The render host's document as the JavaScript runtime adopts it: an
/// element whose inline style is one of the stylesheet's static classes
/// carries that class instead (the runtime gives it that class at
/// adoption, and drops the inline style), and no element carries the view
/// ids only the wasm runtime reads — a link keeps an empty `data-view`, as
/// the runtime's own links have, which the shell's link rules select. Adoption walks tags, so neither changes
/// what it claims; the page paints the same before and after (the class's
/// CSS is the style's, at a specificity above every rule of the shell's
/// but `!important` ones, as an inline style's is). On RealWorld's `/` the
/// two took about 1.4 KB off the page (brotli, as sent); it paints
/// pixel-for-pixel as before, with and without JavaScript.
fn for_runtime(root: &str, classes: &std::collections::HashMap<&str, &str>) -> String {
    let mut out = String::with_capacity(root.len());
    let mut rest = root;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        // A comment ends at `-->`; any other tag at its first `>` (the
        // document escapes `>` and `"` in attribute values).
        let end = if rest.starts_with("<!--") {
            rest.find("-->").map(|at| at + 3)
        } else {
            rest.find('>').map(|at| at + 1)
        };
        let Some(end) = end else { break };
        let tag = &rest[..end];
        rest = &rest[end..];
        if tag.starts_with("</") || tag.starts_with("<!") {
            out.push_str(tag);
            continue;
        }
        out.push_str(&start_tag(tag, classes));
    }
    out.push_str(rest);
    out
}

/// One start tag, as [`for_runtime`] writes it.
fn start_tag(tag: &str, classes: &std::collections::HashMap<&str, &str>) -> String {
    let body = tag.trim_start_matches('<').trim_end_matches('>');
    let (body, close) = match body.strip_suffix('/') {
        Some(body) => (body, "/"),
        None => (body, ""),
    };
    let name_end = body.find([' ', '\t', '\n']).unwrap_or(body.len());
    let mut attrs: Vec<(&str, Option<&str>)> = Vec::new();
    let mut rest = &body[name_end..];
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let stop = rest.find(['=', ' ', '\t', '\n']).unwrap_or(rest.len());
        let name = &rest[..stop];
        rest = &rest[stop..];
        if let Some(value) = rest.strip_prefix("=\"") {
            let Some(q) = value.find('"') else {
                return tag.to_string();
            };
            attrs.push((name, Some(&value[..q])));
            rest = &value[q + 1..];
        } else if rest.starts_with('=') {
            // An unquoted value: not what the document writes.
            return tag.to_string();
        } else {
            attrs.push((name, None));
        }
    }
    let has_class = attrs.iter().any(|(name, _)| *name == "class");
    let mut out = String::with_capacity(tag.len());
    out.push('<');
    out.push_str(&body[..name_end]);
    for (name, value) in attrs {
        let class = match (name, value) {
            // The shell styles a link by `a[data-view]` (the runtime's
            // links carry an empty one); no other element needs its id.
            ("data-view", _) if &body[..name_end] == "a" => {
                out.push_str(" data-view");
                continue;
            }
            ("data-view", _) => continue,
            ("style", Some(css)) if !has_class => classes.get(unescape(css).as_ref()).copied(),
            _ => None,
        };
        out.push(' ');
        match (class, value) {
            (Some(class), _) => {
                out.push_str("class=\"");
                out.push_str(class);
                out.push('"');
            }
            (None, Some(value)) => {
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(value);
                out.push('"');
            }
            (None, None) => out.push_str(name),
        }
    }
    out.push_str(close);
    out.push('>');
    out
}

/// An attribute value's text: the references the document writes, read.
fn unescape(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains('&') {
        return value.into();
    }
    value
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#13;", "\r")
        .replace("&amp;", "&")
        .into()
}

/// A `modulepreload` in the JavaScript shell's head (`host/web-js/build.mjs`).
const MODULE_PRELOAD: &str = "<link rel=\"modulepreload\" href=\"";

#[cfg(test)]
mod tests {
    use super::*;

    /// A streamed page's head went before its render found the page's
    /// scroller: the rest marks `<html>` with the one script the CSP admits.
    #[test]
    fn a_streamed_page_marks_its_scroller_after_the_head() {
        let shell = "<!doctype html>\n<html lang=\"en\">\n<meta charset=\"utf-8\">\n<base href=\"/\">\n<title>Blog</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<link rel=\"modulepreload\" href=\"./app.js\">\n<style>p{margin:0}</style>\n<div id=\"exact-root\"></div>\n<script type=\"module\" src=\"./app.js\"></script>\n";
        let mut rendered = Rendered {
            document: exact_web::document::Document {
                lang: "en".into(),
                dir: "ltr".into(),
                root: "<div></div>".into(),
                viewport_fit: None,
                interactive_widget: None,
                head: Default::default(),
                keyframes: String::new(),
                scroll_document: true,
            },
            head: String::new(),
            checkpoint: "{}".into(),
            digest: "0".into(),
            state: Default::default(),
            settled: crate::Settled::Complete,
            activate: exact_plan::ActivatePolicy::Inferred,
        };
        let head = head_js(shell, "en", "ltr", true, false).unwrap();
        assert!(head.contains("<html lang=\"en\" dir=\"ltr\">"), "{head}");
        let mark = format!("<script>{}</script>", scroll_document_js());
        let rest = body_js(shell, &rendered, false, false).unwrap();
        assert!(rest.find(&mark).unwrap() < rest.find("<div id=\"exact-root\">").unwrap());
        assert!(!body_js(shell, &rendered, false, true)
            .unwrap()
            .contains(&mark));
        rendered.document.scroll_document = false;
        assert!(!body_js(shell, &rendered, false, false)
            .unwrap()
            .contains(&mark));
    }
}
