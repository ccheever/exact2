//! Driver faults (LLP 1103): a fetch whose URL starts with an armed prefix
//! fails as a refused connection does, without going out. The table is the
//! session's: one per runner, shared by every source, placement and lane,
//! and consulted where the host would hand a plain request to its transport
//! (`Runner::fault_dispatch`), so each request is counted once. It is the
//! driver's alone: armed by `EXACT_AGENT_FAIL_FETCH` at launch under the
//! agent, or by the agent's `faults` operation.

use super::Runner;
use crate::{DataSource, Dispatch, FailureKind, Outcome, RequestOut, Work};
use std::sync::Mutex;

/// One armed prefix (D3): how many matching fetches it fails yet (`None`:
/// every one until `pass`), whether it still matches, and how many it has
/// failed since it was armed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Matches a URL that starts with it.
    pub prefix: String,
    /// The count given at arming, if any.
    pub times: Option<u32>,
    /// What is left of it.
    pub left: Option<u32>,
    /// Fetches it has failed since it was armed; `pass` keeps it.
    pub hits: u32,
    /// `pass` clears it.
    pub armed: bool,
}

impl Fault {
    fn live(&self) -> bool {
        self.armed && self.left != Some(0)
    }
}

/// The session's fault table.
#[derive(Debug, Default)]
pub struct Faults(Mutex<Vec<Fault>>);

impl Faults {
    /// The table a launch names: `EXACT_AGENT_FAIL_FETCH`, in
    /// [`Faults::parse`]'s lines, read only under the agent
    /// (`EXACT_AGENT=1`), as its other launch facts are.
    pub fn from_env() -> Faults {
        let agent = std::env::var("EXACT_AGENT").is_ok_and(|v| v == "1");
        let spec = std::env::var("EXACT_AGENT_FAIL_FETCH").ok();
        match spec.filter(|_| agent) {
            Some(spec) => Faults::parse(&spec).unwrap_or_default(),
            None => Faults::default(),
        }
    }

    /// A table from its launch lines, one prefix each: `<prefix>` fails
    /// every fetch, `<prefix>\t<times>` that many; a reload carries the rest
    /// as `<prefix>\t<times>\t<left>\t<hits>\t<armed>` (`-` for no count,
    /// `armed` 1 or 0), which [`Faults::spec`] writes.
    pub fn parse(spec: &str) -> Result<Faults, String> {
        let count = |field: Option<&str>| -> Result<Option<u32>, String> {
            match field.unwrap_or("-") {
                "-" => Ok(None),
                n => n
                    .parse::<u32>()
                    .map(Some)
                    .map_err(|_| format!("fail fetch: `{n}` is not a count")),
            }
        };
        let mut faults: Vec<Fault> = Vec::new();
        for line in spec.lines().filter(|l| !l.trim().is_empty()) {
            let mut fields = line.split('\t');
            let prefix = fields.next().unwrap_or_default();
            if prefix.is_empty() {
                return Err("fail fetch: each line names a non-empty prefix".into());
            }
            let times = count(fields.next())?;
            if times == Some(0) {
                return Err("fail fetch: `times` is a positive integer".into());
            }
            let left = match fields.next() {
                Some(field) => count(Some(field))?,
                None => times,
            };
            let hits = count(fields.next())?.unwrap_or(0);
            let armed = fields.next() != Some("0");
            faults.retain(|f| f.prefix != prefix);
            faults.push(Fault {
                prefix: prefix.to_string(),
                times,
                left,
                hits,
                armed,
            });
        }
        Ok(Faults(Mutex::new(faults)))
    }

    /// The table as launch lines, for a reload to carry as it is.
    pub fn spec(&self) -> String {
        let n = |c: Option<u32>| c.map_or("-".to_string(), |n| n.to_string());
        self.table()
            .iter()
            .map(|f| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    f.prefix,
                    n(f.times),
                    n(f.left),
                    f.hits,
                    u8::from(f.armed)
                )
            })
            .collect()
    }

    fn table(&self) -> std::sync::MutexGuard<'_, Vec<Fault>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Arms `prefix` for `times` fetches, or every one; arming it again
    /// replaces its entry.
    pub fn arm(&self, prefix: &str, times: Option<u32>) -> Result<(), String> {
        if prefix.is_empty() {
            return Err("fail fetch needs a URL prefix".into());
        }
        if times == Some(0) {
            return Err("fail fetch: `times` is a positive integer".into());
        }
        let mut table = self.table();
        table.retain(|f| f.prefix != prefix);
        table.push(Fault {
            prefix: prefix.to_string(),
            times,
            left: times,
            hits: 0,
            armed: true,
        });
        Ok(())
    }

    /// Stops `prefix` failing; its hits stay for the test's check. Whether
    /// it was in the table.
    pub fn pass(&self, prefix: &str) -> bool {
        let mut table = self.table();
        let entry = table.iter_mut().find(|f| f.prefix == prefix);
        entry.map(|f| f.armed = false).is_some()
    }

    /// Whether a fetch of `url` fails, counting it: the longest live prefix
    /// it starts with decides.
    pub fn take(&self, url: &str) -> bool {
        let mut table = self.table();
        let hit = table
            .iter_mut()
            .filter(|f| f.live() && url.starts_with(&f.prefix))
            .max_by_key(|f| f.prefix.len());
        match hit {
            Some(f) => {
                f.hits += 1;
                f.left = f.left.map(|n| n - 1);
                true
            }
            None => false,
        }
    }

    /// Whether the table holds nothing.
    pub fn is_empty(&self) -> bool {
        self.table().is_empty()
    }

    /// The table as `state.faults` shows it: `prefix`, `times`, `left`
    /// (`null` without a count), `hits` and `armed`.
    pub fn json(&self) -> String {
        let n = |c: Option<u32>| c.map_or("null".to_string(), |n| n.to_string());
        let mut out = String::from("[");
        for (i, f) in self.table().iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"prefix\":");
            crate::agent::quote(&f.prefix, &mut out);
            out.push_str(&format!(
                ",\"times\":{},\"left\":{},\"hits\":{},\"armed\":{}}}",
                n(f.times),
                n(f.left),
                f.hits,
                f.armed
            ));
        }
        out.push(']');
        out
    }
}

impl<D: DataSource> Runner<D> {
    /// The session's driver faults (LLP 1103).
    pub fn faults(&self) -> &Faults {
        &self.faults
    }

    /// What a host runs for a plain request instead of its transport, when
    /// a driver fault matches its URL (LLP 1103 D1, D2): the failure a
    /// refused connection is, `Failed { kind: Network }`, which a
    /// TypeScript source's `fetch` rejects with as `FetchError('Network')`.
    /// A stream is not matched (D5). The journal says so.
    pub fn fault_dispatch(&mut self, r: &RequestOut) -> Option<Dispatch> {
        let request = &r.request;
        if request.stream || request.is_native() || request.is_auth() || request.surface.is_some() {
            return None;
        }
        if self.faults.is_empty() || !self.faults.take(&request.url) {
            return None;
        }
        let message = format!("fetch failed (driver fault): {}", request.url);
        self.log(message.clone());
        Some(Dispatch::Run(Work::Now(Box::new(move || {
            Outcome::Failed {
                kind: FailureKind::Network,
                message,
            }
        }))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_prefix_decides_counts_run_out_and_pass_keeps_hits() {
        let f = Faults::default();
        f.arm("https://api.test/", None).unwrap();
        f.arm("https://api.test/recipes", Some(1)).unwrap();
        assert!(f.take("https://api.test/recipes/3"));
        let json = f.json();
        assert!(
            json.contains("\"prefix\":\"https://api.test/recipes\""),
            "{json}"
        );
        assert!(json.contains("\"left\":0"), "{json}");
        // Its count spent, the shorter prefix decides.
        assert!(f.take("https://api.test/recipes/3"));
        assert!(!f.take("https://other.test/"));
        assert!(f.pass("https://api.test/"));
        assert!(!f.take("https://api.test/x"));
        assert!(
            f.json()
                .contains("\"left\":null,\"hits\":1,\"armed\":false"),
            "{}",
            f.json()
        );
        // Arming again replaces the entry.
        f.arm("https://api.test/", Some(2)).unwrap();
        assert!(f.json().contains("\"left\":2,\"hits\":0"), "{}", f.json());
        assert!(f.arm("", None).is_err());
        assert!(f.arm("x", Some(0)).is_err());
    }

    #[test]
    fn a_reload_carries_the_table_as_it_is() {
        let f = Faults::default();
        f.arm("https://a.test/", Some(1)).unwrap();
        assert!(f.take("https://a.test/1"));
        f.arm("https://b.test/", None).unwrap();
        f.pass("https://b.test/");
        let carried = Faults::parse(&f.spec()).unwrap();
        assert_eq!(carried.json(), f.json());
        assert!(
            !carried.take("https://a.test/2"),
            "a spent count stays spent"
        );
        assert!(
            !carried.take("https://b.test/"),
            "a passed prefix stays passed"
        );
        assert!(Faults::parse("\t2").is_err());
        assert!(Faults::parse("x\t0").is_err());
        assert!(Faults::parse("x\tmany").is_err());
        let launch = Faults::parse("https://c.test/\nhttps://d.test/\t2\n").unwrap();
        assert!(launch
            .json()
            .contains("\"prefix\":\"https://d.test/\",\"times\":2,\"left\":2"));
    }
}
