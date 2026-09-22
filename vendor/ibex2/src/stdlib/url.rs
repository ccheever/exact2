//! `URL` / `URLSearchParams` — pure, ungated (LLP 0059.000 §3.4).
//!
//! Full WHATWG parsing including IDNA and percent-encoding, delegated to the
//! `url` crate rather than hand-written, because §3.4 is explicit that a
//! partial URL parser is a security bug and not a missing feature.

use url::Url;

use crate::boundary::HostError;

/// The parsed components a `URL` object exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    pub href: String,
    pub origin: String,
    pub protocol: String,
    pub username: String,
    pub password: String,
    pub host: String,
    pub hostname: String,
    pub port: String,
    pub pathname: String,
    pub search: String,
    pub hash: String,
}

impl ParsedUrl {
    /// The components as one string, one per line, in the order `url.js`
    /// destructures them. No component can contain a newline: the parser
    /// strips ASCII tab and newline from its input and the serializer
    /// percent-encodes everything else, so a line is a safe frame and JSON
    /// would be a parser's problem on the startup path.
    pub fn joined(&self) -> String {
        [
            &self.href,
            &self.origin,
            &self.protocol,
            &self.username,
            &self.password,
            &self.host,
            &self.hostname,
            &self.port,
            &self.pathname,
            &self.search,
            &self.hash,
        ]
        .map(String::as_str)
        .join("\n")
    }
}

/// Parse, optionally against a base — `new URL(input)` and `new URL(input, base)`.
pub fn parse(input: &str, base: Option<&str>) -> Result<ParsedUrl, HostError> {
    let parsed = match base {
        Some(base) => {
            let base = Url::parse(base)
                .map_err(|e| HostError::Failed(format!("TypeError: invalid base URL: {e}")))?;
            base.join(input)
        }
        None => Url::parse(input),
    }
    .map_err(|e| HostError::Failed(format!("TypeError: invalid URL: {e}")))?;

    Ok(components(&parsed))
}

/// A WHATWG setter — `url.<field> = value` — returning the URL as it is after.
///
/// The setters' semantics are the `url` crate's, which are the spec's: a
/// scheme change between special and non-special is refused, a host cannot be
/// set on a cannot-be-a-base URL, and every refusal leaves the URL unchanged —
/// the spec's setters fail silently, and only `href` throws. `search` and
/// `hash` drop one leading sigil, `protocol` one trailing colon, and `port`
/// takes its leading digits, as the spec's state overrides do.
pub fn set(href: &str, field: &str, value: &str) -> Result<ParsedUrl, HostError> {
    let mut url =
        Url::parse(href).map_err(|e| HostError::Failed(format!("TypeError: invalid URL: {e}")))?;
    match field {
        "href" => {
            url = Url::parse(value)
                .map_err(|e| HostError::Failed(format!("TypeError: invalid URL: {e}")))?;
        }
        "protocol" => {
            // Scheme start state with a state override: everything before the
            // first colon is the scheme, and the rest is ignored — so
            // `http:garbage` sets `http:`.
            let _ = url.set_scheme(value.split(':').next().unwrap_or(""));
        }
        "username" => {
            let _ = url.set_username(value);
        }
        "password" => {
            let _ = url.set_password(if value.is_empty() { None } else { Some(value) });
        }
        "host" => {
            // Host state, then port state, each with a state override: the
            // host runs to the first colon outside brackets; the port takes
            // its leading digits and stops — `x:80abc` is host `x`, port 80 —
            // and an empty port buffer leaves the port as it was, so `x:`
            // changes the host and nothing else.
            let after_bracket = if value.starts_with('[') {
                value.find(']').map(|at| at + 1).unwrap_or(value.len())
            } else {
                0
            };
            match value[after_bracket..]
                .find(':')
                .map(|at| at + after_bracket)
            {
                Some(at) => {
                    if url.set_host(Some(&value[..at])).is_ok() {
                        set_port_prefix(&mut url, &value[at + 1..]);
                    }
                }
                None => {
                    let _ = url.set_host(Some(value));
                }
            }
        }
        "hostname" => {
            let _ = url.set_host(Some(value));
        }
        "port" => set_port_digits(&mut url, value),
        "pathname" => {
            if !url.cannot_be_a_base() {
                url.set_path(value);
            }
        }
        "search" => {
            let query = value.strip_prefix('?').unwrap_or(value);
            url.set_query(if query.is_empty() { None } else { Some(query) });
        }
        "hash" => {
            let fragment = value.strip_prefix('#').unwrap_or(value);
            url.set_fragment(if fragment.is_empty() {
                None
            } else {
                Some(fragment)
            });
        }
        other => {
            return Err(HostError::InvalidArgument(format!(
                "not a URL field: {other}"
            )))
        }
    }
    Ok(components(&url))
}

/// The port setter: empty clears; otherwise the leading digits, if they make
/// a port; anything else is a silent no-op.
fn set_port_digits(url: &mut Url, value: &str) {
    if value.is_empty() {
        let _ = url.set_port(None);
        return;
    }
    set_port_prefix(url, value);
}

/// The port state under the host setter: leading digits set the port; no
/// digits at all leaves it alone, which is what an empty buffer means there.
fn set_port_prefix(url: &mut Url, value: &str) {
    let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
    if let Ok(port) = digits.parse::<u16>() {
        let _ = url.set_port(Some(port));
    }
}

fn components(parsed: &Url) -> ParsedUrl {
    // The web's accessors are not the crate's: `protocol` keeps its colon,
    // `search` and `hash` keep their leading sigil and are empty (not "?"/"#")
    // when absent, and `port` is the empty string when the port is the scheme's
    // default. Normalizing here is what "Rust owns semantics" means for URL.
    let host = match (parsed.host_str(), parsed.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        (None, _) => String::new(),
    };
    ParsedUrl {
        href: parsed.as_str().to_string(),
        origin: parsed.origin().ascii_serialization(),
        protocol: format!("{}:", parsed.scheme()),
        username: parsed.username().to_string(),
        password: parsed.password().unwrap_or("").to_string(),
        host,
        hostname: parsed.host_str().unwrap_or("").to_string(),
        port: parsed.port().map(|p| p.to_string()).unwrap_or_default(),
        pathname: parsed.path().to_string(),
        search: match parsed.query() {
            Some("") | None => String::new(),
            Some(query) => format!("?{query}"),
        },
        hash: match parsed.fragment() {
            Some("") | None => String::new(),
            Some(fragment) => format!("#{fragment}"),
        },
    }
}

/// `URLSearchParams`, ordered and allowing duplicate names as the spec requires.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchParams {
    pairs: Vec<(String, String)>,
}

impl SearchParams {
    /// Parse from a query string, with or without a leading `?`.
    pub fn parse(input: &str) -> Self {
        let trimmed = input.strip_prefix('?').unwrap_or(input);
        let pairs = form_urlencoded::parse(trimmed.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        Self { pairs }
    }

    pub fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        Self { pairs }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn get_all(&self, name: &str) -> Vec<&str> {
        self.pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.pairs.iter().any(|(k, _)| k == name)
    }

    pub fn append(&mut self, name: &str, value: &str) {
        self.pairs.push((name.to_string(), value.to_string()));
    }

    /// `set`: replace the first occurrence and drop the rest, keeping position.
    /// Appends when absent.
    pub fn set(&mut self, name: &str, value: &str) {
        let mut seen = false;
        self.pairs.retain(|(k, _)| {
            if k != name {
                return true;
            }
            if seen {
                return false;
            }
            seen = true;
            true
        });
        match self.pairs.iter_mut().find(|(k, _)| k == name) {
            Some(pair) => pair.1 = value.to_string(),
            None => self.append(name, value),
        }
    }

    pub fn delete(&mut self, name: &str) {
        self.pairs.retain(|(k, _)| k != name);
    }

    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn to_query_string(&self) -> String {
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(self.pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .finish()
    }

    /// `has(name, value)` — the two-argument form.
    pub fn has_pair(&self, name: &str, value: &str) -> bool {
        self.pairs.iter().any(|(k, v)| k == name && v == value)
    }

    /// `delete(name, value)` — only the pairs with that exact value.
    pub fn delete_pair(&mut self, name: &str, value: &str) {
        self.pairs.retain(|(k, v)| !(k == name && v == value));
    }

    /// `sort()`: stable, by name, comparing UTF-16 code units as the spec says
    /// — which is what a JavaScript string comparison would do, done here so
    /// there is one answer.
    pub fn sort(&mut self) {
        self.pairs
            .sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    }

    /// The pairs as a JSON array of `[name, value]` arrays, for one crossing
    /// that hands JavaScript its iteration order.
    pub fn entries_json(&self) -> String {
        let items: Vec<String> = self
            .pairs
            .iter()
            .map(|(k, v)| format!("[{},{}]", json_string(k), json_string(v)))
            .collect();
        format!("[{}]", items.join(","))
    }

    /// `getAll(name)` as a JSON array of strings.
    pub fn get_all_json(&self, name: &str) -> String {
        let items: Vec<String> = self.get_all(name).into_iter().map(json_string).collect();
        format!("[{}]", items.join(","))
    }
}

/// A JSON string literal. The escapes JSON requires and nothing more; this is
/// the one direction the boundary encodes, and the engine's own `JSON.parse`
/// decodes it.
pub(crate) fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_url_into_web_shaped_components() {
        let u = parse("https://user:pw@example.com:8443/a/b?x=1&y=2#frag", None).unwrap();
        assert_eq!(u.protocol, "https:");
        assert_eq!(u.username, "user");
        assert_eq!(u.password, "pw");
        assert_eq!(u.hostname, "example.com");
        assert_eq!(u.port, "8443");
        assert_eq!(u.host, "example.com:8443");
        assert_eq!(u.pathname, "/a/b");
        assert_eq!(u.search, "?x=1&y=2");
        assert_eq!(u.hash, "#frag");
        assert_eq!(u.origin, "https://example.com:8443");
    }

    #[test]
    fn a_default_port_serializes_as_an_empty_port() {
        let u = parse("https://example.com/", None).unwrap();
        assert_eq!(u.port, "", "the scheme's default port is not exposed");
        assert_eq!(u.host, "example.com");
        assert_eq!(u.origin, "https://example.com");
    }

    #[test]
    fn absent_query_and_fragment_are_empty_not_bare_sigils() {
        let u = parse("https://example.com/path", None).unwrap();
        assert_eq!(u.search, "");
        assert_eq!(u.hash, "");
    }

    #[test]
    fn resolves_against_a_base() {
        let u = parse("../c", Some("https://example.com/a/b/")).unwrap();
        assert_eq!(u.href, "https://example.com/a/c");
    }

    #[test]
    fn rejects_a_relative_url_with_no_base() {
        assert!(parse("/just/a/path", None).is_err());
        assert!(parse("not a url", None).is_err());
    }

    /// The reason §3.4 insists on a real parser: IDNA and percent-encoding are
    /// where a hand-rolled one becomes a security bug.
    #[test]
    fn applies_idna_and_percent_encoding() {
        let u = parse("https://例え.テスト/ä", None).unwrap();
        assert_eq!(u.hostname, "xn--r8jz45g.xn--zckzah");
        assert_eq!(u.pathname, "/%C3%A4");
    }

    #[test]
    fn search_params_keep_order_and_duplicates() {
        let params = SearchParams::parse("?a=1&b=2&a=3");
        assert_eq!(params.get("a"), Some("1"));
        assert_eq!(params.get_all("a"), vec!["1", "3"]);
        assert_eq!(params.len(), 3);
        assert!(params.has("b"));
        assert!(!params.has("c"));
    }

    #[test]
    fn set_replaces_in_place_and_drops_later_duplicates() {
        let mut params = SearchParams::parse("a=1&b=2&a=3");
        params.set("a", "9");
        assert_eq!(params.get_all("a"), vec!["9"]);
        // Position is preserved: 'a' stays first, ahead of 'b'.
        assert_eq!(params.to_query_string(), "a=9&b=2");
    }

    #[test]
    fn set_appends_when_the_name_is_absent() {
        let mut params = SearchParams::parse("a=1");
        params.set("z", "26");
        assert_eq!(params.to_query_string(), "a=1&z=26");
    }

    #[test]
    fn delete_removes_every_occurrence() {
        let mut params = SearchParams::parse("a=1&b=2&a=3");
        params.delete("a");
        assert_eq!(params.to_query_string(), "b=2");
    }

    #[test]
    fn serialization_escapes_and_round_trips() {
        let mut params = SearchParams::default();
        params.append("q", "a b&c=d");
        let encoded = params.to_query_string();
        assert_eq!(encoded, "q=a+b%26c%3Dd");
        assert_eq!(SearchParams::parse(&encoded).get("q"), Some("a b&c=d"));
    }

    #[test]
    fn setters_follow_the_spec_and_fail_silently_except_href() {
        let u = set("https://h/p?a=1#x", "search", "?b=2").unwrap();
        assert_eq!(u.href, "https://h/p?b=2#x");
        let u = set(&u.href, "hash", "").unwrap();
        assert_eq!(u.href, "https://h/p?b=2");
        let u = set(&u.href, "pathname", "q r").unwrap();
        assert_eq!(u.pathname, "/q%20r");
        let u = set(&u.href, "port", "8080x").unwrap();
        assert_eq!(u.port, "8080", "leading digits");
        let u = set(&u.href, "port", "abc").unwrap();
        assert_eq!(u.port, "8080", "not a port: unchanged");
        let u = set(&u.href, "host", "h2:99").unwrap();
        assert_eq!((u.hostname.as_str(), u.port.as_str()), ("h2", "99"));
        let u = set(&u.href, "host", "h3:80abc").unwrap();
        assert_eq!(
            (u.hostname.as_str(), u.port.as_str()),
            ("h3", "80"),
            "leading digits"
        );
        let u = set(&u.href, "host", "h4:").unwrap();
        assert_eq!(
            (u.hostname.as_str(), u.port.as_str()),
            ("h4", "80"),
            "empty port buffer: unchanged"
        );
        let u = set("https://h/", "host", "[::1]:8080").unwrap();
        assert_eq!((u.hostname.as_str(), u.port.as_str()), ("[::1]", "8080"));
        let u = set("https://h/", "protocol", "http:garbage").unwrap();
        assert_eq!(
            u.protocol, "http:",
            "everything after the first colon is ignored"
        );
        let u = set(&u.href, "protocol", "http:").unwrap();
        assert_eq!(u.protocol, "http:");
        let u = set(&u.href, "protocol", "mailto").unwrap();
        assert_eq!(u.protocol, "http:", "special to non-special is refused");
        assert!(
            set(&u.href, "href", "nope").is_err(),
            "href is the one that throws"
        );
        let u = set("mailto:a@b", "pathname", "x").unwrap();
        assert_eq!(u.pathname, "a@b", "cannot-be-a-base: no-op");
    }

    #[test]
    fn search_params_sort_delete_pair_and_json() {
        let mut p = SearchParams::parse("?b=2&a=1&a=3&\"q\"=x\ty");
        p.sort();
        assert_eq!(p.to_query_string(), "%22q%22=x%09y&a=1&a=3&b=2");
        assert!(p.has_pair("a", "3") && !p.has_pair("a", "2"));
        p.delete_pair("a", "1");
        assert_eq!(p.get_all_json("a"), r#"["3"]"#);
        assert_eq!(
            p.entries_json(),
            r#"[["\"q\"","x\ty"],["a","3"],["b","2"]]"#
        );
    }
}
