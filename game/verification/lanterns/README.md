# Frozen Lanterns engine evidence

These are engine regressions against the twelve-lantern source/scene at the E1
boundary. They compile in `lanterns-logic`'s `difficult_moment` integration test,
using the same production engine, physics, typed kinds and restore code. Gameplay
changes use the live source and its separate tests. Do not repin this historical
record when adding an entity. The frozen source is test input, not a shipped game.

- `game.rs`, `scene.json`, and the two fragments reproduce the measured world;
  `scene.rs` retains the complete old Rust-construction vs authored-scene equality.
- `before.rs` and `fixtures/before-{0,60,180}.world` preserve complete historical
  world-byte equality after only the declared saved-binding/schema adjustment.
  No roster-only replacement or byte assertion was removed.
- `fixtures/difficult-moment.script.json` is ordinary input with queued actions.
  The two `.sim` files pin all checkpoint bytes, including the moving crate,
  airborne controller, spring, animation and pending input.
- `difficult-moment.json` pins the detailed inspection at that checkpoint;
  `difficult-moment.continuations.jsonl` pins all 605 continued observations under
  the unedited, placement, physics, clip and appearance builds. The existing
  full input/queue-byte, state, collision, timer, animation and reload assertions
  still run against the production engine in every paranoid mode.
- `binding-hash-changes.txt` is the historical 612-entry migration inventory. It
  records that engine change, not a list to maintain for future game edits.

The real game's `pins.json` protects its current tick 0/60/180 world hashes.
`bun game/proof.mjs lanterns --repin` requires complete current-game saves to
agree byte-for-byte in continuous, Save and FreshGame, and runs the unchanged
historical assertions and host proofs before publishing any candidate hashes.
This is the only current-game committed generated fixture after E1. Capture
bundles and host-proof artifacts remain derived outputs under `artifacts/`.
