//! A look: `Game::present` declared rather than written in Rust (LLP 1046.009
//! §3.2, an experiment). A `.look` file is checked against the game's
//! registered types and compiled to a program the engine runs inside
//! `Game::present`, through `Present`'s read-only view: it can read the
//! simulation and write presentation rows (`Offset`, `Opacity`), and nothing
//! else, by construction. Replacing its source changes what the running game
//! draws on the next present, with no Cargo build and no change to the world.
//!
//! ```text
//! let player = named("player")
//! guard some(player)
//! let t = seconds()
//! each e, part in Part
//!   Offset(e, rotation=pitch(part.swing * sin(t * 3 + part.phase)))
//! ```
mod check;
mod eval;
mod report;
mod syntax;
mod value;

pub use check::Rule;
pub use eval::{Cache, Stats};
pub use syntax::{Error, Span};

use exact_game::{Present, Registered};
use std::time::Instant;

/// The game's own pure functions and constants a look may call by name: an
/// escape hatch to Rust that cannot read or write the world (numbers in,
/// a number out).
/// A pure function of numbers.
pub type Pure = fn(&[f32]) -> f32;

#[derive(Clone, Default)]
pub struct Externs {
    functions: Vec<(&'static str, usize, Pure)>,
    constants: Vec<(&'static str, f32)>,
}
impl Externs {
    pub fn new() -> Self {
        Self::default()
    }
    /// A pure function of `arity` numbers.
    pub fn function(mut self, name: &'static str, arity: usize, f: Pure) -> Self {
        assert!(arity <= 8, "an extern takes at most eight numbers");
        self.functions.push((name, arity, f));
        self
    }
    /// A named constant (the game's own, so a look never restates it).
    pub fn constant(mut self, name: &'static str, value: f32) -> Self {
        self.constants.push((name, value));
        self
    }
    pub(crate) fn constant_of(&self, name: &str) -> Option<f32> {
        self.constants
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    }
    pub(crate) fn function_of(&self, name: &str) -> Option<usize> {
        self.functions.iter().position(|(n, _, _)| *n == name)
    }
    pub(crate) fn arity(&self, i: usize) -> usize {
        self.functions[i].1
    }
    pub(crate) fn call(&self, i: usize, args: &[f32]) -> f32 {
        (self.functions[i].2)(args)
    }
}

/// A checked look, ready to present.
pub struct Look {
    prog: check::Program,
    externs: Externs,
}
impl Look {
    /// Parse and check `source` against the game's registered types
    /// (`Present::registered` or `World::registered`): every refusal, with its
    /// line and column.
    pub fn compile(
        source: &str,
        types: &[Registered],
        externs: &Externs,
    ) -> Result<Look, Vec<Error>> {
        let file = syntax::parse(source).map_err(|e| vec![e])?;
        let prog = check::check(&file, types, externs)?;
        Ok(Look {
            prog,
            externs: externs.clone(),
        })
    }
    /// Write this look's rows. Runtime failures (a missing entity a rule read)
    /// are appended to `errors`; the rest of the look still presents.
    pub fn present(&self, p: &mut Present<'_>, cache: &mut Cache, errors: &mut Vec<String>) {
        eval::present(&self.prog, &self.externs, p, cache, errors);
    }
    /// The top-level rules, with what each reads.
    pub fn rules(&self) -> &[Rule] {
        &self.prog.rules
    }
    /// What each rule reads, whether it can be kept between presents, and for
    /// each row that follows the time, the part a GPU could evaluate per frame
    /// over per-instance parameters the CPU computes when their inputs change.
    pub fn report(&self) -> String {
        report::report(&self.prog)
    }
}

/// A look as a running game holds it: its source, the checked look, and a
/// replacement waiting for the next present. A replacement that is refused
/// leaves the running look in place and says why.
pub struct Live {
    source: String,
    pending: Option<String>,
    look: Option<Look>,
    externs: Externs,
    cache: Cache,
    errors: Vec<String>,
    /// Microseconds the last compile took.
    pub compile_us: f64,
}
impl Live {
    pub fn new(source: impl Into<String>, externs: Externs) -> Self {
        let source = source.into();
        Self {
            pending: Some(source.clone()),
            source,
            look: None,
            externs,
            cache: Cache::default(),
            errors: Vec::new(),
            compile_us: 0.0,
        }
    }
    /// Replace the source; the next present checks it against the world's types.
    pub fn replace(&mut self, source: impl Into<String>) {
        self.pending = Some(source.into());
    }
    /// The source now presenting.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Refusals and runtime failures since the last present that compiled.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
    pub fn look(&self) -> Option<&Look> {
        self.look.as_ref()
    }
    pub fn stats(&self) -> Stats {
        self.cache.stats
    }
    /// Microseconds each rule took in the last present (0 when kept).
    pub fn rule_us(&self) -> &[f64] {
        &self.cache.rule_us
    }
    /// Present: compile a waiting replacement first.
    pub fn present(&mut self, p: &mut Present<'_>) {
        if let Some(source) = self.pending.take() {
            let started = Instant::now();
            let compiled = Look::compile(&source, &p.registered(), &self.externs);
            self.compile_us = started.elapsed().as_secs_f64() * 1e6;
            self.errors.clear();
            match compiled {
                Ok(look) => {
                    self.look = Some(look);
                    self.source = source;
                    self.cache = Cache::default();
                }
                Err(errors) => self.errors.extend(
                    errors
                        .iter()
                        .map(|e| format!("look refused, the last one stays: {e}")),
                ),
            }
        }
        if let Some(look) = &self.look {
            look.present(p, &mut self.cache, &mut self.errors);
        }
    }
}
