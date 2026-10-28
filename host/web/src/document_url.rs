//! URL admission shared by the projected document and its page head.

/// Whether `href` navigates somewhere the page allows: `navigableURL`'s one
/// scheme allowlist (http, https, mailto, tel), read the way the URL parser
/// reads a scheme — surrounding C0 controls and spaces stripped, tabs and
/// newlines removed anywhere, the scheme case-folded. A URL with no scheme is
/// relative to the page's https base; an http(s) authority whose host does
/// not parse is refused, as the parser's failure is (IDNA aside).
pub fn navigable(href: &str) -> bool {
    let url = parsed(href);
    if let Some(at) = scheme_end(&url) {
        let (scheme, rest) = (&url[..at], &url[at + 1..]);
        let is = |name: &str| scheme.eq_ignore_ascii_case(name);
        return if is("mailto") || is("tel") {
            true
        } else if is("https") && !rest.starts_with("//") {
            // The base's own scheme reads the rest as relative
            // unless it starts `//`; another special scheme's
            // authority follows whatever slashes there are.
            true
        } else {
            (is("http") || is("https")) && authority_parses(rest)
        };
    }
    // Relative: a leading pair of slashes (either way) starts an authority.
    let mut slashes = url.chars().take_while(|c| matches!(c, '/' | '\\'));
    if slashes.next().is_some() && slashes.next().is_some() {
        return authority_parses(&url);
    }
    true
}

/// Whether an iframe in `sandbox` (its attribute, `None` when absent) shows
/// `src`: a [`navigable`] URL, or a `data:` document in a sandbox without
/// `allow-same-origin`, whose opaque origin cannot reach the page
/// (`navigation.js` `frameURL`, LLP 1020 §10). The browser splits the
/// sandbox on ASCII whitespace and reads its tokens case-insensitively.
pub fn frame_navigable(src: &str, sandbox: Option<&str>) -> bool {
    let opaque = sandbox.is_some_and(|s| {
        !s.split_ascii_whitespace()
            .any(|t| t.eq_ignore_ascii_case("allow-same-origin"))
    });
    let url = parsed(src);
    navigable(src)
        || opaque && scheme_end(&url).is_some_and(|at| url[..at].eq_ignore_ascii_case("data"))
}

/// Whether an iframe's `src` is resolved by the page's glue before the
/// browser sees it (`data-app-src`): the app's own file, or a relative page
/// whose query has a value beginning `app:` (raw or percent-escaped), which
/// a bundled page is given as an object URL (LLP 1020.001 D7;
/// `picker-glue.js` decides which pages are given files). The same test as
/// rt.js `frame` and glue.js `appFrameSource`.
pub fn frame_app_source(src: &str) -> bool {
    if src.starts_with("app:/") {
        return true;
    }
    let url = parsed(src);
    if scheme_end(&url).is_some()
        || url.starts_with(['/', '\\']) && url[1..].starts_with(['/', '\\'])
    {
        return false;
    }
    let before_fragment = url.split('#').next().unwrap_or_default();
    let Some((_, query)) = before_fragment.split_once('?') else {
        return false;
    };
    query.split('&').any(|pair| {
        pair.split_once('=').is_some_and(|(_, value)| {
            let value = value.to_ascii_lowercase();
            value.starts_with("app:") || value.starts_with("app%3a")
        })
    })
}

/// `href` as the URL parser reads it: surrounding C0 controls and spaces
/// stripped, tabs and newlines removed anywhere.
fn parsed(href: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = href.trim_matches(|c: char| c <= ' ');
    if trimmed.contains(['\t', '\n', '\r']) {
        trimmed
            .chars()
            .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
            .collect::<String>()
            .into()
    } else {
        trimmed.into()
    }
}

/// Where a scheme's `:` is: a letter, then letters, digits, `+`, `-` or
/// `.`, then `:`. `None` when the URL is relative.
fn scheme_end(url: &str) -> Option<usize> {
    for (at, b) in url.bytes().enumerate() {
        match b {
            b':' if at > 0 => return Some(at),
            b if b.is_ascii_alphabetic() => {}
            b if at > 0 && (b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')) => {}
            _ => break,
        }
    }
    None
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
