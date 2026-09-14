//! Explicit source ownership across two executors, without depending on either engine.
//! @ref LLP 1027 D8 / LLP 1027.001 / LLP 1029.000 — paired replacement.

use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

type Retain<R> = fn(&R) -> Result<R, DataError>;

/// Two data executors with disjoint, declared source names and one app identity.
/// Each child retains its own grants; hosts receive their union. Construction
/// never probes an answer to discover ownership, so writes cannot happen twice.
pub struct Mixed<J, R> {
    javascript: J,
    rust: R,
    sources: BTreeMap<String, bool>,
    app_id: String,
    grants: String,
    revision: String,
    retain_rust: Option<Retain<R>>,
    continuations: BTreeMap<u64, (bool, u64)>,
    next_continuation: u64,
}

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

impl<J: DataSource, R: DataSource> Mixed<J, R> {
    /// Require a complete JavaScript/Rust pair for every replacement.
    pub fn new(
        javascript: J,
        rust: R,
        javascript_sources: &[&str],
        rust_sources: &[&str],
    ) -> Result<Self, DataError> {
        let mut sources = BTreeMap::new();
        for (names, owner) in [(javascript_sources, false), (rust_sources, true)] {
            for name in names {
                if name.is_empty() || sources.insert((*name).into(), owner).is_some() {
                    return Err(unavailable(format!(
                        "duplicate or empty data source: {name}"
                    )));
                }
            }
        }
        Self::construct(javascript, rust, sources, None)
    }

    /// Disable Rust replacement and explicitly preserve its state when replacing
    /// JavaScript. `retain` must make an isolated candidate (for example a clone);
    /// it must not reset live state or share mutable state with validation.
    pub fn with_embedded_rust(mut self, retain: Retain<R>) -> Self {
        self.retain_rust = Some(retain);
        self
    }

    fn construct(
        javascript: J,
        rust: R,
        sources: BTreeMap<String, bool>,
        retain_rust: Option<Retain<R>>,
    ) -> Result<Self, DataError> {
        if javascript.app_id() != rust.app_id() {
            return Err(unavailable(
                "mixed executors must declare the same app identity",
            ));
        }
        let app_id = javascript.app_id().into();
        let grants = [javascript.grants(), rust.grants()]
            .into_iter()
            .flat_map(str::lines)
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n");
        // If Rust adds no capabilities, retain the existing JS receipt spelling.
        // Otherwise the union is deterministic; each child keeps its own spelling.
        let grants = if grants.lines().all(|line| {
            javascript
                .grants()
                .lines()
                .map(str::trim)
                .any(|grant| grant == line)
        }) {
            javascript.grants().to_owned()
        } else {
            grants
        };
        let revision = format!(
            "mixed:{}",
            serde_json::to_string(&(javascript.revision(), rust.revision())).unwrap()
        );
        Ok(Self {
            javascript,
            rust,
            sources,
            app_id,
            grants,
            revision,
            retain_rust,
            continuations: BTreeMap::new(),
            next_continuation: 1,
        })
    }

    fn owner(&self, source: &str) -> Result<bool, DataError> {
        self.sources
            .get(source)
            .copied()
            .ok_or_else(|| DataError::UnknownSource(source.into()))
    }

    fn route_answer(&mut self, rust: bool, mut answer: Answer) -> Result<Answer, DataError> {
        if let Answer::Later(request) = &mut answer {
            let grants = if rust {
                self.rust.grants()
            } else {
                self.javascript.grants()
            };
            if let Some(scope) = &request.grants {
                if scope.lines().map(str::trim).any(|line| {
                    !line.is_empty() && !grants.lines().map(str::trim).any(|grant| grant == line)
                }) {
                    return Err(unavailable("nested request exceeds its executor's grants"));
                }
            } else {
                request.grants = Some(grants.into());
            }
            if let Some(child) = request.continuation {
                let token = self.next_continuation;
                self.next_continuation = token
                    .checked_add(1)
                    .ok_or_else(|| unavailable("mixed continuation tokens exhausted"))?;
                self.continuations.insert(token, (rust, child));
                request.continuation = Some(token);
            }
        }
        Ok(answer)
    }
}

impl<J: DataSource, R: DataSource> DataSource for Mixed<J, R> {
    fn preload(&self) -> Result<bool, DataError> {
        let javascript = self.javascript.preload()?;
        let rust = self.rust.preload()?;
        Ok(javascript && rust)
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if self.owner(source)? {
            self.rust.query(source, args)
        } else {
            self.javascript.query(source, args)
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let rust = self.owner(source)?;
        let answer = if rust {
            let grants = self.rust.grants().to_owned();
            store.with_grants(&grants, |store| self.rust.answer(store, source, args))?
        } else {
            let grants = self.javascript.grants().to_owned();
            store.with_grants(&grants, |store| self.javascript.answer(store, source, args))?
        };
        self.route_answer(rust, answer)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let rust = self.owner(source)?;
        let answer = if rust {
            let grants = self.rust.grants().to_owned();
            store.with_grants(&grants, |store| {
                self.rust.parse(store, source, args, outcome)
            })?
        } else {
            let grants = self.javascript.grants().to_owned();
            store.with_grants(&grants, |store| {
                self.javascript.parse(store, source, args, outcome)
            })?
        };
        self.route_answer(rust, answer)
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.javascript.ready() && self.rust.ready()
    }

    fn bind(&mut self, plan: &Plan) {
        self.javascript.bind(plan);
        self.rust.bind(plan);
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.javascript.activate()?;
        self.rust.activate()
    }

    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.javascript.activate_for_validation()?;
        self.rust.activate_for_validation()
    }

    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        self.javascript
            .configure_storage(data.clone(), cache.clone(), temporary.clone())?;
        self.rust.configure_storage(data, cache, temporary)
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let (rust, child) = self.continuations.remove(&token)?;
        if rust {
            self.rust.continuation(child)
        } else {
            self.javascript.continuation(child)
        }
    }

    fn continuation_token(&mut self, token: u64) -> Option<u64> {
        let (rust, child) = self.continuations.remove(&token)?;
        if rust {
            self.rust.continuation_token(child)
        } else {
            self.javascript.continuation_token(child)
        }
    }

    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        if receipt.len() > 1024 * 1024
            || module.len() > 32 * 1024 * 1024
            || plan.len() > 32 * 1024 * 1024
        {
            return Err(unavailable("mixed replacement exceeds its byte limit"));
        }
        let metadata: Json = serde_json::from_str(receipt)
            .map_err(|error| unavailable(format!("invalid mixed receipt: {error}")))?;
        let (javascript, rust) = if metadata["kind"] != "mixed" {
            let retain = self.retain_rust.ok_or_else(|| {
                unavailable("mixed replacement requires a paired JavaScript and Rust generation")
            })?;
            if metadata["kind"] == "rust" {
                return Err(unavailable("Rust replacement is disabled for this host"));
            }
            (
                self.javascript.replacement(plan, receipt, module)?,
                retain(&self.rust)?,
            )
        } else {
            if self.retain_rust.is_some() {
                return Err(unavailable("Rust replacement is disabled for this host"));
            }
            if metadata["version"] != 1
                || !metadata["javascript"].is_object()
                || !metadata["rust"].is_object()
                || metadata["rust"]["kind"] != "rust"
            {
                return Err(unavailable("invalid paired mixed receipt"));
            }
            let split = metadata["javascriptBytes"]
                .as_u64()
                .and_then(|length| usize::try_from(length).ok())
                .filter(|length| *length > 0 && *length < module.len())
                .ok_or_else(|| unavailable("invalid paired mixed module boundary"))?;
            (
                self.javascript.replacement(
                    plan,
                    &metadata["javascript"].to_string(),
                    module[..split].to_vec(),
                )?,
                self.rust.replacement(
                    plan,
                    &metadata["rust"].to_string(),
                    module[split..].to_vec(),
                )?,
            )
        };
        if javascript.app_id() != self.javascript.app_id()
            || rust.app_id() != self.rust.app_id()
            || javascript.grants().trim() != self.javascript.grants().trim()
            || rust.grants().trim() != self.rust.grants().trim()
        {
            return Err(unavailable(
                "mixed replacement changes admitted identity or grants",
            ));
        }
        Self::construct(javascript, rust, self.sources.clone(), self.retain_rust)
    }
}
