# Independent agent change trials — protocol fixed before baseline completion

Run on frozen, accepted playable games, not the earlier microfixtures.
Each attempt starts in a fresh copy of the same engine baseline with its
dependencies already installed. A fresh Codex session receives only its game,
the common game brief and one change request. Model: `gpt-5.6-sol`, reasoning
high. Same 15-minute wall ceiling for every attempt; at most three fixes per
failure loop. No access to other attempts or the external evaluator. Engine
implementation authors are not the trial sessions. The common-model direct
source workflow is tested; vendor-hosted agent products are not substituted.

Two tasks, two independent attempts per task per engine: 16 attempts total.
Do not count reruns of the evaluator as new agent attempts. Tasks are dispatched
in alternating order across engines; order and exact times recorded. Failed or
timed-out attempts stay in the denominator. Setup/import/export work remains
separate from the subsequent warmed change task. The evaluator is parent-owned
and trial sessions cannot change it.

## Task A: another reachable lantern

Add lantern-13 on a new raised platform at [-8,1.2,7]. The platform's center is
[-8,.6,7], size [3,1.2,3]. It must be reachable by jumping from the ground and
collide normally, have the same appearance/interaction rules as the other
lanterns, and be required for victory. Keep the original twelve and their
positions. Update the displayed total and save/load behavior. Prove the new
platform can be reached through ordinary movement and jump input, the lantern
cannot be lit remotely, and twelve no longer wins. No teleport/debug shortcuts.

## Task B: return home to win

Keep twelve lanterns. Lighting all twelve no longer immediately wins: the fox
must return within2 horizontal units of the original spawn to finish before
night. Show an explicit return-home instruction once all twelve are lit. Keep
the timer running until the return; timing out during the return loses. Save
and reload must preserve this intermediate state and its timer. Do not change
level geometry, movement, physics, or the existing inspection command names.

## Scoring and timing

The parent runs an immutable independent task-specific evaluator after the
agent finishes; no further repair after seeing external results in that
attempt. A complete pass requires baseline mechanics outside the intended
change plus the new positive and negative cases, reload persistence and actual
rendered output. Report failures by requirement. If the evaluator itself has a
bug, fix it globally, version it, and rerun all affected candidates; never
selectively relax a failing engine's requirements.

Record prompt dispatch, model completion, build completion, evaluator start
and final result times. Prompt-to-proof includes model/build/evaluator work;
external queue delay is separately reported. Record model token usage where
the CLI exposes it, human interventions, screenshot inspections, generated
screenshots, self-repair rounds, and final source hashes. Screenshots generated
without model inspection do not count as pixel dependence. If the CLI evidence
cannot distinguish an observation, mark it unknown rather than zero.

These are small-sample, within-task measurements, not universal agent success
rates. The shared task list reduces variance while the two replicates expose
some stochasticity; two tasks cannot characterize an engine's whole workload.

## Execution deviations and audit

Only Babylon and PlayCanvas qualified, so eight attempts ran and eight planned
attempts (Godot/Three) did not. Actual order was A1,B1,B2,A2 per engine in
parallel, not randomized engine order. Every submitted game passed the external
evaluator, but PlayCanvas B1 exceeded the three-repair cap: five repairs in one
crate/ledge proof loop, then a separate home-walker repair. Thus eight functional
passes comprise seven compliant successes and one budget-compliance failure.
The 15-minute wall ceiling was mechanically enforced; repair caps relied on
agent compliance and the retained-log audit. No replacement attempt was added.
Readiness and crate-equivalence evaluator corrections were global and reran
unchanged submissions; see harness/EVALUATOR.md.
