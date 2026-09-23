//! The page around a document: its `<head>`, its checkpoint, and the render
//! entry the web build runs.
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

use super::{escape, navigable, project, Document, DocumentError};
use crate::host::Host;
use exact_plan::{Plan, RenderPolicy};
use exact_runner::{DataSource, Runner};
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

/// The 1a checkpoint (LLP 1048.000 D6): the location, the render's time and
/// the resources still pending, as JSON a script element can carry — `<`,
/// `>` and `&` are escapes, so user text can never close the element or
/// open a comment. It has no answers yet: the runtime renders fresh.
pub fn checkpoint<D: DataSource>(runner: &Runner<D>, location: &str) -> String {
    let mut json = String::from("{\"location\":");
    crate::batch::quote(location, &mut json);
    let _ = write!(
        json,
        ",\"time\":{},\"pending\":[",
        exact_runner::agent::num(runner.now_ms())
    );
    for (i, (name, _)) in runner.pending().iter().enumerate() {
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

/// An app's pages as documents, for the web build and the parity check:
///
/// `<app>-render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>]
/// [--origin <url>] (--build | <location>…)`
///
/// boots a fresh host per location at the page viewport (the bake's 390 ×
/// 844 unless told) and prints one JSON line each: `location`, `notfound`,
/// `status`, `robots`, `root` (what `#exact-root` holds), `head` (what
/// `<head>` holds after the shell's charset and base) and `checkpoint`, or
/// `error`. `--build` renders every location the plan declares `render=build`
/// (see [`build_locations`]). `baked` is the app's own plan; the web build
/// passes the one it extracted from the shipped wasm instead.
#[cfg(not(target_arch = "wasm32"))]
pub fn main<D: DataSource + Default>(baked: &[u8]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let mut args = std::env::args().skip(1);
    let mut plan = baked.to_vec();
    let mut viewport = exact_runner::Viewport::default();
    let (mut name, mut origin) = (String::new(), None::<String>);
    let mut locations: Vec<(String, bool)> = Vec::new();
    let mut build = false;
    let usage = || {
        eprintln!("usage: render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>] [--origin <url>] (--build | <location>…)");
        ExitCode::from(2)
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plan" => match args.next().map(std::fs::read) {
                Some(Ok(bytes)) => plan = bytes,
                Some(Err(e)) => {
                    eprintln!("render: --plan: {e}");
                    return ExitCode::FAILURE;
                }
                None => return usage(),
            },
            "--viewport" => {
                let size = args.next().and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                });
                let Some((width, height)) = size else {
                    return usage();
                };
                viewport = exact_runner::Viewport { width, height };
            }
            "--name" => match args.next() {
                Some(value) => name = value,
                None => return usage(),
            },
            "--origin" => match args.next() {
                Some(value) => origin = Some(value),
                None => return usage(),
            },
            "--build" => build = true,
            _ if arg.starts_with('/') => locations.push((arg, false)),
            _ => return usage(),
        }
    }
    let decoded = match Plan::decode(&plan) {
        Ok(decoded) => decoded,
        Err(e) => {
            eprintln!("render: the plan: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    if build {
        match build_locations(&decoded) {
            Ok(found) => locations.extend(found),
            Err(e) => {
                eprintln!("render: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else if locations.is_empty() {
        return usage();
    }
    let site = Site {
        name: &name,
        origin: origin.as_deref(),
    };
    let mut failed = false;
    for (location, notfound) in &locations {
        let mut line = String::from("{\"location\":");
        crate::batch::quote(location, &mut line);
        let rendered = Host::boot(&plan, D::default(), viewport, location)
            .map_err(|e| e.to_string())
            .and_then(|(host, _)| {
                let doc = project(host.runner()).map_err(|e| e.to_string())?;
                let head = doc
                    .page_head(&decoded, &site, location)
                    .map_err(|e| e.to_string())?;
                Ok((doc, head, checkpoint(host.runner(), location)))
            });
        match rendered {
            Ok((doc, head, checkpoint)) => {
                let status = doc.head.status.unwrap_or(if *notfound { 404 } else { 200 });
                let _ = write!(
                    line,
                    ",\"notfound\":{notfound},\"status\":{status},\"robots\":"
                );
                match &doc.head.robots {
                    Some(robots) => crate::batch::quote(robots, &mut line),
                    None => line.push_str("null"),
                }
                for (field, value) in [
                    ("root", &doc.root),
                    ("head", &head),
                    ("checkpoint", &checkpoint),
                ] {
                    let _ = write!(line, ",\"{field}\":");
                    crate::batch::quote(value, &mut line);
                }
            }
            Err(error) => {
                failed = true;
                line.push_str(",\"error\":");
                crate::batch::quote(&error, &mut line);
            }
        }
        line.push('}');
        println!("{line}");
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
