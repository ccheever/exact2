# Code review: the exact-js overlay cut, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `12f1d22a89`.
- **Method:** one brief (sha256 `89ace84681d5a306a202d0666e14765209b4968af0c66f9f79483ac9e5e83006`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The overlay cut holds. Round-1 is fixed and covered by the tests, and the new cases do not add a blocker or a major defect.

The probe in `js/src/prelude.js` reads property descriptors and does not call an overlay getter. An accessor, a throw, or a non-object counts as an overlay, so `__exact_overlay` still performs the old read. `overlaying` is set before that walk, so a Proxy `getOwnPropertyDescriptor` or `getPrototypeOf` trap runs inside the overlay guard. The call is not a turn; that is stated at `js/src/lib.rs`. On wasm, only `{has: false}` means no overlay. Anything else, including an error or a malformed reply, fails as unavailable. A worker realm still answers yes and keeps the `{tag: 4, worker}` note.

The tests match that. A generation loaded with an overlay returns the 300 rows plus `"overlaid"` and encodes once. The next generation without an overlay encodes nothing. Arming and disarming at runtime does the same, including the overlaid value. A getter is read 4 times across 2 overlays, and both store writes are refused inside the guard. The wasm probe answers are tested. The JS web target is unchanged: `host/web-js/ts-data.js` still decides while the module is evaluated, and a dev-loop reload is a new realm.

A frozen, sealed, or null-prototype `exact` is read by the same walk. `getOwnPropertyDescriptor` does not care that the object is non-extensible, and a null prototype ends the walk. If `exact` itself is an accessor on `globalThis`, the descriptor has no `value`, so the probe returns "has overlay" without calling the getter. The old path still reads it. The cost is one short call and a two-step descriptor walk per overlay, including a `found` closure allocated each time. That is the call that replaces encoding the answer when there is no overlay.

VERDICT: APPROVE

1. MINOR — `js/src/prelude.js:1403` — A Proxy whose `get` trap supplies `overlay`, while `getOwnPropertyDescriptor` reports no such property, is treated as no overlay, so the overlay never runs. The traps that do run are inside the guard (`overlaying = true` at line 1399). Descriptors are the source of truth, which is what keeps the probe from calling a getter. The comment that anything unusual is read exactly as before does not hold for this Proxy. Say so next to the probe.

2. NIT — `js/tests/it/overlay.rs:209` — The getter test locks the read count and the refusals inside `__exact_overlay`. It still passes if the probe's own `overlaying = true` is removed, because the probe never calls that getter. Nothing asserts that a Proxy trap's `store.set` during the probe is refused.
