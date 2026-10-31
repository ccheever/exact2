//! The browser's path, query and fragment serializers, without a host parser.
//!
//! @ref LLP 1038 §3 — Location and the searchParam read; D1, D3, D8; LLP 1153 D5.
//!
//! Locations are absolute paths appended to `https://x`, never resolved as
//! network-path references. Backslashes are path separators and the query
//! uses the special-query set (it includes `'`). Existing percent escapes
//! keep their spelling; only parameter formatting uses encodeURIComponent.
//! A fragment rides beside a location's identity, never in it.

use crate::Entry;

/// The browser's `pathname + search` for `https://x` plus an absolute path.
/// A missing leading slash is prefixed before parsing; fragments disappear,
/// and an empty query contributes no `?`.
pub fn canonical(location: &str) -> String {
    let absolute = if location.starts_with('/') {
        location.to_owned()
    } else {
        format!("/{location}")
    };
    path_query(&clean(&absolute))
}

/// The browser's `hash` without its `#`: what follows the first `#`, with
/// the fragment percent-encode set applied (WHATWG URL: C0 controls, space,
/// `"`, `<`, `>`, `` ` `` and every non-ASCII byte); `""` when there is none.
pub fn fragment_of(location: &str) -> String {
    clean(location)
        .split_once('#')
        .map(|(_, fragment)| {
            encode(fragment, |b| {
                !(0x21..=0x7e).contains(&b) || matches!(b, b'"' | b'<' | b'>' | b'`')
            })
        })
        .unwrap_or_default()
}

/// Derive a location from a host's incoming URL, its fragment kept. For
/// non-HTTP schemes the authority is the first path segment, so
/// `interview://post/42` is `/post/42`. A scheme without `//` uses
/// everything after `:` as that path (e.g. mailto).
pub fn location_of(href: &str) -> String {
    let input = clean(href);
    let location = match scheme(&input) {
        Some((name, after))
            if !name.eq_ignore_ascii_case("http") && !name.eq_ignore_ascii_case("https") =>
        {
            path_query(&format!("/{}", after.strip_prefix("//").unwrap_or(after)))
        }
        Some((_, after)) => path_query(without_authority(after.trim_start_matches(['/', '\\']))),
        None => canonical(&input),
    };
    match fragment_of(&input) {
        fragment if fragment.is_empty() => location,
        fragment => format!("{location}#{fragment}"),
    }
}

/// A declared Web App Manifest protocol handler's in-app location. The
/// placeholder carries the original URL as data, including its fragment.
pub fn protocol_location_of(href: &str, protocol: &str, template: &str) -> Option<String> {
    let (name, _) = scheme(href)?;
    if !name.eq_ignore_ascii_case(protocol)
        || !template.starts_with('/')
        || template.starts_with("//")
        || template.contains('#')
    {
        return None;
    }
    let (before, after) = template.split_once("%s")?;
    if after.contains("%s") {
        return None;
    }
    Some(canonical(&format!(
        "{before}{}{after}",
        encode_uri_component(href)
    )))
}

fn clean(input: &str) -> String {
    input
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

fn scheme(input: &str) -> Option<(&str, &str)> {
    let (name, rest) = input.split_once(':')?;
    let mut bytes = name.bytes();
    if !bytes.next()?.is_ascii_alphabetic()
        || !bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return None;
    }
    Some((name, rest))
}

fn without_authority(input: &str) -> &str {
    let end = input.find(['/', '\\', '?', '#']).unwrap_or(input.len());
    &input[end..]
}

fn path_query(input: &str) -> String {
    let input = input.split('#').next().unwrap_or("");
    let (path, query) = input.split_once('?').unwrap_or((input, ""));
    let path = path.replace('\\', "/");
    let path = path.strip_prefix('/').unwrap_or(&path);
    let mut segments = Vec::new();
    let mut iter = path.split('/').peekable();
    while let Some(segment) = iter.next() {
        let dot = segment.to_ascii_lowercase();
        match dot.as_str() {
            "." | "%2e" => {
                if iter.peek().is_none() {
                    segments.push(String::new());
                }
            }
            ".." | ".%2e" | "%2e." | "%2e%2e" => {
                segments.pop();
                if iter.peek().is_none() {
                    segments.push(String::new());
                }
            }
            _ => segments.push(encode(segment, |b| {
                !(0x21..=0x7e).contains(&b)
                    || matches!(
                        b,
                        b'"' | b'#' | b'<' | b'>' | b'?' | b'^' | b'`' | b'{' | b'}' | b'|'
                    )
            })),
        }
    }
    let mut out = String::from("/");
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(segment);
    }
    if !query.is_empty() {
        out.push('?');
        out.push_str(&encode(query, |b| {
            !(0x21..=0x7e).contains(&b) || matches!(b, b'"' | b'#' | b'<' | b'>' | b'\'')
        }));
    }
    out
}

/// JavaScript's encodeURIComponent for a Rust string (D3's path expansion).
/// A slash, percent sign or question mark in a parameter is data, not syntax.
pub fn encode_uri_component(value: &str) -> String {
    encode(value, |b| {
        !b.is_ascii_alphanumeric()
            && !matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
    })
}

/// Every caller escapes every non-ASCII byte, so a run of kept bytes is ASCII
/// and is copied whole.
fn encode(input: &str, escape: impl Fn(u8) -> bool) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len());
    let mut kept = 0;
    for (i, b) in input.bytes().enumerate() {
        if escape(b) {
            if kept < i {
                out.push_str(&input[kept..i]);
            }
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 15) as usize] as char);
            kept = i + 1;
        }
    }
    if kept < input.len() {
        out.push_str(&input[kept..]);
    }
    out
}

/// The `id` a fragment names when it is not one as written (HTML, "find a
/// potential indicated element"): percent-decoded, then read as UTF-8 with
/// no BOM stripped; `None` when the bytes are not UTF-8.
pub fn fragment_id(fragment: &str) -> Option<String> {
    String::from_utf8(decode_bytes(fragment, false)).ok()
}

pub(crate) fn decode(input: &str, plus: bool) -> String {
    String::from_utf8_lossy(&decode_bytes(input, plus)).into_owned()
}

fn decode_bytes(input: &str, plus: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let mut look = bytes.clone();
            if let Some(value) = look
                .next()
                .and_then(hex)
                .zip(look.next().and_then(hex))
                .map(|(a, b)| a * 16 + b)
            {
                bytes = look;
                out.push(value);
                continue;
            }
        }
        out.push(if plus && b == b'+' { b' ' } else { b });
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|n| n as u8)
}

/// URLSearchParams.get: first occurrence, form decoding (`+` is a space),
/// replacement characters for invalid UTF-8, and `""` when absent.
pub fn search_param(entry: &Entry, name: &str) -> String {
    let Some((_, query)) = entry.url.split_once('?') else {
        return String::new();
    };
    for pair in query
        .split('#')
        .next()
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty())
    {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if decode(key, true) == name {
            return decode(value, true);
        }
    }
    String::new()
}

#[cfg(test)]
mod protocol_tests {
    use super::*;

    #[test]
    fn declared_protocol_template_preserves_the_whole_url() {
        let href = "exactreprox05://auth/x?request=1#frag";
        let expected = "/open?u=exactreprox05%3A%2F%2Fauth%2Fx%3Frequest%3D1%23frag";
        assert_eq!(
            protocol_location_of(href, "exactreprox05", "/open?u=%s"),
            Some(expected.into())
        );
        assert_eq!(
            protocol_location_of(href, "EXACTREPROX05", "/open?u=%s"),
            Some(expected.into())
        );
        assert_eq!(protocol_location_of(href, "other", "/open?u=%s"), None);
        assert_eq!(protocol_location_of(href, "exactreprox05", "/open"), None);

        let long = format!("exactreprox05://auth/x?request={}#frag", "x".repeat(16_384));
        let location = protocol_location_of(&long, "exactreprox05", "/open?u=%s").unwrap();
        let entry = Entry {
            id: 1,
            name: "open".into(),
            url: location,
            tab: "home".into(),
            params: Default::default(),
            fragment: String::new(),
        };
        assert_eq!(search_param(&entry, "u"), long);
    }
}
