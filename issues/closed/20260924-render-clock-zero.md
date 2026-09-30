# Server-rendered pages and checkpoints use time zero

**Status:** Closed
**Resolution:** The renderer captures wall time or accepts render_with_at timestamp and seeds it before slot initializers/resources; kernel/direct regression verifies nonzero initial state, now(), resource arguments and checkpoint without ticking timers.
**Systems:** Render server, Runner, Web adoption
**Severity:** P2
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1048.000 D5–D6

`host/render/src/lib.rs:93–116` boots a fresh runner, activates and settles
its resources, and exports its checkpoint without initializing the render
clock. A fresh runner starts at zero. Consequently every request render
evaluates `now()` at zero and embeds `now_ms: 0` into the checkpoint.

Reproduced with the actual renderer and a compiled Contract view containing:

```text
text `Clock ${now()}`
```

`render()` returned HTML containing `Clock 0` and a checkpoint with
`state.now_ms == 0`. LLP 1048.000 D5 explicitly calls for an initialized render
time. The implementation's earlier note deferring that work to adoption has
survived the checkpoint/adoption implementation.

Clock-dependent text and resource arguments therefore describe the Unix epoch
in the server document. Content may remain wrong until client activation and
can be read or indexed before then. Matching the zero-time checkpoint during
adoption does not make the server result correct.

Capture or inject the render time and seed it before initial evaluation and
resource queries. Preserve the policy that rendering does not run actions or
fire timers, and carry the same time into the checkpoint and canonical
projection. Verify with a supplied nonzero timestamp, including a resource
whose arguments depend on `now()`.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Probe:
`/tmp/exact2-review-20260924/game-probe/src/bin/render-review.rs`;
output: `/tmp/exact2-review-20260924/render-review.log`.

Integration verification (2026-09-30): the JS agent keeps its elapsed clock at
zero when adopting a document rendered on wall time; ordinary adoption retains
the rendered clock. The Caltrain web app smoke reproduced the inherited-clock
regression, then passed all three app tests and the sixty-timer seek after this
correction (`web smoke: ok in 4.1 s`).
