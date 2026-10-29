//! Whole-set grant admission, sharing ibex2's pure grammar.
//! @ref LLP 1016 D6 / LLP 1018 D3. Presenter and device grants remain
//! with their owning executors, exactly as `io_grants` treats them.
pub use exact_grants::{is_service_type, DEVELOPMENT, LOCAL_NETWORK};

/// A grant spec that parsed: its lines, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grants<'a> {
    lines: Vec<&'a str>,
}

impl<'a> Grants<'a> {
    /// Every grant line, trimmed, in declaration order.
    pub fn lines(&self) -> &[&'a str] {
        &self.lines
    }

    /// The names `secret.keep` grants, in declaration order: the store's
    /// load list (LLP 1018 D3).
    pub fn secrets(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.lines.iter().filter_map(|line| {
            let mut words = line.split_whitespace();
            (words.next()? == "secret.keep")
                .then(|| words.next())
                .flatten()
        })
    }

    /// The `secret.keep` names that carry a presence policy (LLP 1133.010
    /// D4), in declaration order.
    pub fn presence(&self) -> impl Iterator<Item = (&'a str, exact_grants::Presence)> + '_ {
        self.lines
            .iter()
            .filter_map(|line| exact_grants::line_presence(line))
    }
}

/// How the runner reads an I/O grant line (`net.fetch …`, `secret.keep …`),
/// when an archive links I/O (LLP 1047.001): its own lines (devices, auth)
/// it reads itself. An archive without I/O names no grant grammar, and the
/// URL parsing an origin needs with it.
pub type GrantsLink = Option<fn(&str) -> Result<(), String>>;

/// Read one I/O grant line: [`GrantsLink`]'s linked value.
pub fn validate(line: &str) -> Result<(), String> {
    exact_grants::GrantSet::parse(line).map(|_| ())
}

/// Parse `spec` whole: its grants, or every line that is not one, each as
/// `line N: why` (ibex2's wording where the rule is ibex2's).
pub fn parse(spec: &str) -> Result<Grants<'_>, Vec<String>> {
    parse_with(spec, Some(validate))
}

/// [`parse`] with the I/O grammar `link` names; without one, an I/O line is
/// refused by name.
pub fn parse_with(spec: &str, link: GrantsLink) -> Result<Grants<'_>, Vec<String>> {
    let (mut lines, mut errors) = (Vec::new(), Vec::new());
    for (index, line) in spec.lines().map(str::trim).enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(why) = own(line).then(|| listen_refusal(line)).flatten() {
            errors.push(format!("line {}: {why}", index + 1));
        } else if exact_grants::process::is_process_line(line) {
            match exact_grants::process::validate(line) {
                Ok(()) => lines.push(line),
                Err(error) => errors.push(format!("line {}: {error}", index + 1)),
            }
        } else if own(line) {
            lines.push(line);
        } else {
            let read = match link {
                Some(validate) => validate(line),
                None => Err(format!("`{line}` is I/O, which this archive does not link")),
            };
            match read {
                Ok(()) => lines.push(line),
                Err(error) => errors.push(format!(
                    "line {}: {}",
                    index + 1,
                    error.strip_prefix("line 1: ").unwrap_or(&error)
                )),
            }
        }
    }
    if let Some(twice) = exact_grants::process::named_twice(spec) {
        errors.push(twice);
    }
    if errors.is_empty() {
        Ok(Grants { lines })
    } else {
        Err(errors)
    }
}

/// The line that admits a request to `url`, of the app's grants `spec`
/// narrowed by a source's `scope` (which may only select lines the app
/// has): the first `net.websocket` line for a `ws:` or `wss:` URL, as every
/// executor opens one, else the first `net.fetch` line. `state.streams`
/// names it (LLP 1016.000 D7).
pub fn admitting<'a>(spec: &'a str, scope: Option<&'a str>, url: &str) -> Option<&'a str> {
    use exact_grants::{GrantSet, Operation};
    // A listener's line, by its path as written (LLP 1133.005 D11).
    if let Ok(path) = crate::listen::unix_path(url) {
        return crate::listen::admitting(spec, scope, path, str::to_string);
    }
    let origin = exact_grants::origin_of(url).ok()?;
    let operation = if matches!(origin.scheme.as_str(), "ws" | "wss") {
        Operation::WebSocket { origin }
    } else {
        Operation::Fetch { origin }
    };
    let lines = |text: &'a str| text.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut candidates = lines(scope.unwrap_or(spec));
    if scope.is_some_and(|scope| lines(scope).any(|l| !lines(spec).any(|a| a == l))) {
        return None;
    }
    candidates.find(|line| {
        matches!(
            line.split_whitespace().next(),
            Some("net.fetch" | "net.websocket")
        ) && GrantSet::parse(line).is_ok_and(|set| set.permits(&operation))
    })
}

/// What a reader says of a set that did not parse, as the native executor
/// says it: the set grants nothing, and every refusal carries this.
pub fn refusal(errors: &[String]) -> String {
    format!("the app's grants did not parse: {}", errors.join("; "))
}

/// Whether a trimmed line is one of exact2's own grants: enforced by the
/// presenter (`surface.*`), by the OS and the capability that asks
/// (`device.*`, LLP 1069.008 D3), by the OS's hardened runtime (`runtime.*`,
/// D9), by the host's auth arm (`auth.*`, LLP 1069.006), by the host's
/// kept documents (`doc.keep`, LLP 1069.010.000 D7), by the module
/// admission of `@exact/browser-automation`'s names (`browser.automation`,
/// LLP 1020.002 D9), of `@exact/local-network`'s service types
/// (`net.dns-sd <type>`, LLP 1133.036 D7) and of `@exact/payments`'s
/// (`payment.*`, LLP 1133.034 D13), never by the I/O executor's grammar;
/// and the desktop-process lines (`process.spawn`, `env.read login:`,
/// `secret.hold`, LLP 1133.007 §6), which the runner and the data executor
/// read with `exact_grants::process`, and `net.listen`, which the listener
/// checks (LLP 1133.005 D11).
pub(crate) fn own(line: &str) -> bool {
    line == "doc.keep"
        || exact_grants::process::is_process_line(line)
        || line.starts_with("net.dns-sd ")
        || line.split_whitespace().next() == Some("net.listen")
        || line == "browser.automation"
        || line == "payment.google-pay"
        || line.starts_with("payment.apple-pay ")
        || line.starts_with("surface.read ")
        || line.starts_with("surface.write ")
        || crate::device::is_device_line(line)
        || crate::hardened::is_runtime_line(line)
        || line.starts_with("auth.")
}

/// Why an own line naming `net.listen` is not one, if it is not.
fn listen_refusal(line: &str) -> Option<String> {
    (line.split_whitespace().next() == Some("net.listen"))
        .then(|| crate::listen::grant_refusal(line))
        .flatten()
}

/// One declaration in the browser-facing form. `source` retains every
/// nonblank line because native child scopes compare exact trimmed lines
/// before parsing their I/O portion. Exact-owned lines and comments have no
/// I/O grant. An error omits its line prefix so a child reports its own line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedLine {
    number: usize,
    source: String,
    grant: Option<exact_grants::Grant>,
    error: Option<String>,
}

fn normalized(spec: &str) -> (Vec<NormalizedLine>, Option<String>) {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (index, raw) in spec.lines().enumerate() {
        let source = raw.trim();
        if source.is_empty() {
            continue;
        }
        let number = index + 1;
        if source.starts_with('#') || own(source) {
            lines.push(NormalizedLine {
                number,
                source: source.into(),
                grant: None,
                error: None,
            });
            continue;
        }
        match exact_grants::GrantSet::parse(source) {
            Ok(set) => lines.push(NormalizedLine {
                number,
                source: source.into(),
                grant: set.iter().next().cloned(),
                error: None,
            }),
            Err(error) => {
                let reason = error.strip_prefix("line 1: ").unwrap_or(&error).to_string();
                errors.push(format!("line {number}: {reason}"));
                lines.push(NormalizedLine {
                    number,
                    source: source.into(),
                    grant: None,
                    error: Some(reason),
                });
            }
        }
    }
    let error = (!errors.is_empty()).then(|| refusal(&errors));
    (lines, error)
}

fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn grant_json(grant: &exact_grants::Grant, out: &mut String) {
    use exact_grants::Grant;
    match grant {
        Grant::Fetch(origin)
        | Grant::FetchSubdomains(origin)
        | Grant::WebSocket(origin)
        | Grant::WebSocketSubdomains(origin) => {
            let kind = match grant {
                Grant::Fetch(_) => "fetch",
                Grant::FetchSubdomains(_) => "fetch-subdomains",
                Grant::WebSocketSubdomains(_) => "websocket-subdomains",
                _ => "websocket",
            };
            out.push('[');
            quote(kind, out);
            out.push(',');
            quote(&origin.scheme, out);
            out.push(',');
            quote(&origin.host, out);
            out.push(',');
            out.push_str(&origin.port.to_string());
            out.push(']');
        }
        Grant::FetchLocalNetwork { loopback } | Grant::WebSocketLocalNetwork { loopback } => {
            out.push('[');
            let kind = match grant {
                Grant::FetchLocalNetwork { .. } => "fetch-local-network",
                _ => "websocket-local-network",
            };
            quote(kind, out);
            if *loopback {
                out.push(',');
                quote(exact_grants::DEVELOPMENT, out);
            }
            out.push(']');
        }
        Grant::FetchScheme(scheme) | Grant::WebSocketScheme(scheme) => {
            let kind = match grant {
                Grant::FetchScheme(_) => "fetch-scheme",
                _ => "websocket-scheme",
            };
            out.push('[');
            quote(kind, out);
            out.push(',');
            quote(scheme, out);
            out.push(']');
        }
        Grant::FsRead(prefix) | Grant::FsWrite(prefix) | Grant::SqliteOpen(prefix) => {
            let kind = match grant {
                Grant::FsRead(_) => "fs-read",
                Grant::FsWrite(_) => "fs-write",
                _ => "sqlite-open",
            };
            out.push('[');
            quote(kind, out);
            for component in prefix.components() {
                out.push(',');
                quote(component, out);
            }
            out.push(']');
        }
        Grant::EnvRead(name) | Grant::SecretKeep(name) | Grant::StorageKv(name) => {
            let kind = match grant {
                Grant::EnvRead(_) => "env-read",
                Grant::SecretKeep(_) => "secret-keep",
                _ => "storage-kv",
            };
            out.push('[');
            quote(kind, out);
            out.push(',');
            quote(name, out);
            out.push(']');
        }
    }
}

fn seal(text: &str) -> String {
    // An integrity marker, not a sandbox boundary: app code is the page. It
    // distinguishes Rust parser output from a merely plausible object.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// A sealed, typed grant set for browser matchers. This is the only producer
/// of runtime grant objects on either web target.
pub fn normalized_json(spec: &str) -> String {
    let (lines, error) = normalized(spec);
    let mut body = String::from("{\"version\":1,\"entries\":[");
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push('[');
        body.push_str(&line.number.to_string());
        body.push(',');
        quote(&line.source, &mut body);
        body.push(',');
        match &line.grant {
            Some(grant) => grant_json(grant, &mut body),
            None => body.push_str("null"),
        }
        body.push(',');
        match &line.error {
            Some(error) => quote(error, &mut body),
            None => body.push_str("null"),
        }
        body.push(']');
    }
    body.push_str("],\"error\":");
    match error {
        Some(error) => quote(&error, &mut body),
        None => body.push_str("null"),
    }
    body.push('}');
    let marker = seal(&body);
    body.pop();
    body.push_str(",\"seal\":");
    quote(&marker, &mut body);
    body.push('}');
    body
}

#[cfg(test)]
mod tests {
    use super::normalized_json;

    #[test]
    fn quoted_targets_keep_source_spelling_beside_decoded_namespaces() {
        let source = "fs.read \"C:\\\\With Space\\\\雪\"\nfs.write \"doc:/chosen/a b\"";
        let (lines, error) = super::normalized(source);
        assert!(error.is_none());
        assert_eq!(lines[0].source, source.lines().next().unwrap());
        let json = normalized_json(source);
        assert!(
            json.contains(r#"["fs-read","win:C","With Space","雪"]"#),
            "{json}"
        );
        assert!(
            json.contains(r#"["fs-write","doc:","chosen","a b"]"#),
            "{json}"
        );
        let (_, error) = super::normalized("fs.read \"C:/safe\" fs.write C:/\nfs.read app:/data");
        assert!(error.is_some(), "one malformed declaration refuses the set");
    }

    #[test]
    fn normalized_output_keeps_exact_lines_comments_and_bad_parent_sources() {
        let json = normalized_json(
            "# app\nauth.session https://login.test\nnet.fetch https://API.example\nsecret.keep camelCase\ndoc.keep",
        );
        assert!(json.contains(r#"[5,"doc.keep",null,null]"#), "{json}");
        assert!(super::parse("fs.read doc:/\ndoc.keep").is_ok());
        assert_eq!(crate::io_grants("fs.read doc:/\ndoc.keep"), "fs.read doc:/");
        assert!(json.contains("# app"), "{json}");
        assert!(json.contains("auth.session https://login.test"), "{json}");
        assert!(
            json.contains("[\"fetch\",\"https\",\"api.example\",443]"),
            "{json}"
        );
        assert!(json.contains("line 4: `camelCase`"), "{json}");
        assert!(json.contains("\"seal\":\""), "{json}");
    }

    /// The desktop-process lines (LLP 1133.007 §6) are the runner's own:
    /// read whole here, never handed to the I/O grammar, and a name is
    /// kept or held, not both.
    #[test]
    fn process_lines_are_own_lines_read_whole() {
        let spec = "process.spawn path:ssh\nenv.read login:PATH\nsecret.hold ssh.password\nfs.read home:/.ssh";
        assert_eq!(super::parse(spec).unwrap().lines().len(), 4);
        assert_eq!(crate::io_grants(spec), "fs.read home:/.ssh");
        assert!(super::parse("process.spawn path:a/b").unwrap_err()[0].starts_with("line 1: "));
        let twice = super::parse("secret.keep ssh.password\nsecret.hold ssh.password").unwrap_err();
        assert!(twice[0].starts_with("secret-name-twice"), "{twice:?}");
        assert!(super::parse("fs.read home:/").unwrap_err()[0].contains("home-grant-whole"));
    }

    /// `browser.automation` is the runner's own line and names no target
    /// (LLP 1020.002 D9): with a word after it, it is no such line, and the
    /// I/O grammar refuses it.
    #[test]
    fn browser_automation_is_an_own_line_with_no_target() {
        let spec = "browser.automation\nnet.fetch https://a.test";
        assert!(super::parse(spec).is_ok());
        assert_eq!(crate::io_grants(spec), "net.fetch https://a.test");
        assert!(super::parse("browser.automation tab-1").is_err());
        assert!(super::parse("browser.automations").is_err());
    }

    #[test]
    fn a_scheme_line_normalizes_to_its_family_and_scheme_and_names_what_it_admits() {
        let spec = "net.fetch HTTPS:\nnet.websocket wss://feed.example\nnet.websocket ws:";
        let json = normalized_json(spec);
        assert!(json.contains(r#"["fetch-scheme","https"]"#), "{json}");
        assert!(json.contains(r#"["websocket-scheme","ws"]"#), "{json}");
        let admitting = |url| super::admitting(spec, None, url);
        assert_eq!(
            admitting("https://10.0.0.1:8443/x"),
            Some("net.fetch HTTPS:")
        );
        assert_eq!(
            admitting("ws://127.0.0.1:3773/ws"),
            Some("net.websocket ws:")
        );
        assert_eq!(
            admitting("wss://feed.example/s"),
            Some("net.websocket wss://feed.example")
        );
        assert_eq!(admitting("wss://other.example/s"), None);
        assert_eq!(admitting("http://127.0.0.1/"), None);
        // A source's scope narrows the lines; one beyond the app's admits nothing.
        assert_eq!(
            super::admitting(spec, Some("net.fetch HTTPS:"), "ws://127.0.0.1/"),
            None
        );
        assert_eq!(
            super::admitting(spec, Some("net.websocket ws:"), "ws://127.0.0.1/"),
            Some("net.websocket ws:")
        );
        assert_eq!(
            super::admitting(spec, Some("net.websocket ws://a"), "ws://a/"),
            None
        );
    }

    #[test]
    fn websocket_subdomains_normalize_and_name_the_admitting_line() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../host/web/tests/fixtures/grants.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let spec = case["spec"].as_str().unwrap();
            if !spec.starts_with("net.websocket ") || !spec.contains("://*.") {
                continue;
            }
            let json: serde_json::Value = serde_json::from_str(&normalized_json(spec)).unwrap();
            let valid = case["ok"].as_bool().unwrap();
            assert_eq!(json["error"].is_null(), valid, "{spec}");
            if valid {
                assert_eq!(json["entries"][0][2][0], "websocket-subdomains", "{spec}");
            }
            for probe in case["websocket"].as_array().into_iter().flatten() {
                let url = probe[0].as_str().unwrap();
                let expected = probe[1].as_bool().unwrap().then_some(spec);
                assert_eq!(super::admitting(spec, None, url), expected, "{spec}: {url}");
            }
        }
    }

    #[test]
    fn normalized_output_uses_javascript_json_control_escapes_before_sealing() {
        let json = normalized_json(
            "net.fetch\u{c}https://api.example\n# back\u{8}space\n# line\u{2028}separator\n# paragraph\u{2029}separator",
        );
        assert!(json.contains("net.fetch\\fhttps://api.example"), "{json}");
        assert!(json.contains("# back\\bspace"), "{json}");
        assert!(json.contains("# line\\u2028separator"), "{json}");
        assert!(json.contains("# paragraph\\u2029separator"), "{json}");
    }
}
