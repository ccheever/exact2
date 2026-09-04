//! The envelope: the pointer card a stream publishes, its canonical bytes, and
//! the signature over them.
//!
//! @ref LLP 1023 D2 (the envelope; relative URLs; unknown majors refused,
//! unknown fields ignored) / LLP 1026 D11 (the Ed25519 signature over the
//! canonical bytes) / LLP 1030 D3a (the head binds app, channel, compatibility
//! id, `seq`, and key id) / D5 (the sunset card)
//!
//! The form this crate reads, LLP 1023 D2 extended by 1026 D9/D11 and 1030
//! D3a:
//!
//! ```json
//! {
//!   "exact": 1,
//!   "app": { "id": "com.exact.caltrain", "name": "Caltrain" },
//!   "plan": { "url": "./app.plan", "sha256": "<hex>", "bytes": 12580,
//!             "formatVersion": 4, "kernelSchema": "<u64 hex>" },
//!   "assets": [ { "name": "mark.png", "url": "./assets/mark.png",
//!                 "sha256": "<hex>", "bytes": 4211 } ],
//!   "stream": { "app": "com.exact.caltrain", "channel": "release",
//!               "compatibilityId": "<hex>", "seq": 41 },
//!   "release": "2026-09-03T18:04:11Z-7f1c",
//!   "sunset": { "message": "…", "store": "https://…" },
//!   "signature": { "keyId": "k1", "ed25519": "<base64 of 64 bytes>" }
//! }
//! ```
//!
//! `stream` is required here and nowhere else: a bundle in an update store
//! always belongs to a cohort's stream, which is what makes a head unreplayable
//! onto another channel (LLP 1030 D3a). `sunset` and `signature` are optional;
//! every other member of an unknown name is ignored.

use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes`, lowercase hex — payload integrity as LLP 1023 D2
/// defines it. Bundle identity instead hashes [`canonical_bytes`].
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The bytes an envelope's signature covers (LLP 1026 D11; LLP 1030 D3a).
///
/// The rule, stated so that a publisher written in another language produces
/// the same bytes: take the envelope, **remove the top-level `signature`
/// member**, and serialize what is left as JSON with
///
/// - every object's keys sorted ascending by their UTF-8 bytes, recursively;
/// - arrays in the order they are written;
/// - **no whitespace anywhere** — nothing after `:` or `,`, no trailing
///   newline;
/// - strings escaped the way `JSON.stringify` escapes them: `"` and `\` and
///   the C0 controls, of which `\b` `\f` `\n` `\r` `\t` go by name and the rest
///   as `\u00xx`; every other character is its literal UTF-8, so no `\u`
///   escapes above ASCII and no escaped `/`;
/// - numbers as their shortest integer form — no `+`, no `-0`, no exponent, no
///   fraction. **A non-integer number is refused**, here and at the publisher,
///   so that no float-formatting rule has to be agreed between two languages.
///   Nothing in an envelope is fractional: `seq` and every byte count are
///   integers.
///
/// The Node publisher emits each sorted key/value pair directly. It cannot
/// rebuild an object and then call `JSON.stringify`: JavaScript always
/// enumerates integer-like keys numerically, which would undo lexical order
/// for an otherwise valid unknown field such as `{ "10": …, "2": … }`.
///
/// The signature therefore covers the app id, the channel, the compatibility
/// id, the `seq`, and every file digest: a head cannot be replayed onto another
/// stream, and a bundle's files cannot be swapped under it.
pub fn canonical_bytes(envelope_json: &str) -> Result<Vec<u8>, String> {
    let value: serde_json::Value = serde_json::from_str(envelope_json)
        .map_err(|e| format!("the envelope is not JSON: {e}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "the envelope is not a JSON object".to_string())?;
    let mut without = object.clone();
    without.remove("signature");
    let mut out = String::new();
    canonical(&serde_json::Value::Object(without), &mut out)?;
    Ok(out.into_bytes())
}

/// One value, canonically: sorted keys, no whitespace, integers only.
fn canonical(value: &serde_json::Value, out: &mut String) -> Result<(), String> {
    use serde_json::Value;
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.as_u64().is_none() && n.as_i64().is_none() {
                return Err(format!(
                    "the envelope carries the non-integer number {n}; canonical bytes are integers only"
                ));
            }
            out.push_str(&n.to_string());
        }
        Value::String(s) => out.push_str(
            &serde_json::to_string(s).map_err(|e| format!("a string will not serialize: {e}"))?,
        ),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(
                    &serde_json::to_string(key)
                        .map_err(|e| format!("a key will not serialize: {e}"))?,
                );
                out.push(':');
                canonical(&map[*key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// The sunset card (LLP 1030 D5): a stream's advisory retirement notice. A
/// client that still checks shows the message and the link; nothing is forced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// What to tell the person using the app.
    pub message: String,
    /// Where a new binary lives, if the developer named a place.
    pub store: Option<String>,
}

/// One file the envelope names by digest: the plan, or an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCard {
    /// The asset's name, as the plan names it; `app.plan` for the plan.
    pub name: String,
    /// Where to fetch it, absolute or relative to the envelope's URL.
    pub url: String,
    /// SHA-256 of the bytes, lowercase hex — verified on arrival.
    pub sha256: String,
    /// How many bytes, exactly. A body of another length is refused.
    pub bytes: u64,
}

/// The stream this envelope heads (LLP 1030 D3a): `(app, channel,
/// compatibility id)`, with its own monotonic `seq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCard {
    /// The app id the stream belongs to, when the head states it again.
    pub app: Option<String>,
    /// The channel — `release`, `beta`, whatever the developer publishes to.
    pub channel: String,
    /// The cohort this bundle is safe for (LLP 1030 D3a).
    pub compatibility_id: String,
    /// Monotonic per stream. A client refuses anything below what it selected.
    pub seq: u64,
}

/// The signature over the canonical bytes (LLP 1026 D11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Which embedded key signed it.
    pub key_id: String,
    /// The 64 raw Ed25519 bytes, decoded from the head's base64.
    pub bytes: [u8; 64],
}

/// A parsed envelope: the head of a stream, or the `exact.json` of an entry in
/// the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// The bytes as they arrived — what the entry stores for later signature
    /// verification.
    pub raw: Vec<u8>,
    /// SHA-256 of the authenticated canonical bytes, lowercase hex: the
    /// entry's name. Transport whitespace, key order, and the signature's own
    /// encoding cannot give one signed bundle another crash-quarantine name.
    pub digest: String,
    /// The app this bundle belongs to (LLP 1023 D5).
    pub app_id: String,
    /// The name people see, when the envelope carries one.
    pub app_name: Option<String>,
    /// The plan card. Its `name` is `app.plan`.
    pub plan: FileCard,
    /// The format version the plan header carries, when stated (LLP 1023 D2).
    pub format_version: Option<u64>,
    /// The kernel schema digest, when stated: a client may refuse before
    /// downloading, though the compatibility id already covers it.
    pub kernel_schema: Option<String>,
    /// Every asset, by name and digest (LLP 1026 D11).
    pub assets: Vec<FileCard>,
    /// The stream (LLP 1030 D3a).
    pub stream: StreamCard,
    /// The release id: one deploy across every stream it touched.
    pub release: Option<String>,
    /// The sunset card, when the stream is retiring (LLP 1030 D5).
    pub sunset: Option<Card>,
    /// The signature, when the head carries one.
    pub signature: Option<Signature>,
}

impl Envelope {
    /// Parse an envelope from the bytes as fetched or as stored.
    ///
    /// Refuses: a body over [`MAX_ENVELOPE_BYTES`], anything that is not UTF-8
    /// JSON, an `exact` major this crate does not know, a missing or malformed
    /// `app.id`, `plan`, or `stream`, an asset name that is not a safe relative
    /// path, and a non-integer number anywhere (the canonical-bytes rule).
    ///
    /// [`MAX_ENVELOPE_BYTES`]: crate::MAX_ENVELOPE_BYTES
    pub fn parse(raw: &[u8]) -> Result<Envelope, String> {
        if raw.len() > crate::MAX_ENVELOPE_BYTES {
            return Err(format!(
                "the envelope is {} bytes; the most is {}",
                raw.len(),
                crate::MAX_ENVELOPE_BYTES
            ));
        }
        let text =
            std::str::from_utf8(raw).map_err(|e| format!("the envelope is not UTF-8: {e}"))?;
        // Canonicalizing here is the parse's own check that the head can be
        // signed and verified at all: it refuses a non-integer number before
        // anything downstream depends on the bytes.
        let canonical = canonical_bytes(text)?;
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("the envelope is not JSON: {e}"))?;
        let root = value
            .as_object()
            .ok_or_else(|| "the envelope is not a JSON object".to_string())?;
        let major = root
            .get("exact")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "the envelope declares no `exact` major".to_string())?;
        if major != crate::ENVELOPE_MAJOR {
            return Err(format!(
                "the envelope is exact {major}; this binary reads exact {}",
                crate::ENVELOPE_MAJOR
            ));
        }
        let app = root
            .get("app")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "the envelope names no app".to_string())?;
        let app_id = app
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "the envelope names no app id".to_string())?
            .to_string();
        if app_id.is_empty() {
            return Err("the envelope's app id is empty".into());
        }
        let plan_object = root
            .get("plan")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "the envelope names no plan".to_string())?;
        let plan = file_card("app.plan", plan_object)?;
        let stream_object = root
            .get("stream")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "the envelope names no stream (LLP 1030 D3a)".to_string())?;
        let stream = StreamCard {
            app: stream_object
                .get("app")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            channel: stream_object
                .get("channel")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "the stream names no channel".to_string())?
                .to_string(),
            compatibility_id: stream_object
                .get("compatibilityId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "the stream names no compatibility id".to_string())?
                .to_string(),
            seq: stream_object
                .get("seq")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "the stream names no seq".to_string())?,
        };
        let mut assets = Vec::new();
        if let Some(list) = root.get("assets") {
            let list = list
                .as_array()
                .ok_or_else(|| "the envelope's assets are not a list".to_string())?;
            for item in list {
                let object = item
                    .as_object()
                    .ok_or_else(|| "an asset is not an object".to_string())?;
                let name = object
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "an asset has no name".to_string())?;
                safe_name(name)?;
                if assets.iter().any(|a: &FileCard| a.name == name) {
                    return Err(format!("the envelope names the asset {name} twice"));
                }
                assets.push(file_card(name, object)?);
            }
        }
        let sunset = match root.get("sunset") {
            None => None,
            Some(v) => {
                let object = v
                    .as_object()
                    .ok_or_else(|| "the sunset card is not an object".to_string())?;
                Some(Card {
                    message: object
                        .get("message")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| "the sunset card has no message".to_string())?
                        .to_string(),
                    store: object
                        .get("store")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                })
            }
        };
        let signature = match root.get("signature") {
            None => None,
            Some(v) => {
                let object = v
                    .as_object()
                    .ok_or_else(|| "the signature is not an object".to_string())?;
                let key_id = object
                    .get("keyId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "the signature names no key id".to_string())?
                    .to_string();
                let encoded = object
                    .get("ed25519")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "the signature carries no ed25519 member".to_string())?;
                let decoded = base64_decode(encoded)?;
                let bytes: [u8; 64] = decoded.as_slice().try_into().map_err(|_| {
                    format!(
                        "the signature is {} bytes; an Ed25519 signature is 64",
                        decoded.len()
                    )
                })?;
                Some(Signature { key_id, bytes })
            }
        };
        Ok(Envelope {
            raw: raw.to_vec(),
            digest: sha256_hex(&canonical),
            app_id,
            app_name: app.get("name").and_then(|v| v.as_str()).map(str::to_string),
            plan,
            format_version: plan_object.get("formatVersion").and_then(|v| v.as_u64()),
            kernel_schema: plan_object
                .get("kernelSchema")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            assets,
            stream,
            release: root
                .get("release")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            sunset,
            signature,
        })
    }

    /// Verify the head against the binary's embedded keys (LLP 1026 D11).
    ///
    /// A binary with keys refuses an unsigned head and one signed by a key it
    /// does not carry — the CDN is untrusted, and a bundle whose signature does
    /// not verify is never written to the store. A binary with **no** keys is a
    /// dev binary: it admits an unsigned head, and ignores a signature it
    /// cannot check. Key rotation is a new binary, and a new trust epoch moves
    /// the compatibility id, so this needs no negotiation.
    pub fn verify(&self, keys: &[(String, [u8; 32])]) -> Result<(), String> {
        if keys.is_empty() {
            return Ok(());
        }
        let signature = self
            .signature
            .as_ref()
            .ok_or_else(|| "the head is unsigned and this binary carries keys".to_string())?;
        let key = keys
            .iter()
            .find(|(id, _)| *id == signature.key_id)
            .ok_or_else(|| {
                format!(
                    "the head is signed by {}, which this binary does not carry",
                    signature.key_id
                )
            })?;
        let verifying = VerifyingKey::from_bytes(&key.1)
            .map_err(|e| format!("the embedded key {} is not a key: {e}", key.0))?;
        let text = std::str::from_utf8(&self.raw)
            .map_err(|e| format!("the envelope is not UTF-8: {e}"))?;
        let canonical = canonical_bytes(text)?;
        verifying
            .verify_strict(&canonical, &Ed25519Signature::from_bytes(&signature.bytes))
            .map_err(|_| format!("the head's signature by {} does not verify", key.0))
    }
}

fn file_card(
    name: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<FileCard, String> {
    let url = object
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{name} has no url"))?
        .to_string();
    let sha256 = object
        .get("sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{name} has no sha256"))?
        .to_string();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{name}'s sha256 is not 64 hex digits"));
    }
    if sha256.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(format!("{name}'s sha256 is not lowercase"));
    }
    let bytes = object
        .get("bytes")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("{name} has no byte count"))?;
    Ok(FileCard {
        name: name.to_string(),
        url,
        sha256,
        bytes,
    })
}

/// An asset name is a relative path under the entry's `assets/`: no root, no
/// `..`, no drive letter, no empty segment, no backslash. A name that could
/// leave the entry is refused before anything is written.
pub(crate) fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("an asset name is empty".into());
    }
    if name.starts_with('/') || name.starts_with('\\') || name.contains('\\') || name.contains(':')
    {
        return Err(format!("the asset name {name} is not a relative path"));
    }
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("the asset name {name} is not a relative path"));
        }
    }
    Ok(())
}

/// Resolve a card's `url` against the envelope's own URL, the way a browser
/// resolves against the document URL (LLP 1023 D1/D2). `http` and `https`
/// only, and the result must have the base URL's scheme, host, and effective
/// port: every file card is same-origin in v1. Credentials are refused rather
/// than copied into a request.
pub fn resolve_url(base: &str, url: &str) -> Result<String, String> {
    if base.contains('\\') || url.contains('\\') {
        return Err(format!("the url {url} is not an http or https URL"));
    }
    let base = parse_http_url(base, "the envelope url")?;
    if url.contains("://") || url.starts_with("//") {
        let absolute = if url.starts_with("//") {
            format!("{}:{url}", base.scheme)
        } else {
            url.to_string()
        };
        let resolved = parse_http_url(&absolute, "the file url")?;
        if resolved.origin() != base.origin() {
            return Err(format!(
                "the url {url} is cross-origin; file cards must stay on {}://{}",
                base.scheme, base.authority
            ));
        }
        return Ok(resolved.without_fragment());
    }
    if url.contains(':') && !url.starts_with('/') {
        // `mailto:`, `data:`, `file:` — anything with a scheme and no origin.
        return Err(format!("the url {url} is not http or https"));
    }
    let (reference, fragment) = url.split_once('#').unwrap_or((url, ""));
    let (reference_path, query) = reference.split_once('?').unwrap_or((reference, ""));
    let (base_path, _) = base.path.split_once('?').unwrap_or((base.path, ""));
    let joined = if let Some(absolute) = reference_path.strip_prefix('/') {
        format!("/{absolute}")
    } else if reference_path.is_empty() {
        base_path.to_string()
    } else {
        let directory = &base_path[..=base_path.rfind('/').unwrap_or(0)];
        format!("{directory}{reference_path}")
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(format!("the url {url} climbs above the origin"));
                }
            }
            other => segments.push(other),
        }
    }
    let mut out = format!(
        "{}://{}/{}",
        base.scheme,
        base.authority,
        segments.join("/")
    );
    if reference.contains('?') {
        out.push('?');
        out.push_str(query);
    }
    let _ = fragment; // Fragments identify no bytes and are never sent.
    Ok(out)
}

/// The pieces needed to compare HTTP origins without accepting credentials or
/// confusing an explicit default port with another origin.
struct HttpUrl<'a> {
    scheme: &'a str,
    authority: &'a str,
    host: String,
    port: u16,
    path: &'a str,
}

impl HttpUrl<'_> {
    fn origin(&self) -> (&str, &str, u16) {
        (self.scheme, self.host.as_str(), self.port)
    }

    fn without_fragment(&self) -> String {
        let path = self
            .path
            .split_once('#')
            .map_or(self.path, |(path, _)| path);
        format!("{}://{}{}", self.scheme, self.authority, path)
    }
}

fn parse_http_url<'a>(text: &'a str, what: &str) -> Result<HttpUrl<'a>, String> {
    let (scheme, rest) = text
        .split_once("://")
        .ok_or_else(|| format!("{what} {text} has no scheme"))?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("{what} {text} is not http or https"));
    }
    let boundary = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..boundary];
    if authority.is_empty() || authority.contains('@') {
        return Err(format!(
            "{what} {text} has an invalid or credentialed authority"
        ));
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let close = bracketed
            .find(']')
            .ok_or_else(|| format!("{what} {text} has an invalid IPv6 host"))?;
        let host = format!("[{}]", &bracketed[..close]);
        let suffix = &bracketed[close + 1..];
        let port = if suffix.is_empty() {
            default_port(scheme)
        } else {
            parse_port(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| format!("{what} {text} has an invalid authority"))?,
                what,
                text,
            )?
        };
        (host.to_ascii_lowercase(), port)
    } else {
        if authority.matches(':').count() > 1 {
            return Err(format!("{what} {text} has an unbracketed IPv6 host"));
        }
        match authority.rsplit_once(':') {
            Some((host, port)) => {
                if host.is_empty() {
                    return Err(format!("{what} {text} has no host"));
                }
                (host.to_ascii_lowercase(), parse_port(port, what, text)?)
            }
            None => (authority.to_ascii_lowercase(), default_port(scheme)),
        }
    };
    if host.is_empty() || host.chars().any(char::is_whitespace) {
        return Err(format!("{what} {text} has no valid host"));
    }
    let path = if boundary == rest.len() {
        "/"
    } else {
        &rest[boundary..]
    };
    Ok(HttpUrl {
        scheme,
        authority,
        host,
        port,
        path,
    })
}

fn default_port(scheme: &str) -> u16 {
    if scheme == "https" {
        443
    } else {
        80
    }
}

fn parse_port(port: &str, what: &str, text: &str) -> Result<u16, String> {
    if port.is_empty() {
        return Err(format!("{what} {text} has an empty port"));
    }
    port.parse::<u16>()
        .map_err(|_| format!("{what} {text} has an invalid port"))
}

/// Standard base64 with padding, decoded strictly: the signature is 64 bytes
/// and a baked verification key 32, and nothing else is encoded, so this is
/// the whole need and costs no dependency.
pub(crate) fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    fn sextet(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err("the signature is not padded base64".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let last = bytes.len() / 4 - 1;
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let padding = chunk.iter().filter(|b| **b == b'=').count();
        if padding > 2 || (padding > 0 && index != last) {
            return Err("the signature is not padded base64".into());
        }
        let mut acc: u32 = 0;
        for (i, b) in chunk.iter().enumerate() {
            let value = if *b == b'=' {
                if i < 4 - padding {
                    return Err("the signature is not padded base64".into());
                }
                0
            } else {
                sextet(*b).ok_or_else(|| "the signature is not base64".to_string())?
            };
            acc = (acc << 6) | u32::from(value);
        }
        let decoded = acc.to_be_bytes();
        out.extend_from_slice(&decoded[1..4 - padding]);
    }
    Ok(out)
}

#[cfg(test)]
mod url_tests {
    use super::resolve_url;

    #[test]
    fn file_cards_resolve_only_within_the_envelopes_origin() {
        let base = "https://Updates.Example:443/.exact/prod/cohort/exact.json";
        assert_eq!(
            resolve_url(base, "./blobs/plan?download=1#ignored").unwrap(),
            "https://Updates.Example:443/.exact/prod/cohort/blobs/plan?download=1"
        );
        assert_eq!(
            resolve_url(base, "/.exact/blobs/asset").unwrap(),
            "https://Updates.Example:443/.exact/blobs/asset"
        );
        assert_eq!(
            resolve_url(base, "https://updates.example/a").unwrap(),
            "https://updates.example/a"
        );
        assert_eq!(
            resolve_url(base, "//updates.example:443/a").unwrap(),
            "https://updates.example:443/a"
        );

        for refused in [
            "https://elsewhere.example/a",
            "https://updates.example:444/a",
            "http://updates.example/a",
            "//elsewhere.example/a",
            "https://user:secret@updates.example/a",
            "file:///tmp/plan",
            "data:text/plain,no",
        ] {
            assert!(
                resolve_url(base, refused).is_err(),
                "cross-origin or credentialed card was admitted: {refused}"
            );
        }
    }

    #[test]
    fn relative_cards_cannot_escape_or_smuggle_a_url() {
        let base = "http://127.0.0.1:8000/a/b/exact.json";
        assert_eq!(
            resolve_url(base, "../app.plan").unwrap(),
            "http://127.0.0.1:8000/a/app.plan"
        );
        assert!(resolve_url(base, "../../../../app.plan").is_err());
        assert!(resolve_url(base, r"..\app.plan").is_err());
        assert!(resolve_url("https://user@updates.example/head", "./plan").is_err());
    }
}
