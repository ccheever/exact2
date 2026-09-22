//! Tally's unchanged game, with an explicit failure-injection test action.
pub mod metrics;
use exact_runner::{Answer, DataError, DataSource, Store, Value};
use exact_world::{args::SetupArgs, Action, Game, Input, World};
use tally_logic::{Options, Tally};
use world_source::WorldSource;

pub struct Probe;
impl Game for Probe {
    const ID: &'static str = Tally::ID;
    type Args = Options;
    const ACTIONS: &'static [Action] = &[
        Action::button("draw", &["KeyD"]),
        Action::button("hold", &["KeyH"]),
        Action::button("reset", &["KeyR"]),
        Action::button("fail", &[]),
    ];
    fn register(w: &mut World, a: SetupArgs<'_, Options>) -> Result<(), exact_world::DataError> {
        Tally::register(w, a)
    }
    fn setup(w: &mut World, a: &Options) -> Result<(), exact_world::DataError> {
        Tally::setup(w, a)
    }
    fn tick(w: &mut World, i: &Input, a: &Options) -> Result<(), exact_world::DataError> {
        if i.pressed("fail") {
            return Err(exact_world::DataError::new("injected Tally tick failure"));
        }
        Tally::tick(w, i, a)
    }
}
pub struct TallySource(pub WorldSource<Probe>);
impl Default for TallySource {
    fn default() -> Self {
        let source = Self(
            WorldSource::new(
                || Options { seed: 7 },
                &["score", "hand", "pile_count", "over", "ticks"],
            )
            .unwrap(),
        );
        metrics::first_tick();
        source
    }
}
impl DataSource for TallySource {
    fn app_id(&self) -> &str {
        "com.exact.x1-tally"
    }
    fn grants(&self) -> &str {
        self.0.grants()
    }
    fn query(&mut self, s: &str, a: &[Value]) -> Result<Value, DataError> {
        if s == "metrics" {
            Ok(metrics::value())
        } else {
            self.0.query(s, a)
        }
    }
    fn answer(&mut self, store: &mut Store, s: &str, a: &[Value]) -> Result<Answer, DataError> {
        if s == "metrics" {
            Ok(Answer::Now(metrics::value()))
        } else {
            let out = self.0.answer(store, s, a);
            #[cfg(not(target_arch = "wasm32"))]
            {
                metrics::trace(self, s, a);
                metrics::live_save(self, store, s, a)?;
            }
            out
        }
    }
}
