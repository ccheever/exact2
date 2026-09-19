# Opt-in Lanterns change trials

This parent-owned harness is excluded from default checks. The immutable prompts,
all original task assertions, negative controls and 900-second agent ceiling are
retained. No agent trial is required to validate a merge.

`bun game/bench/trials/run-trial.mjs --ref REF --prepare-only a 1` validates an
immutable archive: app-owned workspaces and captured locks, production Linux bake,
Lanterns package tests, same-ref evaluator negative controls, real Landlock read
denial and cleanup. It does not copy credentials or dispatch an agent. Preparation
uses the parent shared Cargo target to avoid a second large build directory.
Without `--prepare-only`, the explicit command starts a timed trial after these
checks; its separate target, source and auth copy are removed afterward.

Archives prepare every consumer's metadata before removing other game and fixed
verification implementations. Candidate context contains Lanterns and its shared
fragments, engine dependencies, and metadata-only consumer stubs. Evaluators,
research, old trial records and inherited agent instructions are removed. Targets
and binary names come from the archived `resolveApp` and host Rust target. No warm
target tree is copied. Parent receipts live under
`~/lanes/gamenext/scratch/land-exec/trials/REF/TASK-ATTEMPT`.

Adapter version 4 reads G’s `CapsuleController` and baked `fox.model`; the
assertions and routes retain the historical methodology. The adapter uses ordinary keyboard input, exact tick driving and full world
save/load. It quantizes analog directions to eight keys. Linux world pixels remain
unsupported and cannot pass rendering assertions. The old direct crate route's
sign obstruction remains an expected negative control. Task A and B must fail on
the unchanged game. The adapter's process-restart save mechanism is retained for
methodology continuity; the combined runtime's HUD Save/Load now also works.

Historical cohorts, all outcomes and limitations, evaluator/prompt hashes, protocol
changes and original refs are preserved in
[the trial diary](../../diaries/006-historical-change-trials.md). Historical raw
receipts remain in the immutable incoming parent. Functional passes do not imply
strict acceptance or a measured speed improvement.
