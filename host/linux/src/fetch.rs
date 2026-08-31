//! The network form of the dev plan (LLP 1023 Stage 1, Linux): given the
//! app URL — the one a browser opens — resolve the envelope, negotiated or
//! through `index.html`'s `rel=alternate` link, fetch the plan, verify its
//! length and SHA-256, and hand back the bytes to boot. One shot, at
//! process start: a headless run is per-invocation (the fleet's shape —
//! run, read, run again), and the display loop's live SSE half waits for a
//! Linux display to verify it on (QUEUE).
//!
//! The transport is ibex2's default — rustls off Apple, the platform's on a
//! Mac — used directly: grants govern *app* authority (ibex LLP 0067); this
//! is host apparatus, the same standing as the Apple host's URLSession use
//! in `PlanURL.swift`. Redirects are not followed in v1: the error names
//! the limitation and the direct URL fixes it. Every line goes to stderr —
//! in agent mode stdout is the protocol.

use sha2::{Digest, Sha256};

const ENVELOPE_TYPE: &str = "application/vnd.exact.envelope+json";

/// The env locator accepts a path or a URL; this decides which.
pub fn is_url(v: &str) -> bool {
    v.starts_with("http://") || v.starts_with("https://")
}

/// Resolve `page` and return the verified plan bytes.
pub fn fetch_app(page: &str) -> Result<Vec<u8>, String> {
    let transport = ibex2::transport::default_transport();
    let get = |url: &str, accept: &str| -> Result<(u16, Vec<u8>), String> {
        let mut req = ibex2::stdlib::fetch::Request::get(url);
        req.headers.set("accept", accept);
        req.headers.set("cache-control", "no-cache");
        let r = transport.send(&req).map_err(|e| format!("{url}: {e}"))?;
        Ok((r.status, r.body))
    };
    let (status, body) = get(page, &format!("{ENVELOPE_TYPE}, text/html;q=0.9"))?;
    if status != 200 {
        return Err(format!("{page}: HTTP {status}"));
    }
    let envelope = if body.first() == Some(&b'{') {
        String::from_utf8_lossy(&body).into_owned()
    } else {
        // The link rung: a bounded, parser-free scan of the page's first
        // 16 KB (exact1 LLP 0268's rule) for the alternate-representation
        // link, resolved against the page URL. No redirects, so the page
        // URL is the base.
        let href = envelope_link(&body)
            .ok_or_else(|| format!("{page} is not an Exact app: no envelope and no link"))?;
        let url = join(page, &href)?;
        same_host(page, &url)?;
        let (status, body) = get(&url, ENVELOPE_TYPE)?;
        if status != 200 {
            return Err(format!("{url}: HTTP {status}"));
        }
        String::from_utf8_lossy(&body).into_owned()
    };
    let major =
        int_field(&envelope, "\"exact\":").ok_or("no \"exact\" version — not an envelope")?;
    if major != 1 {
        return Err(format!("envelope version {major} is newer than this host"));
    }
    let plan = envelope
        .find("\"plan\":")
        .map(|at| &envelope[at..])
        .ok_or("the envelope names no plan")?;
    let path = str_field(plan, "\"url\":\"").ok_or("the envelope's plan has no url")?;
    let sha = str_field(plan, "\"sha256\":\"").ok_or("the envelope's plan has no sha256")?;
    let count = int_field(plan, "\"bytes\":").ok_or("the envelope's plan has no bytes")?;
    let url = join(page, &path)?;
    same_host(page, &url)?;
    let (status, bytes) = get(&url, "application/vnd.exact.plan")?;
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    if bytes.len() as i64 != count {
        return Err(format!(
            "plan is {} bytes, envelope said {count}; refusing it",
            bytes.len()
        ));
    }
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex != sha.to_lowercase() {
        return Err(format!("plan sha256 mismatch at {url}; refusing it"));
    }
    Ok(bytes)
}

fn envelope_link(html: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(&html[..html.len().min(16384)]);
    let at = text.find(ENVELOPE_TYPE)?;
    let open = text[..at].rfind("<link")?;
    let close = at + text[at..].find('>')?;
    let tag = &text[open..close];
    for quote in ['"', '\''] {
        let needle = format!("href={quote}");
        if let Some(h) = tag.find(&needle) {
            let rest = &tag[h + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].replace("&amp;", "&"));
            }
        }
    }
    None
}

/// `href` against `base` — absolute kept, `/path` from the origin,
/// `./name` and `name` beside the base's last segment. Enough for the
/// envelope the build emits; not a general resolver.
fn join(base: &str, href: &str) -> Result<String, String> {
    if is_url(href) {
        return Ok(href.to_string());
    }
    let scheme_end = base
        .find("://")
        .ok_or_else(|| format!("{base}: not a URL"))?
        + 3;
    let origin_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |i| scheme_end + i);
    if let Some(rooted) = href.strip_prefix('/') {
        return Ok(format!("{}/{rooted}", &base[..origin_end]));
    }
    let path_end = base.find(['?', '#']).unwrap_or(base.len());
    let dir_end = base[..path_end]
        .rfind('/')
        .map_or(path_end, |i| i.max(origin_end));
    Ok(format!(
        "{}/{}",
        &base[..dir_end.max(origin_end)],
        href.strip_prefix("./").unwrap_or(href)
    ))
}

/// Same-origin only in v1 (LLP 1023 D2): host and port must match the page's.
fn same_host(page: &str, url: &str) -> Result<(), String> {
    let host = |u: &str| {
        u.split_once("://").map(|(_, rest)| {
            rest.split(['/', '?', '#'])
                .next()
                .unwrap_or(rest)
                .to_string()
        })
    };
    if host(page) == host(url) {
        Ok(())
    } else {
        Err(format!("refused: {url} is not on {page}"))
    }
}

fn str_field(json: &str, needle: &str) -> Option<String> {
    let at = json.find(needle)? + needle.len();
    let end = at + json[at..].find('"')?;
    Some(json[at..end].to_string())
}

fn int_field(json: &str, needle: &str) -> Option<i64> {
    let at = json.find(needle)? + needle.len();
    let digits: String = json[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}
