# Semantics: an event does not first fire the work already due

**Status:** Closed
**Resolution:** Charlie approved 2026-10-07: Observe.step and CompSem.cstep deliver a handled event after the work due at c.now (dispatchAt), as every host's dispatch_at; observe_step_sound composes advance and dispatch; the difftest driver uses dispatch_at for a handled event. The 10 random and 4 mutation-queue corpus divergences agree; grouped-touch is a separate cause (20261007-js-grouped-list-rows-missing.md).
**Systems:** semantics (Observe.step, CompSem.cstep), semantics/difftest/src/observe.rs, runner (dispatch_at), host/web-js/rt.js
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-07

When an event arrives while a mutation's `then`, or a queue's next send, is already due, every host fires the due work first and then handles the event: `dispatch_at` on the runner, `on()` in `rt.js`. The Lean semantics (`Observe.step`, `dispatch`) and the difftest's runner driver (`observe.rs`, `r.dispatch`) deliver the event without it. So the JS target takes a different path from the runner driver, and the oracle is asked questions the runner never asked.

**Evidence.**
- The random JS difftest (500 cases) showed 10 divergences of this shape. With the driver switched to `dispatch_at` as an experiment, all 10 agreed (lane/web-followups, 2026-10-06; reverted).
- The async lane on the mini also reports 5 corpus divergences: `corpus/mutations/{queue-then,queue-two-sends,then-chain,then}.contract` and `contract/corpus/grouped-touch.contract`. These look like the same cause, but that is unconfirmed. A crude experiment that changed only the two event dispatches in `observe.rs` made 48 of 69 corpus cases diverge, so a faithful version must also advance the clock as `dispatch_at` does.

**Decision needed (Charlie).** Should an event advance to its time first in the semantics, as every host does? That changes `Observe.step` and `CompSem.cstep`, updates `step_sound` and `dont_go_wrong` (composing `advance` with `dispatch`), and switches `observe.rs` to `dispatch_at`. Recommended: yes.
