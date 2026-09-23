//! The safe document projection: the page the live host builds, as HTML,
//! before browser layout.
//!
//! @ref LLP 1048 D1 (the document is the page) / LLP 1048.000 D1
//!
//! One walk over the kernel tree from the runner's roots. Each element's tag,
//! attributes and inline style come from the live host's own rules —
//! [`super::tag_for`], [`super::props_for`], [`crate::css`] and
//! [`super::host_css`] — and the rules `glue.js` applies imperatively when it
//! creates an element (`applyProps`, `attach`, the canvas wrapper,
//! `renderMarkup`, `navigation.project`) are restated here, each beside the
//! rule it mirrors. A change to one side without the other is what the parity
//! check (a parsed document against the live DOM, in Chrome) exists to catch.
//!
//! What the browser decides after layout is not in a document: font loading,
//! symbol sizing from computed styles (a symbol image has no `src`), focus
//! (`autofocus` is the focus controller's), scrolling, context positioning,
//! windows chosen from scrollport geometry, and controls the glue disables
//! until its module is ready.
//!
//! Output the HTML parser would restructure — a link inside a link, a button
//! inside a button, a NUL it drops — is a refusal naming the view, never
//! repaired: the page a reader gets without JavaScript is the page the live
//! host builds, or it is no page.

use super::{font_names, host_css, props_for, tag_for, Host};

#[path = "page.rs"]
mod page;
use crate::css;
use exact_kernel::{NodeRef, PropId, ViewId};
use exact_plan::EventKind;
use exact_runner::{DataSource, Runner};
pub use page::{
    build_locations, canonical_location, checkpoint, digest, read_checkpoint, route_at,
    route_location, Site,
};
use std::collections::BTreeMap;
use std::fmt;

/// A projected document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// What `#exact-root` holds: the roots' elements, in order, with no
    /// whitespace between elements (a parser keeps whitespace as text).
    pub root: String,
    /// The first root's `viewport-fit`, which the glue writes into the
    /// viewport meta (`syncViewportFit`).
    pub viewport_fit: Option<String>,
    /// The first root's `interactive-widget`, likewise.
    pub interactive_widget: Option<String>,
    /// The active head's fields: the page's `<head>` (LLP 1048.003 D1). A
    /// head node has no element in the root.
    pub head: exact_runner::Head,
}

/// Why a tree has no document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentError {
    /// The view whose element the parser would restructure.
    pub view: ViewId,
    /// What it would do.
    pub reason: String,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "view {}: {}", self.view, self.reason)
    }
}

impl std::error::Error for DocumentError {}

impl<D: DataSource> Host<D> {
    /// This page's document: [`project`] over the host's runner.
    pub fn document(&self) -> Result<Document, DocumentError> {
        project(&self.runner)
    }
}

/// The document of `runner`'s current tree.
pub fn project<D: DataSource>(runner: &Runner<D>) -> Result<Document, DocumentError> {
    let kernel = runner.kernel();
    let mut walk = Walk {
        runner,
        fonts: font_names(runner.plan()),
        handlers: runner.handlers(),
        routes: BTreeMap::new(),
        out: String::new(),
        links: 0,
        buttons: 0,
    };
    let roots = runner.roots();
    for root in &roots {
        walk.element(*root)?;
    }
    let first = roots.first().and_then(|id| kernel.node(*id));
    let prop = |id| {
        first
            .as_ref()
            .and_then(|n| n.props.str(id))
            .map(str::to_owned)
    };
    Ok(Document {
        root: walk.out,
        viewport_fit: prop(PropId::ViewportFit),
        interactive_widget: prop(PropId::InteractiveWidget),
        head: runner.head(),
    })
}

/// What the router's projection (`navigation.project`) sets on a route.
#[derive(Debug, Clone, Copy)]
struct Route {
    hidden: bool,
    inert: bool,
}

struct Walk<'r, D: DataSource> {
    runner: &'r Runner<D>,
    fonts: Vec<String>,
    handlers: BTreeMap<ViewId, Vec<EventKind>>,
    routes: BTreeMap<ViewId, Route>,
    out: String,
    /// Open `a` and `button` elements: the parser closes an open one when a
    /// second starts inside it, and a button's containers are `<span>`s.
    links: u32,
    buttons: u32,
}

impl<D: DataSource> Walk<'_, D> {
    fn element(&mut self, id: ViewId) -> Result<(), DocumentError> {
        let runner = self.runner;
        let kernel = runner.kernel();
        let node = kernel.node(id).expect("the runner's tree names live views");
        if node.node_type.is_metadata() {
            // The page's `<head>`, never an element (as the live host).
            return Ok(());
        }
        let refuse = |reason: &str| DocumentError {
            view: id,
            reason: reason.to_owned(),
        };
        let tag = tag_for(&node, self.buttons > 0);
        match tag {
            "a" if self.links > 0 => return Err(refuse("a link inside a link")),
            "button" if self.buttons > 0 => return Err(refuse("a button inside a button")),
            _ => {}
        }
        let props = props_for(&node);
        let (text, _) = css::css_text(node.style, &self.fonts);
        let mut style = host_css(&node, text, tag);
        // `glue.js` create: a canvas is a `div` holding the surface element.
        let element = if tag == "canvas" { "div" } else { tag };
        let children = node.children();
        self.route_children(&node, &children);
        let mut attrs: Vec<(String, Option<String>)> = Vec::new();
        let mut content: Option<String> = None;
        let mut markup: Option<String> = None;
        for (name, value) in &props {
            match name.as_str() {
                // Browser-owned state the glue keeps in JavaScript.
                "scrollFollowEnd" | "scrollTop" | "scrollLeft" | "autofocus" => {}
                // A symbol's source is its mask; the glue writes a sized
                // placeholder after layout (`refreshSymbols`).
                "src" if element == "img" && value.starts_with("symbol:") => {}
                // `el.textContent = value` while it has no element children:
                // a canvas already holds its surface.
                "text" => {
                    if tag != "canvas" && !value.is_empty() {
                        content = Some(value.clone());
                    }
                }
                "markupPieces" => markup = Some(value.clone()),
                "data-action" => {
                    attrs.push((name.clone(), Some(value.clone())));
                    style.push_str("touch-action:none;");
                }
                // `writeValue`: an input's value and a button's (a reflected
                // attribute) are the element's; a textarea's is its text.
                "value" => match element {
                    "input" | "button" => attrs.push((name.clone(), Some(value.clone()))),
                    "textarea" => content = Some(value.clone()),
                    _ => {}
                },
                "checked" | "inert" | "disabled" | "readonly" => {
                    if value == "true" {
                        attrs.push((name.clone(), None));
                    }
                }
                "autoplay"
                | "controls"
                | "loop"
                | "muted"
                | "playsinline"
                | "disablepictureinpicture"
                | "disableremoteplayback"
                    if element == "video" =>
                {
                    if value == "true" {
                        attrs.push((name.clone(), None));
                    }
                }
                // `navigates` + `navigableURL`: a refused link loses its
                // href; a refused frame shows about:blank.
                "href" if !navigable(value) => {}
                "src" if element == "iframe" && !navigable(value) => {
                    attrs.push((name.clone(), Some("about:blank".into())));
                }
                _ => attrs.push((name.clone(), Some(value.clone()))),
            }
        }
        // `navigation.project`: routes other than the selected one (and the
        // one under a selected modal) are hidden; every route but the
        // selected one is inert.
        if let Some(route) = self.routes.get(&id).copied() {
            if route.hidden {
                style.push_str("visibility:hidden;");
            }
            if route.inert && !attrs.iter().any(|(n, _)| n == "inert") {
                attrs.push(("inert".into(), None));
            }
        }
        // `attach`: the view id, and a tab stop for an element that hears
        // focus, blur or keys and is not one already.
        attrs.push(("data-view".into(), Some(id.to_string())));
        if let Some(kinds) = self.handlers.get(&id).filter(|kinds| !kinds.is_empty()) {
            attrs.push((
                "data-exact-on".into(),
                Some(
                    kinds
                        .iter()
                        .map(|kind| kind.name())
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
            ));
        }
        let hears = self.handlers.get(&id).is_some_and(|kinds| {
            kinds
                .iter()
                .any(|k| matches!(k, EventKind::Focus | EventKind::Blur | EventKind::Key))
        });
        if hears && !matches!(element, "input" | "button") {
            attrs.push(("tabindex".into(), Some("0".into())));
        }
        if !style.is_empty() {
            attrs.push(("style".into(), Some(style)));
        }
        self.open(id, element, &attrs)?;
        if matches!(element, "img" | "input") {
            // Void: no content, no end tag.
            return Ok(());
        }
        if tag == "canvas" {
            self.out.push_str(SURFACE);
        }
        if let (Some(json), true) = (&markup, children.is_empty()) {
            // `renderMarkup`; a node's children replace its pieces.
            self.markup(id, json)?;
        } else if let Some(text) = &content {
            // The parser drops a newline right after `<textarea>`.
            if element == "textarea" && text.starts_with('\n') {
                self.out.push('\n');
            }
            escape(&mut self.out, text, false).map_err(refuse)?;
        }
        let (link, button) = (element == "a", element == "button");
        self.links += u32::from(link);
        self.buttons += u32::from(button);
        for child in children {
            self.element(child)?;
        }
        self.links -= u32::from(link);
        self.buttons -= u32::from(button);
        self.out.push_str("</");
        self.out.push_str(element);
        self.out.push('>');
        Ok(())
    }

    fn open(
        &mut self,
        id: ViewId,
        element: &str,
        attrs: &[(String, Option<String>)],
    ) -> Result<(), DocumentError> {
        self.out.push('<');
        self.out.push_str(element);
        for (name, value) in attrs {
            self.out.push(' ');
            self.out.push_str(name);
            if let Some(value) = value {
                self.out.push_str("=\"");
                escape(&mut self.out, value, true).map_err(|reason| DocumentError {
                    view: id,
                    reason: format!("attribute `{name}`: {reason}"),
                })?;
                self.out.push('"');
            }
        }
        self.out.push('>');
        Ok(())
    }

    /// Mark the routes under a navigation root as `navigation.project` does.
    fn route_children(&mut self, node: &NodeRef<'_>, children: &[ViewId]) {
        if node.props.str(PropId::NavigationBack).is_none() {
            return;
        }
        let runner = self.runner;
        let kernel = runner.kernel();
        let key = node.props.str(PropId::NavigationKey);
        let routes: Vec<NodeRef<'_>> = children
            .iter()
            .filter_map(|c| kernel.node(*c))
            .filter(|c| c.props.str(PropId::NavigationKey).is_some())
            .collect();
        // A key that names no route leaves the stack as it is.
        let Some(selected) = routes
            .iter()
            .position(|r| r.props.str(PropId::NavigationKey) == key)
        else {
            return;
        };
        let modal = routes[selected].props.str(PropId::NavigationPresentation) == Some("modal");
        for (index, route) in routes.iter().enumerate() {
            let active = index == selected;
            let shown = active || (modal && index + 1 == selected);
            self.routes.insert(
                route.id,
                Route {
                    hidden: !shown,
                    inert: !active,
                },
            );
        }
    }

    /// `renderMarkup`: each piece a `span`, or an `a` when it is a link to
    /// a navigable destination; newlines are `<br>`s.
    fn markup(&mut self, id: ViewId, json: &str) -> Result<(), DocumentError> {
        let refuse = |reason: String| DocumentError { view: id, reason };
        for piece in markup_pieces(json).map_err(refuse)? {
            let link = piece.flags & 8 != 0 && !piece.href.is_empty() && navigable(&piece.href);
            if link && self.links > 0 {
                return Err(refuse("a Markdown link inside a link".into()));
            }
            let mut style = String::new();
            if piece.scale != "1" {
                style.push_str(&format!("font-size:{}em;", piece.scale));
            }
            if piece.weight != "0" {
                style.push_str(&format!("font-weight:{};", piece.weight));
            }
            if piece.flags & 1 != 0 {
                style.push_str("font-style:italic;");
            }
            if piece.flags & 2 != 0 {
                style.push_str("font-family:ui-monospace, monospace;");
            }
            if piece.flags & 4 != 0 {
                style.push_str("text-decoration:line-through;");
            }
            if piece.flags & 16 != 0 {
                style.push_str("opacity:0.62;");
            }
            let element = if link { "a" } else { "span" };
            let mut attrs = Vec::new();
            if !style.is_empty() {
                attrs.push(("style".to_owned(), Some(style)));
            }
            if link {
                attrs.push(("href".to_owned(), Some(piece.href.clone())));
            }
            self.open(id, element, &attrs)?;
            for (n, line) in piece.text.split('\n').enumerate() {
                if n > 0 {
                    self.out.push_str("<br>");
                }
                escape(&mut self.out, line, false).map_err(|e| refuse(e.to_owned()))?;
            }
            self.out.push_str("</");
            self.out.push_str(element);
            self.out.push('>');
        }
        Ok(())
    }
}

/// The surface element `glue.js` puts first in a canvas's `div`.
const SURFACE: &str = "<canvas data-surface=\"\" style=\"position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1\"></canvas>";

/// Append `text` escaped for HTML text (`attribute` false) or a
/// double-quoted attribute value. A carriage return is a reference (the
/// parser folds a literal one into a newline); a NUL has no spelling the
/// parser keeps.
fn escape(out: &mut String, text: &str, attribute: bool) -> Result<(), &'static str> {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\0' => return Err("a NUL character, which the HTML parser drops"),
            c => out.push(c),
        }
    }
    Ok(())
}

/// Whether `href` navigates somewhere the page allows: `navigableURL`'s one
/// scheme allowlist (http, https, mailto, tel), read the way the URL parser
/// reads a scheme — surrounding C0 controls and spaces stripped, tabs and
/// newlines removed anywhere, the scheme case-folded. A URL with no scheme is
/// relative to the page's https base; an http(s) authority whose host does
/// not parse is refused, as the parser's failure is (IDNA aside).
pub fn navigable(href: &str) -> bool {
    let url: String = href
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let mut scheme = String::new();
    for (at, c) in url.char_indices() {
        match c {
            ':' if !scheme.is_empty() => {
                let rest = &url[at + 1..];
                return match scheme.as_str() {
                    "mailto" | "tel" => true,
                    // The base's own scheme reads the rest as relative
                    // unless it starts `//`; another special scheme's
                    // authority follows whatever slashes there are.
                    "https" if !rest.starts_with("//") => true,
                    "http" | "https" => authority_parses(rest),
                    _ => false,
                };
            }
            c if c.is_ascii_alphabetic() => scheme.push(c.to_ascii_lowercase()),
            c if !scheme.is_empty() && (c.is_ascii_digit() || matches!(c, '+' | '-' | '.')) => {
                scheme.push(c)
            }
            _ => break,
        }
    }
    // Relative: a leading pair of slashes (either way) starts an authority.
    let mut slashes = url.chars().take_while(|c| matches!(c, '/' | '\\'));
    if slashes.next().is_some() && slashes.next().is_some() {
        return authority_parses(&url);
    }
    true
}

/// The special authority after any slashes: a host (a bracketed IPv6, an
/// IPv4 when its last label is numeric, or a domain free of forbidden code
/// points) and a port of at most 65535.
fn authority_parses(rest: &str) -> bool {
    let rest = rest.trim_start_matches(['/', '\\']);
    let authority = &rest[..rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len())];
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let (host, port) = if let Some(inner) = hostport.strip_prefix('[') {
        let Some((v6, after)) = inner.split_once(']') else {
            return false;
        };
        if v6.is_empty()
            || !v6
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return false;
        }
        match after {
            "" => (hostport, None),
            p => match p.strip_prefix(':') {
                Some(port) => (hostport, Some(port)),
                None => return false,
            },
        }
    } else {
        match hostport.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (hostport, None),
        }
    };
    if port.is_some_and(|p| {
        !p.is_empty()
            && (!p.bytes().all(|b| b.is_ascii_digit())
                || p.parse::<u32>().map_or(true, |n| n > 65535))
    }) {
        return false;
    }
    if host.starts_with('[') {
        return true;
    }
    let decoded = percent_decoded(host);
    if decoded.is_empty()
        || decoded.chars().any(|c| {
            c <= ' '
                || c == '\u{7f}'
                || matches!(
                    c,
                    '#' | '%' | '/' | ':' | '<' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '|'
                )
        })
    {
        return false;
    }
    // A host whose last label is a number is an IPv4 address and must be one.
    let labels: Vec<&str> = decoded.trim_end_matches('.').split('.').collect();
    let number = |label: &str| -> Option<u64> {
        let lower = label.to_ascii_lowercase();
        match lower.strip_prefix("0x") {
            Some("") => Some(0),
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None if lower.len() > 1 && lower.starts_with('0') => {
                u64::from_str_radix(&lower[1..], 8).ok()
            }
            None => lower.parse().ok(),
        }
    };
    let last = labels.last().copied().unwrap_or_default();
    let numeric = !last.is_empty()
        && (last.bytes().all(|b| b.is_ascii_digit())
            || last
                .to_ascii_lowercase()
                .strip_prefix("0x")
                .is_some_and(|h| h.bytes().all(|b| b.is_ascii_hexdigit())));
    if !numeric {
        return true;
    }
    let Some(parts) = labels
        .iter()
        .map(|l| number(l))
        .collect::<Option<Vec<u64>>>()
    else {
        return false;
    };
    let (init, tail) = parts.split_at(parts.len() - 1);
    parts.len() <= 4
        && init.iter().all(|n| *n <= 255)
        && tail[0] < 256u64.pow(5 - parts.len() as u32)
}

/// `host`'s percent-escapes decoded as the host parser decodes them (bytes
/// that are not UTF-8 make it unparseable, as an empty host is).
fn percent_decoded(host: &str) -> String {
    let bytes = host.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).and_then(|b| hex(*b)),
            bytes.get(i + 2).and_then(|b| hex(*b)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// One piece of `markupPieces`, as the page reads it: numbers keep the
/// spelling the glue's template literals would give them.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    text: String,
    scale: String,
    weight: String,
    flags: u8,
    href: String,
}

/// Read the host's own `markup_json` back (`[[text, scale, weight, flags,
/// href], …]`). The host wrote it; anything else is a defect.
fn markup_pieces(json: &str) -> Result<Vec<Piece>, String> {
    let mut p = Json {
        bytes: json.as_bytes(),
        at: 0,
    };
    let mut pieces = Vec::new();
    p.expect(b'[')?;
    if p.peek() == Some(b']') {
        return Ok(pieces);
    }
    loop {
        p.expect(b'[')?;
        let text = p.string()?;
        p.expect(b',')?;
        let scale = p.number()?;
        p.expect(b',')?;
        let weight = p.number()?;
        p.expect(b',')?;
        let flags = p.number()?.parse::<u8>().map_err(|e| e.to_string())?;
        p.expect(b',')?;
        let href = p.string()?;
        p.expect(b']')?;
        pieces.push(Piece {
            text,
            scale,
            weight,
            flags,
            href,
        });
        match p.next() {
            Some(b',') => continue,
            Some(b']') => return Ok(pieces),
            other => return Err(format!("markup pieces: unexpected {other:?}")),
        }
    }
}

struct Json<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Json<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let b = self.peek();
        self.at += 1;
        b
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        match self.next() {
            Some(got) if got == b => Ok(()),
            got => Err(format!(
                "markup pieces: expected `{}`, found {got:?}",
                b as char
            )),
        }
    }

    fn number(&mut self) -> Result<String, String> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'.' | b'-' | b'e' | b'E' | b'+')
        ) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?;
        // The glue's `${scale}` and `fontWeight = weight` spell the number
        // as JavaScript does; the host's `css::num` already writes it so.
        text.parse::<f64>()
            .map(|_| text.to_owned())
            .map_err(|e| format!("markup pieces: number {text:?}: {e}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?,
            );
            match self.next() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.next() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let hex = self
                            .bytes
                            .get(self.at..self.at + 4)
                            .and_then(|h| std::str::from_utf8(h).ok())
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .and_then(char::from_u32)
                            .ok_or("markup pieces: bad \\u escape")?;
                        self.at += 4;
                        out.push(hex);
                    }
                    other => return Err(format!("markup pieces: bad escape {other:?}")),
                },
                _ => return Err("markup pieces: unterminated string".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape, markup_pieces, navigable};

    #[test]
    fn schemes_are_read_as_the_url_parser_reads_them() {
        for allowed in [
            "https://e.dev/a",
            "HTTP://e.dev",
            "mailto:a@e.dev",
            "tel:+15555550100",
            " https://e.dev ",
            "/post/5",
            "post/5?q=1#x",
            "#top",
            "",
            "//e.dev/a",
            "?q=javascript:x",
        ] {
            assert!(navigable(allowed), "{allowed:?}");
        }
        for refused in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java\tscript:alert(1)",
            "java\nscript:alert(1)",
            "\u{1}javascript:alert(1)",
            " javascript:alert(1)",
            "data:text/html,<b>x</b>",
            "vbscript:x",
            "file:///etc/passwd",
            "c:/x",
            "a+b.c-d:x",
            // Hosts the parser cannot read: the browser refuses the link.
            "http:",
            "http://",
            "//exa mple.com/",
            "//[::1",
            "https://a b/",
            "https://e.dev:99999/",
            "https://e.dev:8o/",
            "http://1.2.3.999/",
            "http://e%20v.dev/",
        ] {
            assert!(!navigable(refused), "{refused:?}");
        }
        // Read as the browser reads them (each checked against Bun's URL).
        for allowed in [
            "\u{a0}javascript:x",
            "\\\\e.dev\\x",
            "1http:x",
            "-x:y",
            "javascript\u{a0}:x",
            "https:post/5",
            "http:e.dev",
            "http://[::1]:8080/",
            "http://user@e.dev:80/a",
            "http://0x7f.1/",
            "http://e.dev./",
            "https://bücher.example/",
        ] {
            assert!(navigable(allowed), "{allowed:?}");
        }
        assert!(!navigable("javas\rcript:x"));
    }

    #[test]
    fn escaping_keeps_what_the_parser_would_fold_or_drop() {
        let mut out = String::new();
        escape(&mut out, "a<b>&\"c\"\r\n", true).unwrap();
        assert_eq!(out, "a&lt;b&gt;&amp;&quot;c&quot;&#13;\n");
        let mut out = String::new();
        escape(&mut out, "\"q\"", false).unwrap();
        assert_eq!(out, "\"q\"");
        assert!(escape(&mut String::new(), "a\0b", false).is_err());
    }

    #[test]
    fn markup_pieces_read_back_what_the_host_wrote() {
        let json =
            super::super::element::markup_json("# T \"q\"\n\n**b** [l](https://e.dev/a?b=1) `c`");
        let pieces = markup_pieces(&json).unwrap();
        assert_eq!(pieces[0].text, "T \"q\"");
        assert_eq!(pieces[0].scale, "1.6");
        assert_eq!(pieces[0].weight, "700");
        assert_eq!(pieces[5].flags, 8);
        assert_eq!(pieces[5].href, "https://e.dev/a?b=1");
        assert_eq!(pieces.last().unwrap().scale, "0.92");
        assert!(markup_pieces("[]").unwrap().is_empty());
        assert_eq!(
            markup_pieces(&super::super::element::markup_json("a\\b\tc\u{1}")).unwrap()[0].text,
            "a\\b\tc\u{1}"
        );
    }
}
