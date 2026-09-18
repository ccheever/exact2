# Lanterns headless change trials

Parent-owned instrument, version 1. `createCommand({root, store?})` returns
`{command(request), reload(), tree(), screenshot(path), close(), unavailable,
limitations}`. Requests are BRIEF's ready/start/reset/input/step/state/pause/save/load.
`step` uses the selected ref's `s.world('world').ticks(N)`, verifies the exact
world tick delta, and leaves title/paused/terminal states stationary. State comes
from `state world:*`, world resources/publications and bounded logs. Missing data
is null, listed in reports. Feet are the physics capsule center minus 0.65m.

Linux cannot hold pointer contacts. Input vectors are explicitly quantized to the
nearest of eight ordinary keyboard directions. This is a contract deviation;
continuous analog input cannot be implemented on this carrier without a host
change, forbidden by I1. World pixels are unavailable; CPU screenshots show UI
and flat canvas. Cases requiring screenshots are unsupported, never pass, even
when their mechanics pass. Asset readiness means the world declares the Fox
asset and animation, not that a GPU uploaded it. Save/load uses the existing
world save carrier plus a sidecar for the UI's live title/pause binding, then
restarts the entire host and releases held input. Linux's UI save button remained
at “Saving…” in the probe, so it is not used to claim checkpoint persistence.

Baseline case bodies come from comparison-full/harness/acceptance.mjs. The
transport changes only: native driver keys, process reload, CPU screenshot
classification. The movement helper and case assertions are retained. Task
routes reuse the comparison's ordinary input route. No teleport or test mutation.

Run from the repository root (Bun and Cargo on PATH, development update trust):

```
bun game/bench/trials/build.mjs
bun game/bench/trials/acceptance.mjs
bun game/bench/trials/task-a.mjs
bun game/bench/trials/task-b.mjs
bun game/bench/trials/run-trial.mjs --ref next/baseline a 1
```

Evaluators accept `root=/archive` and `out=/evidence`. Reports distinguish failures
from unsupported checks and exit nonzero for either. `functionalPassed` on task
reports excludes only unavailable rendering. This is not a full comparison pass.
The baseline currently has a direct crate-route failure at the sign; it is
retained, not repaired by changing the game or evaluator's route.

The original request named next/trunk; the later common rule says commit on the
current branch. This clone is next/i1, so increments land there. No engine, host
or game changes are authorized. Parent construction used three adapter rounds:
(1) rejected pointer contact, (2) keyboard mapping and discovery that UI save
stays pending, (3) existing world-save carrier. Baseline records and both negative
controls are committed before any model attempt.
