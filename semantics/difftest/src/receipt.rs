//! Reproduction inputs and denominators for an app verification run.

use crate::{census::Census, property::Report, verify::Options};
use contract_syntax::{PropertyDecl, PropertyKind};
use exact_plan::Plan;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct Identity {
    app: String,
    tests: String,
    embedding: String,
    commit: String,
}
impl Identity {
    pub fn new(file: &Path, source: &str, files: &[PathBuf]) -> Result<Self, String> {
        let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let mut tests = Vec::new();
        let mut files: Vec<_> = files.iter().filter(|p| *p != file && p.is_file()).collect();
        files.sort();
        for path in files {
            tests.extend(std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        let embedding = contract::lean::lean_path(file, "app")
            .map(|s| hash(s.as_bytes()))
            .unwrap_or_else(|e| format!("unavailable ({e})"));
        let commit = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        Ok(Self {
            app: hash(source.as_bytes()),
            tests: hash(&tests),
            embedding,
            commit,
        })
    }
}
#[derive(Default)]
pub(crate) struct Delivery {
    pub settle_bounds: usize,
    pub until_met: usize,
    pub until_not_met: usize,
    pub until_field_paths: usize,
    pub mutation_events: usize,
    pub mutation_bounds: usize,
    pub whole: usize,
    pub partial: Vec<String>,
    pub skipped: Vec<String>,
    pub walks: usize,
    pub stores: usize,
}
#[derive(Default)]
pub(crate) struct Semantics {
    pub reason: Option<String>,
    pub run: usize,
    pub agreed: usize,
    pub outside: Vec<String>,
    pub trace_disagreed: bool,
    pub seconds: f64,
    pub candidates: usize,
}
pub(crate) struct Receipt<'a> {
    pub file: &'a str,
    pub options: &'a Options,
    pub seed: u64,
    pub identity: &'a Identity,
    pub delivery: &'a Delivery,
    pub census: &'a Census,
    pub plan: &'a Plan,
    pub never: &'a BTreeSet<String>,
    pub properties: &'a [PropertyDecl],
    pub property_files: &'a [PathBuf],
    pub report: &'a Report,
    pub semantics: &'a Semantics,
    pub js: &'a str,
    pub runner_seconds: f64,
    pub candidates: usize,
    pub precision: &'a crate::suggest::Precision,
}
impl Receipt<'_> {
    pub fn print(&self) {
        let Self {
            options: o,
            identity: id,
            delivery: d,
            census: c,
            semantics: s,
            ..
        } = self;
        let mut command = format!(
            "contract verify {} --walks {} --steps {} --walk-seed 0x{:016x} --confirm {}",
            shell(self.file),
            o.walks,
            o.steps,
            self.seed,
            o.confirm
        );
        if o.runner_only {
            command.push_str(" --runner-only");
        }
        if o.js {
            command.push_str(" --js");
        }
        if o.types {
            command.push_str(" --types");
        }
        if let Some(module) = &o.prove {
            command.push_str(&format!(" --prove {}", shell(module)));
        }
        println!("verify receipt — reproduce: {command}");
        println!(
            "  app sha256:{}  tests sha256:{}  embedding sha256:{}  exact2 {}",
            id.app, id.tests, id.embedding, id.commit
        );
        println!("  scripts: {} tests delivered ({} whole; {} in part: {}; {} skipped: {}), 1 explored, {} walks", d.whole+d.partial.len(), d.whole, d.partial.len(), names(&d.partial), d.skipped.len(), names(&d.skipped), d.walks);
        println!(
            "  delivery: {} settle bounds; {} test-clock advances, {} bounds",
            d.settle_bounds, d.mutation_events, d.mutation_bounds
        );
        println!(
            "  clock until: {} met, {} not met, {} field path",
            d.until_met, d.until_not_met, d.until_field_paths
        );
        println!("  steps: {} ({} with more than one commit; {} poisoned; {} with a commit the trace leaves out); stores: empty, {} seeded", c.steps, c.multiple, c.poisoned, c.omitted, d.stores);
        let declared: BTreeSet<_> = self
            .plan
            .actions
            .iter()
            .map(|a| self.plan.str(a.name))
            .collect();
        let committed = declared
            .iter()
            .filter(|a| c.committed.contains(**a))
            .count();
        let clocks: BTreeSet<_> = self
            .plan
            .timers
            .iter()
            .map(|t| self.plan.str(self.plan.action(t.action).name))
            .collect();
        let fired = clocks.iter().filter(|a| c.tasks.contains(**a)).count();
        println!("  actions: committed {committed} of {} (never reached: {}); clock actions {fired} of {}", declared.len(), names(&self.never.iter().cloned().collect::<Vec<_>>()), clocks.len());
        let (mut held, mut false_, mut counted, mut idle) = (0, 0, 0, 0);
        let mut details = Vec::new();
        for ((p, file), counts) in self
            .properties
            .iter()
            .zip(self.property_files)
            .zip(&self.report.counts)
        {
            if counts.iter().any(|c| c.false_ > 0) {
                false_ += 1;
            } else if counts.iter().all(|c| c.checked > 0) {
                held += 1;
            } else if counts.iter().any(|c| c.counted > 0) {
                counted += 1;
            } else {
                idle += 1;
            }
            for (i, c) in counts.iter().enumerate() {
                if matches!(p.kind, PropertyKind::After(_) | PropertyKind::Clock) || c.counted > 0 {
                    let scope = match &p.kind {
                        PropertyKind::After(a) => format!("after {a:?}"),
                        PropertyKind::Clock => "after clock".into(),
                        _ => format!("property {:?}", p.name),
                    };
                    let span = p.lines.get(i).map_or(p.span, |l| l.span);
                    details.push(format!(
                        "{scope} {}:{}:{}: {} counted only, {} checked",
                        crate::verify::show(file),
                        span.line,
                        span.col,
                        c.counted,
                        c.checked
                    ));
                }
            }
        }
        println!("  properties: {held} held, {false_} false, {counted} counted only, {idle} not exercised{}", if details.is_empty() { String::new() } else { format!(" ({})", details.join("; ")) });
        self.precision.print();
        let count = |class| c.classes.get(class).copied().unwrap_or(0);
        println!("  refusals: {} of the app's own (above); {} of class T; {} of the world's; {} limit; {} poisoned; {} cycle; {} value; {} unclassified", count("app"), count("type"), count("world"), count("limit"), count("poisoned"), count("cycle"), count("value"), count("unclassified"));
        println!("  world: seeded oracle answering at once and never failing; tap, type, land and clock events only; no reload, no late answer, no host input; {}", crate::settle::HOST_WORLD);
        let cost = format!(
            "runner {:.3}s; semantics {:.3}s; shrink candidates: {} runner, {} two-engine",
            self.runner_seconds, s.seconds, self.candidates, s.candidates
        );
        if let Some(reason) = &s.reason {
            println!(
                "  semantics: did not run: {reason}; {cost}; js: {}",
                self.js
            );
        } else {
            let outside: BTreeSet<_> = s.outside.iter().cloned().collect();
            println!("  semantics: agreed on {} of {} scripts, {} outside ({}); configurations {} advance on every step; {cost}; js: {}", s.agreed, s.run, s.outside.len(), names(&outside.into_iter().collect::<Vec<_>>()), if s.trace_disagreed { "did not match" } else { "matched" }, self.js);
        }
    }
}
fn names(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join("; ")
    }
}
fn shell(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
    {
        s.into()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
