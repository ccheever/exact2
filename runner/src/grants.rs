//! Whole-set grant admission, sharing ibex2's pure grammar.
//! @ref LLP 1016 D6 / LLP 1018 D3. Presenter and device grants remain
//! with their owning executors, exactly as `io_grants` treats them.
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
}

/// Parse `spec` whole: its grants, or every line that is not one, each as
/// `line N: why` (ibex2's wording where the rule is ibex2's).
pub fn parse(spec: &str) -> Result<Grants<'_>, Vec<String>> {
    let (mut lines, mut errors) = (Vec::new(), Vec::new());
    for (index, line) in spec.lines().map(str::trim).enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if own(line) {
            lines.push(line);
        } else {
            match exact_grants::GrantSet::parse(line) {
                Ok(_) => lines.push(line),
                Err(error) => errors.push(format!(
                    "line {}: {}",
                    index + 1,
                    error.strip_prefix("line 1: ").unwrap_or(&error)
                )),
            }
        }
    }
    if errors.is_empty() {
        Ok(Grants { lines })
    } else {
        Err(errors)
    }
}

/// What a reader says of a set that did not parse, as the native executor
/// says it: the set grants nothing, and every refusal carries this.
pub fn refusal(errors: &[String]) -> String {
    format!("the app's grants did not parse: {}", errors.join("; "))
}

/// Whether a trimmed line is one of exact2's own grants: enforced by the
/// presenter (`surface.*`), by the OS and the capability that asks
/// (`device.*`, LLP 1069.008 D3) and by the host's auth arm (`auth.*`, LLP
/// 1069.006), never by the I/O executor.
pub(crate) fn own(line: &str) -> bool {
    line.starts_with("surface.read ")
        || line.starts_with("surface.write ")
        || crate::device::is_device_line(line)
        || line.starts_with("auth.")
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
        Grant::Fetch(origin) | Grant::FetchSubdomains(origin) | Grant::WebSocket(origin) => {
            let kind = match grant {
                Grant::Fetch(_) => "fetch",
                Grant::FetchSubdomains(_) => "fetch-subdomains",
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
    fn normalized_output_keeps_exact_lines_comments_and_bad_parent_sources() {
        let json = normalized_json(
            "# app\nauth.session https://login.test\nnet.fetch https://API.example\nsecret.keep camelCase",
        );
        assert!(json.contains("# app"), "{json}");
        assert!(json.contains("auth.session https://login.test"), "{json}");
        assert!(
            json.contains("[\"fetch\",\"https\",\"api.example\",443]"),
            "{json}"
        );
        assert!(json.contains("line 4: `camelCase`"), "{json}");
        assert!(json.contains("\"seal\":\""), "{json}");
    }

    #[test]
    fn normalized_output_uses_javascript_json_control_escapes_before_sealing() {
        let json = normalized_json("net.fetch\u{c}https://api.example\n# back\u{8}space");
        assert!(json.contains("net.fetch\\fhttps://api.example"), "{json}");
        assert!(json.contains("# back\\bspace"), "{json}");
    }
}
