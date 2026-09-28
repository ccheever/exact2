//! The page around a document: its `<head>`, its checkpoint, and the
//! locations a build renders. The render host (`exact-render`) composes them.
//!
//! @ref LLP 1048.000 D3 (the head), D6 (the checkpoint, 1a's form), D7
//! (`dist/`)
//!
//! The head is the active `head` element's fields (LLP 1048.003 D1) with the
//! app's name as the title when no head sets one, the viewport meta the glue
//! would write (`syncViewportFit`), and the plan's fonts as `@font-face` rules
//! with their preloads — so a reader without JavaScript sees the page's own
//! faces. URLs a crawler reads (canonical, image) are made absolute against
//! the manifest's origin, never a request's. Nothing preloads the glue or the
//! wasm (D3).

use super::{escape, navigable, Document, DocumentError, Json};
use exact_plan::{Plan, RenderPolicy, Value};
use exact_runner::{Checkpoint, DataSource, Runner};
use std::fmt::Write as _;

/// What the page knows of the app beyond its plan: the manifest's name, the
/// title when no head sets one, and its production origin.
#[derive(Debug, Clone, Copy, Default)]
pub struct Site<'a> {
    /// The manifest's name.
    pub name: &'a str,
    /// The manifest's origin (`app.origin`), e.g. `https://example.com`.
    pub origin: Option<&'a str>,
}

impl Document {
    /// What `<head>` holds for this document, after the shell's charset and
    /// base: the title, the viewport meta, the head's description, canonical,
    /// robots, OpenGraph and Twitter tags, the plan's fonts, and the
    /// `@keyframes` the document's animations name.
    pub fn page_head(
        &self,
        plan: &Plan,
        site: &Site<'_>,
        location: &str,
    ) -> Result<String, DocumentError> {
        let head = &self.head;
        let mut out = String::new();
        let title = head.title.as_deref().unwrap_or(site.name);
        out.push_str("<title>");
        escape(&mut out, title, false).map_err(|reason| DocumentError {
            view: 0,
            reason: format!("the head's title: {reason}"),
        })?;
        out.push_str("</title>");
        // `syncViewportFit`, from the first root's policy.
        let mut viewport = String::from("width=device-width, initial-scale=1");
        if self.viewport_fit.as_deref() == Some("cover") {
            viewport.push_str(", viewport-fit=cover");
        }
        if let Some(widget) = &self.interactive_widget {
            let _ = write!(viewport, ", interactive-widget={widget}");
        }
        out.push_str("<meta name=\"viewport\" ");
        attr(&mut out, "content", &viewport)?;
        out.push('>');
        let meta = |out: &mut String, key: &str, name: &str, value: &str| {
            let _ = write!(out, "<meta {key}=\"{name}\" ");
            let mut content = String::new();
            escape(&mut content, value, true).map_err(|reason| DocumentError {
                view: 0,
                reason: format!("the head's `{name}`: {reason}"),
            })?;
            let _ = write!(out, "content=\"{content}\">");
            Ok::<(), DocumentError>(())
        };
        if let Some(description) = &head.description {
            meta(&mut out, "name", "description", description)?;
        }
        if let Some(robots) = &head.robots {
            meta(&mut out, "name", "robots", robots)?;
        }
        let canonical = head
            .canonical
            .as_deref()
            .filter(|url| navigable(url))
            .map(|url| absolute(url, site.origin));
        if let Some(url) = &canonical {
            out.push_str("<link rel=\"canonical\" ");
            attr(&mut out, "href", url)?;
            out.push('>');
        }
        let image = head
            .image
            .as_deref()
            .filter(|url| navigable(url))
            .map(|url| absolute(url, site.origin));
        let url = canonical.or_else(|| site.origin.map(|origin| absolute(location, Some(origin))));
        meta(&mut out, "property", "og:type", "website")?;
        meta(&mut out, "property", "og:site_name", site.name)?;
        meta(&mut out, "property", "og:title", title)?;
        if let Some(description) = &head.description {
            meta(&mut out, "property", "og:description", description)?;
        }
        if let Some(url) = &url {
            meta(&mut out, "property", "og:url", url)?;
        }
        if let Some(image) = &image {
            meta(&mut out, "property", "og:image", image)?;
        }
        let card = if image.is_some() {
            "summary_large_image"
        } else {
            "summary"
        };
        meta(&mut out, "name", "twitter:card", card)?;
        meta(&mut out, "name", "twitter:title", title)?;
        if let Some(description) = &head.description {
            meta(&mut out, "name", "twitter:description", description)?;
        }
        if let Some(image) = &image {
            meta(&mut out, "name", "twitter:image", image)?;
        }
        fonts(&mut out, plan)?;
        if !self.keyframes.is_empty() {
            let _ = write!(out, "<style>{}</style>", self.keyframes);
        }
        Ok(out)
    }
}

/// ` name="value"`, escaped for an attribute.
fn attr(out: &mut String, name: &str, value: &str) -> Result<(), DocumentError> {
    out.push_str(name);
    out.push_str("=\"");
    escape(out, value, true).map_err(|reason| DocumentError {
        view: 0,
        reason: format!("the head's `{name}`: {reason}"),
    })?;
    out.push('"');
    Ok(())
}

/// `url` made absolute against the site's origin; an absolute URL, or no
/// origin, leaves it as it is.
fn absolute(url: &str, origin: Option<&str>) -> String {
    let scheme = url.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    match origin {
        Some(origin) if !scheme && !url.starts_with("//") => {
            let origin = origin.trim_end_matches('/');
            if url.starts_with('/') {
                format!("{origin}{url}")
            } else {
                format!("{origin}/{url}")
            }
        }
        _ => url.to_owned(),
    }
}

/// The plan's declared faces as `@font-face` rules under the family names
/// the host's CSS uses, and a preload each (LLP 1019): what the glue's
/// readiness barrier loads, declared for a reader without JavaScript.
fn fonts(out: &mut String, plan: &Plan) -> Result<(), DocumentError> {
    let faces = super::super::font_faces(plan);
    if faces.is_empty() {
        return Ok(());
    }
    let mut rules = String::new();
    for face in &faces {
        if face
            .source
            .chars()
            .any(|c| matches!(c, '"' | '\\' | '<' | '>' | '\n' | '\r'))
        {
            return Err(DocumentError {
                view: 0,
                reason: format!("font source {:?} can't be a CSS string", face.source),
            });
        }
        let _ = write!(
            rules,
            "@font-face{{font-family:\"{}\";src:url(\"{}\");font-weight:{};font-style:{}}}",
            face.family,
            face.source,
            face.weight,
            if face.italic { "italic" } else { "normal" }
        );
    }
    let _ = write!(out, "<style>{rules}</style>");
    for face in &faces {
        let kind = match face.source.rsplit('.').next().map(str::to_ascii_lowercase) {
            Some(ext) if matches!(ext.as_str(), "ttf" | "otf" | "woff" | "woff2") => ext,
            _ => continue,
        };
        let _ = write!(
            out,
            "<link rel=\"preload\" href=\"{}\" as=\"font\" type=\"font/{kind}\" crossorigin>",
            face.source
        );
    }
    Ok(())
}

/// The runner's checkpoint (LLP 1048.000 D6) as JSON a script element can
/// carry: the location, the render's time, the logic that answered, the
/// answers — each `[name, source, [args…], value]`, the values in
/// [`push_value`]'s JSON — and what is still pending. `<`, `>` and `&` are
/// escapes, so user text can never close the element or open a comment.
/// [`read_checkpoint`] reads it back.
///
/// The values are text, not their canonical bytes in base64, so a page's
/// compression finds the strings its document already shows (titles, names,
/// dates): on RealWorld's `/` that took about 1.8 KB off the page (brotli,
/// as sent).
pub fn checkpoint<D: DataSource>(runner: &Runner<D>, location: &str) -> String {
    let checkpoint = runner.document_checkpoint(location);
    let mut json = String::from("{\"location\":");
    crate::batch::quote(&checkpoint.location, &mut json);
    let _ = write!(
        json,
        ",\"time\":{},\"logic\":",
        exact_runner::agent::num(checkpoint.now_ms)
    );
    match &checkpoint.logic {
        Some(logic) => crate::batch::quote(logic, &mut json),
        None => json.push_str("null"),
    }
    json.push_str(",\"answers\":[");
    for (i, (name, source, args, value)) in checkpoint.answers.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('[');
        crate::batch::quote(name, &mut json);
        json.push(',');
        crate::batch::quote(source, &mut json);
        json.push(',');
        push_value(&Value::list(args.clone()), &mut json);
        json.push(',');
        push_value(value, &mut json);
        json.push(']');
    }
    json.push_str("],\"pending\":[");
    for (i, name) in checkpoint.pending.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        crate::batch::quote(name, &mut json);
    }
    json.push_str("]}");
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

/// A plan value as JSON that reads back to the same value ([`read_value`]):
/// a number as a number (`{"n":"NaN"}`, `{"n":"Infinity"}`,
/// `{"n":"-Infinity"}` when it isn't finite; `-0` kept), a boolean, a string,
/// unit as `null`, `none` as `{}`, `some(v)` as `{"s":v}`, a list as an
/// array, a record as `{"r":[fields…]}`.
fn push_value(value: &Value, out: &mut String) {
    use exact_num::Piece as _;
    match value {
        Value::Number(n) if n.is_nan() => out.push_str("{\"n\":\"NaN\"}"),
        Value::Number(n) if n.is_infinite() && *n > 0.0 => out.push_str("{\"n\":\"Infinity\"}"),
        Value::Number(n) if n.is_infinite() => out.push_str("{\"n\":\"-Infinity\"}"),
        Value::Number(n) if *n == 0.0 && n.is_sign_negative() => out.push_str("-0"),
        Value::Number(n) => exact_runner::agent::num(*n).push_to(out),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        text @ exact_plan::str_value!() => crate::batch::quote(text.text(), out),
        Value::Unit => out.push_str("null"),
        Value::Option(None) => out.push_str("{}"),
        Value::Option(Some(v)) => {
            out.push_str("{\"s\":");
            push_value(v, out);
            out.push('}');
        }
        Value::List(items) | Value::Record(items) => {
            let record = matches!(value, Value::Record(_));
            out.push_str(if record { "{\"r\":[" } else { "[" });
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_value(item, out);
            }
            out.push_str(if record { "]}" } else { "]" });
        }
    }
}

/// A value [`push_value`] wrote.
fn read_value(json: &mut Json<'_>) -> Result<Value, String> {
    let rest = &json.bytes[json.at..];
    let word = |w: &str| rest.starts_with(w.as_bytes());
    match json.peek() {
        Some(b'"') => json.string().map(|s| Value::str(&s)),
        Some(b'[') => {
            json.next();
            let mut items = Vec::new();
            while json.peek() != Some(b']') {
                if !items.is_empty() {
                    json.expect(b',')?;
                }
                items.push(read_value(json)?);
            }
            json.next();
            Ok(Value::list(items))
        }
        Some(b'{') => {
            json.next();
            if json.peek() == Some(b'}') {
                json.next();
                return Ok(Value::NONE);
            }
            let key = json.string()?;
            json.expect(b':')?;
            let value = match key.as_str() {
                "s" => Value::some(read_value(json)?),
                "r" => match read_value(json)? {
                    Value::List(items) => Value::Record(items),
                    _ => return Err("a record's fields are not a list".into()),
                },
                "n" => match json.string()?.as_str() {
                    "NaN" => Value::Number(f64::NAN),
                    "Infinity" => Value::Number(f64::INFINITY),
                    "-Infinity" => Value::Number(f64::NEG_INFINITY),
                    other => return Err(format!("not a number: {other:?}")),
                },
                other => return Err(format!("not a value: `{other}`")),
            };
            json.expect(b'}')?;
            Ok(value)
        }
        _ if word("true") => {
            json.at += 4;
            Ok(Value::Bool(true))
        }
        _ if word("false") => {
            json.at += 5;
            Ok(Value::Bool(false))
        }
        _ if word("null") => {
            json.at += 4;
            Ok(Value::Unit)
        }
        _ => {
            let text = json.number()?;
            exact_num::parse_f64(&text)
                .map(Value::Number)
                .map_err(|e| format!("number {text:?}: {e}"))
        }
    }
}

/// A page's checkpoint, read back: what the web runtime boots from (LLP
/// 1048.000 D6). Refuses anything [`checkpoint`] doesn't write.
pub fn read_checkpoint(text: &str) -> Result<Checkpoint, String> {
    let mut json = Json {
        bytes: text.as_bytes(),
        at: 0,
    };
    let mut checkpoint = Checkpoint::default();
    json.expect(b'{')?;
    loop {
        let key = json.string()?;
        json.expect(b':')?;
        match key.as_str() {
            "location" => checkpoint.location = json.string()?,
            "time" => {
                checkpoint.now_ms =
                    exact_num::parse_f64(&json.number()?).map_err(|e| format!("time: {e}"))?
            }
            "logic" if json.bytes[json.at..].starts_with(b"null") => {
                json.at += 4;
                checkpoint.logic = None;
            }
            "logic" => checkpoint.logic = Some(json.string()?),
            "answers" => {
                json.expect(b'[')?;
                while json.peek() != Some(b']') {
                    if !checkpoint.answers.is_empty() {
                        json.expect(b',')?;
                    }
                    json.expect(b'[')?;
                    let name = json.string()?;
                    json.expect(b',')?;
                    let source = json.string()?;
                    json.expect(b',')?;
                    let Value::List(args) = read_value(&mut json)? else {
                        return Err("answers: arguments not a list".into());
                    };
                    json.expect(b',')?;
                    let value = read_value(&mut json).map_err(|e| format!("answers: {e}"))?;
                    json.expect(b']')?;
                    checkpoint
                        .answers
                        .push((name, source, args.to_vec(), value));
                }
                json.expect(b']')?;
            }
            "pending" => {
                json.expect(b'[')?;
                while json.peek() != Some(b']') {
                    if !checkpoint.pending.is_empty() {
                        json.expect(b',')?;
                    }
                    checkpoint.pending.push(json.string()?);
                }
                json.expect(b']')?;
            }
            other => return Err(format!("unknown field `{other}`")),
        }
        match json.next() {
            Some(b',') => continue,
            Some(b'}') if json.peek().is_none() => return Ok(checkpoint),
            _ => return Err("expected `,` or the end".into()),
        }
    }
}

impl<D: DataSource> crate::Host<D> {
    /// Boot from a page the build or a server rendered (LLP 1048.000 D6):
    /// `page` is its checkpoint as the page carries it, and `page_digest`
    /// the digest the renderer wrote beside it. The checkpoint's answers
    /// seed the runner; the first batch opens with an `adopt` op saying
    /// whether the page's document is this runtime's own first tree — the
    /// digests match — so the glue binds it rather than replacing it. A
    /// checkpoint that doesn't read boots as a page without one, journaled.
    #[allow(clippy::too_many_arguments)] // the boot facts, and the page's two
    pub fn boot_checkpoint(
        plan_bytes: &[u8],
        data: D,
        page: &str,
        page_digest: &str,
        snapshot: Vec<(String, String)>,
        compat: Option<&str>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(crate::Host<D>, String), crate::HostError> {
        crate::Host::boot_checkpoint_linked(
            crate::HostLinks::ALL,
            plan_bytes,
            data,
            page,
            page_digest,
            snapshot,
            compat,
            viewport,
            launch,
        )
    }

    /// [`crate::Host::boot_checkpoint`], with what the artifact links (LLP
    /// 1047 D3).
    #[allow(clippy::too_many_arguments)] // the links, the boot facts, and the page's two
    pub fn boot_checkpoint_linked(
        links: crate::HostLinks<D>,
        plan_bytes: &[u8],
        data: D,
        page: &str,
        page_digest: &str,
        snapshot: Vec<(String, String)>,
        compat: Option<&str>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(crate::Host<D>, String), crate::HostError> {
        let checkpoint = match read_checkpoint(page) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                let (mut host, batch) = crate::Host::boot_linked(
                    links, plan_bytes, data, None, snapshot, compat, viewport, launch,
                )?;
                host.log(&format!("document: the checkpoint doesn't read ({error})"));
                return Ok((host, batch));
            }
        };
        // A page served at another path (a static host's 404.html, or its
        // fallback to `/`) isn't this location's document: render fresh.
        let Some(launch) = page_launch(launch, &checkpoint.location) else {
            let (mut host, batch) = crate::Host::boot_linked(
                links, plan_bytes, data, None, snapshot, compat, viewport, launch,
            )?;
            host.log(&format!(
                "document: rendered at {}, not {launch}; rendering fresh",
                checkpoint.location
            ));
            return Ok((host, batch));
        };
        let launch = launch.as_str();
        let plan = crate::host::decode_plan(plan_bytes).map_err(crate::HostError::Plan)?;
        crate::link::admit(&plan)?;
        let delivery = compat.map_or_else(Default::default, |json| {
            exact_runner::Delivery::default().with_compat(json)
        });
        let runner = Runner::boot_checkpoint_linked(
            crate::link::runner_links(),
            plan,
            data,
            crate::host::browser_kernel(),
            &checkpoint,
            snapshot,
            delivery,
            viewport,
            launch,
        )
        .map_err(crate::HostError::Runner)?;
        // The renderer's location: the runtime's only input of its own is its
        // first tree, so a query the app doesn't read adopts the same document.
        // The projection's per-view results serve the first batch too.
        let (adopted, computed) = match super::project_keeping(&runner) {
            Ok((doc, computed)) => (
                digest(plan_bytes, &checkpoint.location, page, &doc.root) == page_digest,
                computed,
            ),
            Err(_) => (false, Default::default()),
        };
        let mut batch = crate::batch::Batch::new();
        batch.adopt(adopted);
        crate::Host::open(links, runner, launch, batch, computed)
    }
}

/// The document's digest (LLP 1048.000 D6): MurmurHash3's x64 128-bit hash
/// over the plan's bytes, the location the document was rendered at, the
/// checkpoint as the page carries it, and the document — each
/// length-prefixed, so no two different inputs are hashed as the same bytes.
/// The renderer writes it beside the checkpoint; the runtime computes it
/// with its own first tree for the document and adopts the page on a match.
pub fn digest(plan: &[u8], location: &str, checkpoint: &str, document: &str) -> String {
    let mut hash = Murmur128::new(0);
    for part in [
        plan,
        location.as_bytes(),
        checkpoint.as_bytes(),
        document.as_bytes(),
    ] {
        hash.update(&(part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    hex32(hash.finish())
}

/// `format!("{v:032x}")`, without the formatter (the digest is on boot's path).
fn hex32(v: u128) -> String {
    let mut out = String::with_capacity(32);
    let mut shift = 128;
    while shift > 0 {
        shift -= 4;
        out.push(char::from(b"0123456789abcdef"[(v >> shift) as usize & 15]));
    }
    out
}

/// MurmurHash3's x64 128-bit function (Austin Appleby's, public domain),
/// streamed. The document digest is an equality check between the server's
/// document and the runtime's own, not a security boundary: whoever serves the
/// page's HTML already controls the page. So it needs speed and a negligible
/// chance of an accidental match, not collision resistance.
struct Murmur128 {
    h: [u64; 2],
    tail: [u8; 16],
    fill: usize,
    len: u64,
}

const C1: u64 = 0x87c3_7b91_1142_53d5;
const C2: u64 = 0x4cf5_ad43_2745_937f;

impl Murmur128 {
    fn new(seed: u32) -> Self {
        Murmur128 {
            h: [u64::from(seed); 2],
            tail: [0; 16],
            fill: 0,
            len: 0,
        }
    }

    fn block(&mut self, block: &[u8]) {
        let k1 = u64::from_le_bytes(block[..8].try_into().expect("16 bytes"));
        let k2 = u64::from_le_bytes(block[8..16].try_into().expect("16 bytes"));
        let [h1, h2] = &mut self.h;
        *h1 ^= k1.wrapping_mul(C1).rotate_left(31).wrapping_mul(C2);
        *h1 = h1
            .rotate_left(27)
            .wrapping_add(*h2)
            .wrapping_mul(5)
            .wrapping_add(0x52dc_e729);
        *h2 ^= k2.wrapping_mul(C2).rotate_left(33).wrapping_mul(C1);
        *h2 = h2
            .rotate_left(31)
            .wrapping_add(*h1)
            .wrapping_mul(5)
            .wrapping_add(0x3849_5ab5);
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.len += bytes.len() as u64;
        if self.fill > 0 {
            let take = (16 - self.fill).min(bytes.len());
            self.tail[self.fill..self.fill + take].copy_from_slice(&bytes[..take]);
            self.fill += take;
            bytes = &bytes[take..];
            if self.fill < 16 {
                return;
            }
            let tail = self.tail;
            self.block(&tail);
            self.fill = 0;
        }
        let mut blocks = bytes.chunks_exact(16);
        for block in &mut blocks {
            self.block(block);
        }
        let rest = blocks.remainder();
        self.tail[..rest.len()].copy_from_slice(rest);
        self.fill = rest.len();
    }

    fn finish(mut self) -> u128 {
        let mut k = [0u64; 2];
        for (i, byte) in self.tail[..self.fill].iter().enumerate() {
            k[i / 8] |= u64::from(*byte) << (8 * (i % 8));
        }
        let [h1, h2] = &mut self.h;
        if self.fill > 8 {
            *h2 ^= k[1].wrapping_mul(C2).rotate_left(33).wrapping_mul(C1);
        }
        if self.fill > 0 {
            *h1 ^= k[0].wrapping_mul(C1).rotate_left(31).wrapping_mul(C2);
        }
        *h1 ^= self.len;
        *h2 ^= self.len;
        *h1 = h1.wrapping_add(*h2);
        *h2 = h2.wrapping_add(*h1);
        let fmix = |mut k: u64| {
            k ^= k >> 33;
            k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
            k ^= k >> 33;
            k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
            k ^ (k >> 33)
        };
        *h1 = fmix(*h1);
        *h2 = fmix(*h2);
        *h1 = h1.wrapping_add(*h2);
        *h2 = h2.wrapping_add(*h1);
        (u128::from(*h2) << 64) | u128::from(*h1)
    }
}

/// One URL per page (LLP 1048.000 D11): the router's canonical location
/// (dot segments resolved, one percent-encoding spelling for the path and
/// the query), with repeated slashes collapsed and no trailing slash but
/// `/`'s. The server redirects any other spelling here; the runtime compares
/// its URL with a page's location in this form.
pub fn canonical_location(location: &str) -> String {
    let routed = exact_route::canonical(location);
    let (path, query) = match routed.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (routed.as_str(), None),
    };
    let mut out = String::with_capacity(routed.len());
    for part in path.split('/').filter(|p| !p.is_empty()) {
        out.push('/');
        out.push_str(part);
    }
    if out.is_empty() {
        out.push('/');
    }
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    out
}

/// Where a runtime at `launch` boots from a page rendered at `location`
/// (LLP 1048.000 D6): the page's path with the browser's query, when the two
/// are one page once canonical — the route table reads no query, so the
/// query can't make them two — else `None`, and the runtime renders fresh.
fn page_launch(launch: &str, location: &str) -> Option<String> {
    let (browser, rendered) = (canonical_location(launch), canonical_location(location));
    let path = |l: &str| l.split_once('?').map_or(l, |(path, _)| path).to_owned();
    let page = path(&rendered);
    (path(&browser) == page).then(|| match browser.split_once('?') {
        Some((_, query)) => format!("{page}?{query}"),
        None => page,
    })
}

fn route_table(plan: &Plan) -> exact_route::Table {
    exact_route::Table {
        routes: plan
            .routes
            .iter()
            .map(|r| exact_route::Route {
                name: plan.str(r.name).into(),
                pattern: plan.str(r.pattern).into(),
                parent: r.parent.map(|p| p.0 as usize),
                tab: r.tab,
                notfound: r.notfound,
            })
            .collect(),
    }
}

/// The route `location` resolves to (LLP 1038 D2): a declared pattern's,
/// else the not-found route's. `None` for a plan without routes, or one
/// with no not-found route when nothing matches.
pub fn route_at<'p>(plan: &'p Plan, location: &str) -> Option<&'p exact_plan::RoutesRow> {
    let hit = route_table(plan).matches(location)?;
    plan.routes.iter().find(|r| plan.str(r.name) == hit.name)
}

/// Where route `name` is with `value` for its one parameter: a listed page
/// (LLP 1048.000 D2).
pub fn route_location(plan: &Plan, name: &str, value: &str) -> Result<String, String> {
    route_table(plan)
        .path(name, &[value])
        .map_err(|e| format!("{e:?}"))
}

/// The locations a build renders: every route declared `render=build` but
/// the ones whose pages a source lists (the render host asks it), and the
/// not-found document when that route is declared so. The not-found
/// document renders at `/404.html`, which no pattern may match.
pub fn build_locations(plan: &Plan) -> Result<Vec<(String, bool)>, String> {
    let table = route_table(plan);
    let mut out = Vec::new();
    for row in plan
        .routes
        .iter()
        .filter(|r| r.render == RenderPolicy::Build && plan.str(r.pages).is_empty())
    {
        if row.notfound {
            if let Some(hit) = table.matches_pattern("/404.html") {
                return Err(format!(
                    "the not-found document renders at /404.html, which route `{}` matches",
                    hit.name
                ));
            }
            out.push(("/404.html".to_owned(), true));
        } else {
            out.push((exact_route::canonical(plan.str(row.pattern)), false));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod murmur_tests {
    use super::{hex32, Murmur128};

    #[test]
    fn a_digest_is_its_hash_as_32_hex_digits() {
        let mut v: u128 = 0x9e37_79b9_7f4a_7c15;
        for probe in [0, 1, 15, 16, u128::from(u64::MAX), u128::MAX - 1, u128::MAX] {
            assert_eq!(hex32(probe), format!("{probe:032x}"));
        }
        for _ in 0..10_000 {
            v = v
                .wrapping_mul(0x2545_f491_4f6c_dd1d_9e37_79b9_7f4a_7c15)
                .wrapping_add(1);
            let probe = v >> (v % 128);
            assert_eq!(hex32(probe), format!("{probe:032x}"));
        }
    }

    fn hash(bytes: &[u8], seed: u32) -> [u8; 16] {
        let mut h = Murmur128::new(seed);
        h.update(bytes);
        h.finish().to_le_bytes()
    }

    /// SMHasher's verification for MurmurHash3_x64_128: hash the keys
    /// {}, {0}, {0,1}, … {0..254} with seed 256 - n, then hash the
    /// concatenated results with seed 0; its first four bytes, read little-
    /// endian, are 0x6384BA69.
    #[test]
    fn it_is_murmurhash3_x64_128() {
        let key: Vec<u8> = (0..=255u8).collect();
        let mut all = Vec::new();
        for n in 0..256usize {
            all.extend_from_slice(&hash(&key[..n], 256 - n as u32));
        }
        let last = hash(&all, 0);
        assert_eq!(
            u32::from_le_bytes(last[..4].try_into().unwrap()),
            0x6384_BA69
        );
    }

    #[test]
    fn streaming_in_pieces_is_hashing_whole() {
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        for cut in [0, 1, 7, 15, 16, 17, 33, 500, 999, 1000] {
            let mut h = Murmur128::new(0);
            h.update(&bytes[..cut]);
            h.update(&bytes[cut..]);
            assert_eq!(h.finish().to_le_bytes(), hash(&bytes, 0), "cut at {cut}");
        }
    }
}
