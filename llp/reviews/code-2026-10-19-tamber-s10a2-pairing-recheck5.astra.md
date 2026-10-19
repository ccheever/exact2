# Quick re-check 5, LLP 1121 stage 10 A2, Astra

Reviewed: 682b722561 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

SHOULD-FIX — [scripts/tamber-scrub.mjs:117](/tmp/rv-rl-a2h/scripts/tamber-scrub.mjs:117): `stopped()` is called before initialization. A failed opening scrub with another update queued produces a `ReferenceError` and returns 500 instead of 409 (reproduced). Move its declaration above the guard. The post-await 401 fix passes; no other findings.
