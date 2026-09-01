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

use serde::Deserialize;
use sha2::{Digest, Sha256};

const ENVELOPE_TYPE: &str = "application/vnd.exact.envelope+json";
const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[allow(dead_code)]
struct Envelope {
    exact: u64,
    #[allow(dead_code)]
    app: Option<AppCard>,
    plan: PlanCard,
    seq: Option<u64>,
    events: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AppCard {
    id: Option<String>,
    name: String,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
#[allow(dead_code)]
struct PlanCard {
    url: String,
    sha256: String,
    bytes: u64,
    formatVersion: Option<u32>,
    kernelSchema: Option<String>,
}

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
    let (envelope, envelope_base) = if body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
    {
        (decode_envelope(&body)?, page.to_string())
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
        (decode_envelope(&body)?, url)
    };
    if envelope.exact != 1 {
        return Err(format!(
            "envelope version {} is newer than this host",
            envelope.exact
        ));
    }
    let count = usize::try_from(envelope.plan.bytes)
        .map_err(|_| "the envelope's plan byte count is too large".to_string())?;
    let url = join(&envelope_base, &envelope.plan.url)?;
    same_host(page, &url)?;
    let (status, bytes) = get(&url, "application/vnd.exact.plan")?;
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    if bytes.len() != count {
        return Err(format!(
            "plan is {} bytes, envelope said {count}; refusing it",
            bytes.len()
        ));
    }
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex != envelope.plan.sha256.to_lowercase() {
        return Err(format!("plan sha256 mismatch at {url}; refusing it"));
    }
    Ok(bytes)
}

fn decode_envelope(body: &[u8]) -> Result<Envelope, String> {
    if body.len() > MAX_ENVELOPE_BYTES {
        return Err(format!(
            "envelope exceeded the {MAX_ENVELOPE_BYTES}-byte limit"
        ));
    }
    serde_json::from_slice(body).map_err(|error| format!("invalid Exact envelope: {error}"))
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

/// Same-origin only in v1 (LLP 1023 D2): scheme, host, and effective port
/// must match the page's. Explicit default ports equal their implicit form.
fn same_host(page: &str, url: &str) -> Result<(), String> {
    if origin(page).is_some() && origin(page) == origin(url) {
        Ok(())
    } else {
        Err(format!("refused: {url} is not on {page}"))
    }
}

fn origin(url: &str) -> Option<(String, String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = if let Some(after) = authority.strip_prefix('[') {
        let close = after.find(']')?;
        let host = &after[..close];
        let suffix = &after[close + 1..];
        let port = if suffix.is_empty() {
            default
        } else {
            suffix.strip_prefix(':')?.parse().ok()?
        };
        (host, port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host, port.parse().ok()?)
    } else {
        (authority, default)
    };
    if host.is_empty() {
        return None;
    }
    Some((scheme, host.to_ascii_lowercase(), port))
}

#[cfg(test)]
mod tests {
    use super::{decode_envelope, same_host};

    #[test]
    fn origin_includes_scheme_host_and_effective_port() {
        assert!(same_host("http://EXAMPLE.test/app", "http://example.test:80/plan").is_ok());
        assert!(same_host("https://example.test/app", "https://example.test:443/plan").is_ok());
        assert!(same_host("http://example.test:8771/", "http://example.test:9999/plan").is_err());
        assert!(same_host(
            "http://example.test:8771/",
            "https://example.test:8771/plan"
        )
        .is_err());
    }

    #[test]
    fn envelope_json_is_typed_bounded_and_rejects_duplicates() {
        let pretty = br#"{
          "plan": { "kernelSchema": "00", "bytes": 12, "url": "./a\u002eplan", "sha256": "abcd", "formatVersion": 2, "compression": "none" },
          "exact": 1,
          "app": { "name": "Example", "id": "com.example", "subtitle": "future" },
          "assets": [{ "name": "mark.png", "url": "./mark.png" }],
          "seq": 41,
          "events": "./__dev",
          "future": { "enabled": true }
        }"#;
        let envelope = decode_envelope(pretty).unwrap();
        assert_eq!(envelope.plan.url, "./a.plan");
        assert_eq!(envelope.seq, Some(41));
        assert_eq!(envelope.events.as_deref(), Some("./__dev"));
        assert!(decode_envelope(
            br#"{"exact":1,"exact":1,"plan":{"url":"a","sha256":"b","bytes":1}}"#
        )
        .is_err());
        assert!(decode_envelope(
            br#"{"exact":1,"plan":{"url":"a","url":"b","sha256":"b","bytes":1}}"#
        )
        .is_err());
        assert!(decode_envelope(
            br#"{"exact":1,"plan":{"url":"a","sha256":"b","bytes":1},"seq":"new"}"#
        )
        .is_err());
        assert!(decode_envelope(b"not json").is_err());
        assert!(decode_envelope(&vec![b' '; 64 * 1024 + 1]).is_err());
    }
}
