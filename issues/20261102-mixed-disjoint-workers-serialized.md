# Mixed serializes worker sources with disjoint secrets

**Status:** Closed
**Severity:** P2
**Systems:** Data composition, Worker scheduling
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

A mixed app with both data executors placed on workers gets one shared
queue even when their `secret.keep` names do not overlap. A slow turn in
one executor consequently delays unrelated answers from the other.

The governing comment explicitly says disjoint workers have separate sets
and run concurrently (`data/src/mixed.rs:196-199`). The condition at line
208 instead merges any pair of workers, independently of `overlap`.
`dispatch` then returns `Held` for the second executor until the first
turn's envelope commits (`mixed.rs:697-705`).

Reproduce using the public API and existing artifacts:

```rust
use exact_data::{Mixed, Placed};
use exact_runner::{Answer, DataError, DataSource, Dispatch, Placement, Store, Value};
struct Source(&'static str);
impl DataSource for Source {
    fn app_id(&self) -> &str { "test.disjoint" }
    fn grants(&self) -> &str { self.0 }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Unit)
    }
}
fn token(answer: Answer) -> u64 {
    let Answer::Later(r) = answer else { panic!("expected continuation") };
    r.continuation.unwrap()
}
fn main() {
    let a = Placed::new(Source("secret.keep a"), Placement::Worker);
    let b = Placed::new(Source("secret.keep b"), Placement::Worker);
    let mut pair = Mixed::new(a, b, &["a"], &["b"]).unwrap();
    pair.activate().unwrap();
    let mut store = Store::new(pair.grants(), []);
    let a = token(pair.answer(&mut store, "a", &[]).unwrap());
    let b = token(pair.answer(&mut store, "b", &[]).unwrap());
    let first = pair.dispatch(a, &store);
    let second = pair.dispatch(b, &store);
    println!("first dispatch runs={}; disjoint second held={}",
        matches!(first, Dispatch::Run(_)), matches!(second, Dispatch::Held));
    assert!(matches!(second, Dispatch::Held));
}
```

Saved as `/tmp/exact-review-mixed-disjoint.rs` and run with:

```sh
rustc --edition=2021 /tmp/exact-review-mixed-disjoint.rs --crate-name exact_review_mixed_disjoint -L dependency=target/debug/deps --extern exact_data=target/debug/deps/libexact_data-91cd9ac849def138.rlib --extern exact_runner=target/debug/deps/libexact_runner-ea99ef2cb22d8243.rlib -o /tmp/exact-review-mixed-disjoint
/tmp/exact-review-mixed-disjoint
```

Observed: `first dispatch runs=true; disjoint second held=true`. Both
sources are actual `Placed` worker instances; the probe checks dispatch
admission without timing or executing their turns. It proves unnecessary
serialization, not a measured latency increase.

Merge the workers' sets when their secret names overlap; otherwise create
one set per worker, as the existing separate-set branch does. Existing
tests cover a disjoint worker/main pair but omit two disjoint workers. No
duplicate was found in the current or either related issue corpus,
including closed tickets.

## Resolution

Worker executors share an ordered set only when their secret names overlap. Disjoint workers retain separate sets.

## Verification

`disjoint_workers_dispatch_without_waiting_for_each_other` passed with two actual Placed owner threads. Channels establish that both enter before either is released, and both replies commit. Existing overlapping-secret ordering tests passed.
