//! Messages on the shared Linux host, with the native Snapback device.
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, InFlight, Interrupt, Outcome, Store, Target};
include!(concat!(env!("OUT_DIR"), "/module.rs"));
#[path = "../../native.rs"]
mod native;
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

struct Messages(exact_js::Module);
impl Default for Messages {
    fn default() -> Self {
        Self(native::module(BYTECODE, APP, GRANTS))
    }
}
impl DataSource for Messages {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.0.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.0.answer(store, source, args)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.0.parse(store, source, args, outcome)
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.0.answer_for(target, store, source, args)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.0.parse_for(target, store, source, args, outcome)
    }
    fn bind(&mut self, plan: &Plan) {
        self.0.bind(plan);
    }
    fn app_id(&self) -> &str {
        APP
    }
    fn grants(&self) -> &str {
        GRANTS
    }
    fn revision(&self) -> Option<&str> {
        self.0.revision()
    }
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        self.0.replacement(plan, receipt, module).map(Self)
    }
    fn ready(&self) -> bool {
        self.0.ready()
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.0.activate()
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.0.activate_for_validation()
    }
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        self.0.configure_storage(data, cache, temporary)
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.0.continuation(token)
    }
    fn interrupt(&self) -> Option<Interrupt> {
        self.0.interrupt()
    }
    fn forgotten(&mut self, in_flight: &[InFlight<'_>]) {
        self.0.forgotten(in_flight);
    }
}
type ExactEmbeddedData = Messages;
fn embedded_data() -> ExactEmbeddedData {
    Messages::default()
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));

fn main() {
    std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT));
}
