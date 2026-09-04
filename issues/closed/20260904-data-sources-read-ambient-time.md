# TypeScript data sources read time and randomness outside the runner

**Status:** Closed
**Resolution:** Guard ambient Date/Math.random before Hermes module evaluation; explicit time/seed inputs verified through bake, cache, refresh, async replies and reload (LLP 1027.000).
**Systems:** TypeScript executor, Runner, Bake, Agent API
**Severity:** P1
**Author:** Codex, at Charlie Cheever's request
**Date:** 2026-09-04
**Related:** LLP 1027 D1a/D4/D10; LLP 1027.000; LLP 1012 §2

An answer can read `Date.now()` and `Math.random()` without either appearing
in its arguments or the runner's controlled clock. The native shim creates
the Hermes runtime with its builtins; the prelude does not mediate those
reads. LLP 1027's stage-3 note lists the clock shadow as owed.

Reproduced on 2026-09-04 with current `js/src/shim.cc`, the lean Hermes
libraries selected by `js/build.rs`, and the current prelude compiled by
that build's `hermesc`. Load a module whose `answer()` returns
`{ wall: Date.now(), random: Math.random() }`; invoke the existing
`__exact_call("probe", "[]", "")` twice. Both calls succeed with machine
epoch time and different random values. No runner time/seed is passed.
The module/bytecode loading pattern is already in `js/tests/caltrain.rs`.

The consequences include unrepeatable answers at a frozen agent clock and
an undeclared input that bake/resource caching cannot account for. Merely
replacing epoch time with the runner's elapsed milliseconds would introduce
a second semantic error. Async continuation ownership also needs a decision.

LLP 1027.000 is the accepted fix: explicit time/seed arguments and refusal of
ambient reads, with runner-backed globals costed as the alternative.
Done when the accepted design covers module initialization, bake, ordinary
answers, async continuations, resource caching, and reload on each advertised
executor. Verify actual execution, including aliases/global-object access,
using the existing fixtures. Do not close this from native-only evidence
while claiming browser coverage, or from a source-pattern ban alone.

Implemented 2026-09-04 in the existing macOS Hermes executor. Its prelude
installs guards before app bytecode loads, including Date's prototype
constructor; explicit-value Date/UTC computations still use Hermes itself.
Fourteen ambient forms refuse during initialization, direct calls, bake,
and asynchronous fetch continuations. Six new integration tests also drive
explicit inputs through bake, runner caching, refresh, clock changes,
reload carry, and stale async completion. All 18 exact-js tests pass.

The browser TypeScript executor remains unbuilt; iOS/Linux or missing-engine
builds retain the existing named refusing stub. This closes the defect in
the currently implemented executor and does not claim cross-target parity.
Parent LLP 1027's browser design now explicitly records that limitation.
