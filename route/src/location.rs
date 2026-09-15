//! The browser's path and query serializer, without a host parser.
//!
//! @ref LLP 1038 §3 — Location and the searchParam read; D1, D3, D8.
//!
//! Locations are absolute paths appended to `https://x`, never resolved as
//! network-path references. Backslashes are path separators and the query
//! uses the special-query set (it includes `'`). Existing percent escapes
//! keep their spelling; only parameter formatting uses encodeURIComponent.

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

/// Derive a location from a host's incoming URL. For non-HTTP schemes the
/// authority is the first path segment, so `interview://post/42` is `/post/42`.
/// A scheme without `//` uses everything after `:` as that path (e.g. mailto).
pub fn location_of(href: &str) -> String {
    let input = clean(href);
    match scheme(&input) {
        Some((name, after))
            if !name.eq_ignore_ascii_case("http") && !name.eq_ignore_ascii_case("https") =>
        {
            path_query(&format!("/{}", after.strip_prefix("//").unwrap_or(after)))
        }
        Some((_, after)) => path_query(without_authority(after.trim_start_matches(['/', '\\']))),
        None => canonical(&input),
    }
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
    let mut out = format!("/{}", segments.join("/"));
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

fn encode(input: &str, escape: impl Fn(u8) -> bool) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        if escape(b) {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 15) as usize] as char);
        } else {
            out.push(b as char);
        }
    }
    out
}

pub(crate) fn decode(input: &str, plus: bool) -> String {
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
    String::from_utf8_lossy(&out).into_owned()
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
