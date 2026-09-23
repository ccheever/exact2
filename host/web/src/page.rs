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
    /// robots, OpenGraph and Twitter tags, and the plan's fonts.
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
/// answers — one list in the plan's value encoding, base64 — and what is
/// still pending. `<`, `>` and `&` are escapes, so user text can never close
/// the element or open a comment. [`read_checkpoint`] reads it back.
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
    let answers = checkpoint
        .answers
        .iter()
        .map(|(name, source, args, value)| {
            Value::record(vec![
                Value::str(name),
                Value::str(source),
                Value::list(args.clone()),
                value.clone(),
            ])
        })
        .collect();
    let _ = write!(
        json,
        ",\"answers\":\"{}\",\"pending\":[",
        exact_runner::agent::base64(&Value::list(answers).to_bytes())
    );
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
                checkpoint.now_ms = json.number()?.parse().map_err(|e| format!("time: {e}"))?
            }
            "logic" if json.bytes[json.at..].starts_with(b"null") => {
                json.at += 4;
                checkpoint.logic = None;
            }
            "logic" => checkpoint.logic = Some(json.string()?),
            "answers" => {
                let bytes = unbase64(&json.string()?).ok_or("answers: not base64")?;
                let list = Value::from_bytes(&bytes).map_err(|e| format!("answers: {e:?}"))?;
                let Value::List(items) = list else {
                    return Err("answers: not a list".into());
                };
                for item in items.iter() {
                    let answer = match item {
                        Value::Record(fields) => match fields.as_slice() {
                            [Value::Str(name), Value::Str(source), Value::List(args), value] => (
                                name.to_string(),
                                source.to_string(),
                                args.to_vec(),
                                value.clone(),
                            ),
                            _ => return Err("answers: not a (name, source, args, value)".into()),
                        },
                        _ => return Err("answers: not a record".into()),
                    };
                    checkpoint.answers.push(answer);
                }
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
        let checkpoint = match read_checkpoint(page) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                let (mut host, batch) = crate::Host::boot_delivered(
                    plan_bytes, data, None, snapshot, compat, viewport, launch,
                )?;
                host.log(&format!("document: the checkpoint doesn't read ({error})"));
                return Ok((host, batch));
            }
        };
        let plan = Plan::decode(plan_bytes).map_err(crate::HostError::Plan)?;
        let delivery = compat.map_or_else(Default::default, |json| {
            exact_runner::Delivery::default().with_compat(json)
        });
        let runner = Runner::boot_checkpoint(
            plan,
            data,
            exact_kernel::Kernel::with_monospace(),
            &checkpoint,
            snapshot,
            delivery,
            viewport,
            launch,
        )
        .map_err(crate::HostError::Runner)?;
        // The renderer's location, not this launch: the runtime's only input
        // of its own is its first tree, so a query the app doesn't read, or a
        // 404 page served at any path, still adopts the same document.
        let adopted = super::project(&runner).is_ok_and(|doc| {
            digest(runner.plan(), &checkpoint.location, page, &doc.root) == page_digest
        });
        let mut batch = crate::batch::Batch::new();
        batch.adopt(adopted);
        crate::Host::open(runner, launch, batch)
    }
}

/// Standard base64 with padding, as `exact_runner::agent::base64` writes it.
fn unbase64(text: &str) -> Option<Vec<u8>> {
    let digit = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || chunk[..4 - pad].contains(&b'=') {
            return None;
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - pad] {
            n = n << 6 | digit(c)? as u32;
        }
        n <<= 6 * pad as u32;
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Some(out)
}

/// The document's digest (LLP 1048.000 D6): SHA-256 over the plan's own
/// digest, the location the document was rendered at, the checkpoint as
/// the page carries it, and the document — each length-prefixed, so no two
/// different inputs share a preimage. The renderer writes it beside the
/// checkpoint; the runtime computes it with its own first tree for the
/// document and adopts the page on a match.
pub fn digest(plan: &Plan, location: &str, checkpoint: &str, document: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(Sha256::digest(plan.encode()));
    for part in [location, checkpoint, document] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    hash.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The locations a build renders: every route declared `render=build`
/// (their patterns have no parameters; the compiler refuses one that does),
/// and the not-found document when that route is declared so. The
/// not-found document renders at `/404.html`, which no pattern may match.
pub fn build_locations(plan: &Plan) -> Result<Vec<(String, bool)>, String> {
    let table = exact_route::Table {
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
    };
    let mut out = Vec::new();
    for row in plan
        .routes
        .iter()
        .filter(|r| r.render == RenderPolicy::Build)
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
