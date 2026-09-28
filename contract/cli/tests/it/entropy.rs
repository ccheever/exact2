//! LLP 1069.005 D2: an answer that drew secure randomness is a device read
//! whose value bake leaves out of the plan — unlike a store read, whose
//! empty-store answer is compiled as the fresh install's placeholder — so
//! no random value is shared by every install, and the device asks it.

use exact_kernel::Kernel;
use exact_runner::{Answer, DataError, DataSource, Runner, Store, Value};

const SRC: &str = r#"
component App
  resource id = token() as shape string
  resource session = session() as shape string
  resource uuid = uuid() as shape string
  resource digest = digest() as shape number
  view
    column
      text id testId="id"
      text session testId="session"
      text uuid testId="uuid"
      text "{digest}" testId="digest"
"#;

/// `token` draws (as `crypto.randomUUID` does on every executor);
/// `session` reads the device's store.
#[derive(Default)]
struct Device {
    draws: u32,
}

impl DataSource for Device {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "token" => {
                self.draws += 1;
                store.observe_entropy();
                Ok(Answer::Now(Value::str(&format!("draw {}", self.draws))))
            }
            "session" => {
                store.observe_external_read();
                Ok(Answer::Now(Value::str("signed out")))
            }
            // D5: a Rust source's own helpers, marked as TypeScript's are.
            "uuid" => exact_data::crypto::random_uuid(store).map(|id| Answer::Now(Value::str(&id))),
            "digest" => {
                let bytes = exact_data::crypto::digest(exact_data::crypto::Sha::Sha256, b"abc");
                Ok(Answer::Now(Value::Number(bytes[0] as f64)))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

#[test]
fn bake_compiles_no_value_that_drew_randomness() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Device::default()).unwrap();
    let row = |name: &str| {
        plan.resources
            .iter()
            .find(|r| plan.str(r.name) == name)
            .unwrap()
            .clone()
    };
    assert!(row("id").reader && row("id").initial.len == 0);
    assert!(row("session").reader && row("session").initial.len > 0);
    // D5: `exact_data::crypto` marks a Rust source's draw the same way; a
    // digest is pure and compiled.
    assert!(row("uuid").reader && row("uuid").initial.len == 0);
    assert!(!row("digest").reader && row("digest").initial.len > 0);

    // The device answers it, with its own draw.
    let runner = Runner::boot(
        plan,
        Device { draws: 41 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(runner.resource("id"), Some(&Value::str("draw 42")));
    assert!(runner.resource_draws_entropy("id") && runner.resource_reads_store("id"));
    assert!(!runner.resource_draws_entropy("session"));
}
