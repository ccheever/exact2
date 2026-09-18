# Lanterns change trials — interleaved after measurement

Measurement in progress. The first completed after session passes Task A's nine
functional checks, but this is not a strict comparison pass: the unchanged
version-3 evaluator still finds the shared direct crate-route failure, and this
headless host cannot provide world pixels. No conclusion about an overall speed
improvement is warranted from the first attempt.

After is pinned to `7c65726ca8be72a3f10de3a44a836e1b3b00eed5`, baseline to
`e47a09fc36c07103cdc3645b7061cca305af6c14`. The dispatch order is after A1,
baseline A3, after B1, baseline B3, after A2, after B2. Every dispatch uses a fresh
`gpt-5.6-sol` high session, a 900-second ceiling, and the original task prompt.
The retained original A1/A2 prompts agree byte-for-byte, as do B1/B2; copies now
live in this harness, avoiding reads from another lane. The launcher scratch
path changed to `scratch/I2/trials`. No evaluator changed: the six hashes match
all original submissions' final version-3 evaluator receipts. Setup/control time
is outside agent time; the first setup was cold because this clone had no target.
Every trial warms its Linux build before dispatch. Trials run one at a time;
source/auth/build copies are deleted after evaluation, with receipts retained.

| Cohort | Attempt | Agent s | Parent build s | Eval s | Functional result | Uncached input / output tokens | Files | + / − text lines | Build / proof / test calls | Repairs |
|---|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| After | A1 | 541.6 | 20.0 | 21.3 | Pass (9/9); strict incomplete | 178032 / 18170 | 13 | 906 / 671 | 1 / 2 / 12 | audit below |

[After A1 evidence](evidence/I2/after/a-1/trial.json) and
[full log](evidence/I2/after/a-1/agent.log): the session read the first 240 lines
of the API README, game source, scene, tests and existing capture route. It said,
“I also found hard-coded ‘twelve’ UI copy and test fixtures/pins that must be
updated” (item_5). Two old-world fixture repairs included a failed typed child
read: “entity is absent or stale (generation check)” (item_18). The final change
replaces the old full-byte equality assertion with roster/presence checks;
that weakens coverage and must not be mistaken for preserving the old assertion.

The capture-route loop had three repairs: update its old twelve-wins assumption,
then fix routes stalled at `(-3.1399, 6.9012384)` and `(-4.8599186, 7.096641)`
(items 20–27). The completed capture test passed without the baseline's
“semantic state/hash differs” failure. Later, “the remaining failure is an
intentional byte-for-byte ‘difficult moment’ fixture mismatch” (item_29) led to
fixture regeneration, followed by updated tick 0/60/180 hash pins. The full suite
and Linux proof passed. A final workspace formatting check hit missing
Beacons/Greybox test modules caused by context trimming (item_41); the session
fixed its own formatting and checked only its touched Rust files. This is an
instrument/context cost, separate from the baseline A2's guessed missing
`lanterns/logic/tests/sim.rs` path. No per-command elapsed timestamps exist in
the CLI JSONL, so these quotes identify work and retries, not measured durations
for individual thought or repair phases.

A1 used typed `rows::<Lamp>()` in new tests; it did not invoke geometric layout
for route planning, placement helpers, or the `--paranoid` proof flag. It ran the
existing three-mode pin tests and the existing reload-report regression through
the full suite. That inherited coverage is distinct from deliberately using the
facilities to solve the change. Detailed cross-session recurrence and usage
matrices will be completed after the remaining five attempts.
