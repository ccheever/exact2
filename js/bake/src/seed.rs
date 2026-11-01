//! What the development producer knows about the Rust half of a mixed app,
//! which it cannot run (LLP 1027.002 §5 step 0).
use contract::DataSource;
use exact_js::Module;
use std::collections::BTreeMap;
use std::path::Path;

/// What the development producer knows about the Rust half it cannot run:
/// the previous generation's plan and the sources Rust owns (from the last
/// Cargo bake's receipt). A Rust-owned resource keeps its previous
/// first-frame value until the next Cargo bake; one that has none refuses
/// by name.
pub struct Seed {
    values: BTreeMap<String, Result<exact_plan::Value, String>>,
}

impl Seed {
    /// From the previous plan's bytes and the receipt's `rustSources`.
    pub fn new(plan: &[u8], rust_sources: &[String]) -> Result<Seed, String> {
        let plan = exact_plan::Plan::decode(plan).map_err(|e| format!("previous plan: {e:?}"))?;
        let mut values: BTreeMap<String, Result<exact_plan::Value, String>> = BTreeMap::new();
        for row in plan.resources.iter() {
            let source = plan.str(row.source).to_string();
            if !rust_sources.contains(&source) {
                continue;
            }
            let name = plan.str(row.name).to_string();
            let value = if row.initial.len > 0 {
                exact_plan::Value::from_bytes(plan.bytes(row.initial))
                    .map_err(|e| format!("`{name}`: previous value: {e:?}"))
            } else {
                Err(format!(
                    "`{name}` has no baked value; a Cargo bake supplies it"
                ))
            };
            values
                .entry(source)
                .and_modify(|v| {
                    *v = Err(format!(
                        "`{name}` shares its Rust source with another resource; a Cargo bake resolves it"
                    ))
                })
                .or_insert(value);
        }
        for source in rust_sources {
            values.entry(source.clone()).or_insert_with(|| {
                Err(format!(
                    "`{source}` has no seeded resource; checked by its source declaration"
                ))
            });
        }
        Ok(Seed { values })
    }

    /// From the files on disk, when both exist; `None` otherwise.
    pub fn read(plan: Option<&Path>, receipt: Option<&Path>) -> Result<Option<Seed>, String> {
        let (Some(plan), Some(receipt)) = (plan, receipt) else {
            return Ok(None);
        };
        let (Ok(plan), Ok(receipt)) = (std::fs::read(plan), std::fs::read(receipt)) else {
            return Ok(None);
        };
        let receipt: serde_json::Value =
            serde_json::from_slice(&receipt).map_err(|e| format!("previous receipt: {e}"))?;
        let rust_sources: Vec<String> = receipt["rustSources"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if rust_sources.is_empty() {
            return Ok(None);
        }
        Seed::new(&plan, &rust_sources).map(Some)
    }
}

/// The bake's module beside the seed: Rust-owned sources answer from the
/// previous plan; everything else is the module's.
pub(crate) struct Seeded<'a> {
    pub(crate) module: Module,
    pub(crate) seed: &'a Seed,
}

impl DataSource for Seeded<'_> {
    fn query(
        &mut self,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        match self.seed.values.get(source) {
            Some(Ok(value)) => Ok(value.clone()),
            Some(Err(message)) => Err(exact_runner::DataError::Unavailable(message.clone())),
            None => self.module.query(source, args),
        }
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        if self.seed.values.contains_key(source) {
            return self.query(source, args).map(exact_runner::Answer::Now);
        }
        self.module.answer(store, source, args)
    }
    fn answer_for(
        &mut self,
        target: exact_runner::Target,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        if self.seed.values.contains_key(source) {
            return self.query(source, args).map(exact_runner::Answer::Now);
        }
        self.module.answer_for(target, store, source, args)
    }
    fn overlay(
        &mut self,
        source: &str,
        args: &[exact_plan::Value],
        answer: &exact_plan::Value,
        writes: &[exact_runner::Write<'_>],
    ) -> Result<Option<exact_runner::Overlaid>, exact_runner::DataError> {
        if self.seed.values.contains_key(source) {
            return Ok(None);
        }
        self.module.overlay(source, args, answer, writes)
    }
    fn app_id(&self) -> &str {
        self.module.app_id()
    }
    fn grants(&self) -> &str {
        self.module.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.module.revision()
    }
    fn bind(&mut self, plan: &exact_plan::Plan) {
        self.module.bind(plan)
    }
    fn ready(&self) -> bool {
        self.module.ready()
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.module.canvas_surfaces()
    }
}
