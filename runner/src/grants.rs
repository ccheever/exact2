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
