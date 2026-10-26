//! `let` and `const` mean on Hermes what they mean in the language: a closure
//! made in a loop keeps its own iteration's binding. Hermes compiles them as
//! `var` unless the bake asks for ES6 block scoping, and Bun's
//! JavaScriptCore, which runs an app's unit tests, cannot show the difference.
use exact_js::Paired;
use exact_js_bake::{bake, Baked, Producer, Tools};
use exact_kernel::Kernel;
use exact_runner::{Runner, Value};
use std::path::PathBuf;

const APP: &str = "test.exact.scoping";
const CONTRACT: &str = r#"
component App
  state count = 0
  resource shapes = shapes(count) as shape string
  resource awaited = awaited(count) as shape string
  action increment
    count = count + 1
  view
    column
      text shapes testId="shapes"
      text awaited testId="awaited"
      button "Increment" press=increment testId="increment"
"#;
/// Each shape reads back the closures it made in a loop; with `var`
/// scoping every closure would read the last iteration's value.
const SOURCE: &str = r#"
import type { Sources, Answer } from './app.contract.d.ts';
export const appId = 'test.exact.scoping';
export const grants = '';
const read = (made: (() => unknown)[]) => made.map(f => f()).join(',');
function* yielded(names: string[]) {
  for (const name of names) yield () => name;
}
const sources: Sources = {
  shapes: ([count]) => {
    const byEntry = new Map([['a', { k: 'A' }], ['b', { k: 'B' }]]);
    const ofEntries: (() => unknown)[] = [];
    for (const entry of byEntry.values()) ofEntries.push(() => entry.k);
    const counted: (() => unknown)[] = [];
    for (let i = 0; i < 6; i++) { counted.push(() => i + count); i++; }
    const bodies: (() => unknown)[] = [];
    let n = 0;
    while (n < 3) { const tens = n * 10; bodies.push(() => tens); n++; }
    const nested: (() => unknown)[] = [];
    for (const a of [1, 2]) for (const b of [3, 4]) nested.push(() => a * 10 + b);
    const caught: (() => unknown)[] = [];
    for (const x of ['p', 'q']) { try { throw x; } catch (error) { const held = error; caught.push(() => held); } }
    return [read(ofEntries), read(counted), read(bodies), read(nested), read(caught), read([...yielded(['g', 'h'])])].join(' ');
  },
  awaited: async ([count]) => {
    const made: (() => unknown)[] = [];
    for (const name of ['s', 't']) { await Promise.resolve(); made.push(() => name + count); }
    return read(made);
  },
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("exact-scoping-test-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("app.contract"), CONTRACT).unwrap();
        std::fs::write(path.join("app.ts"), SOURCE).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn expected(count: u32) -> [Option<Value>; 2] {
    let shapes = format!(
        "A,B {},{},{} 0,10,20 13,14,23,24 p,q g,h",
        1 + count,
        3 + count,
        5 + count
    );
    [
        Some(Value::str(&shapes)),
        Some(Value::str(&format!("s{count},t{count}"))),
    ]
}

fn read(runner: &Runner<exact_js::Module>) -> [Option<Value>; 2] {
    ["shapes", "awaited"].map(|name| runner.resource(name).cloned())
}

/// The bindings the bake evaluated into the plan, then the ones the loaded
/// module answers after an action.
fn assert_per_iteration(baked: &Baked) {
    let paired =
        Paired::decode(&baked.receipt, &baked.plan, baked.bytecode.clone(), APP, "").unwrap();
    let mut live = Runner::boot(
        paired.plan,
        paired.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(read(&live), expected(0), "the baked first frame");
    live.data().load().unwrap();
    live.data_ready().unwrap();
    live.act("increment", vec![]).unwrap();
    assert_eq!(read(&live), expected(1), "the loaded module's answers");
}

#[test]
fn closures_made_in_loops_keep_their_own_iteration_in_both_producers() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    let standalone = bake(&f.0, &Tools::default()).unwrap();
    assert_per_iteration(&standalone);
    let resident = Producer::new(Tools::default())
        .unwrap()
        .bake(&f.0, None)
        .unwrap();
    assert_eq!(resident.bytecode, standalone.bytecode);
}
