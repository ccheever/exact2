# Quick re-check 2, LLP 1121 stage 10 A2, Astra

Reviewed: 9f26eaf7e7 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

- **SHOULD-FIX** — [scripts/tamber-scrub.mjs:146](/tmp/rv-rl-a2e/scripts/tamber-scrub.mjs:146): First preview completing past deadline triggers expiry through `opened()` → `send()`, clearing `open` before line 147. Reproduced HTTP 500 with the preview never restored.

- **SHOULD-FIX** — [scripts/tamber-server.mjs:309](/tmp/rv-rl-a2e/scripts/tamber-server.mjs:309), [903](/tmp/rv-rl-a2e/scripts/tamber-server.mjs:903): Presence and held-proposal reads still depend on timer cleanup. Reproduced expired polling-client pins/inspect/holds replaying on build registration, with proposals still unmarked.

- **SHOULD-FIX** — [scripts/tamber-pairing.mjs:279](/tmp/rv-rl-a2e/scripts/tamber-pairing.mjs:279): `onEnd()` checks membership, not deadline. Registration past expiry but before the timer gives the gateway no immediate callback.

- **SHOULD-FIX** — [scripts/tamber-server.mjs:202](/tmp/rv-rl-a2e/scripts/tamber-server.mjs:202): The SSE guard prevents the expired recipient’s delivery, but expiry synchronously re-enters `send()`. Reproduced a healthy subscriber receiving `revoked(seq=3)` before `builds(seq=2)`, breaking event ordering.
