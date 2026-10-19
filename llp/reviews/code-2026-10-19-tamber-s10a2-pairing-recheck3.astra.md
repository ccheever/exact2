# Quick re-check 3, LLP 1121 stage 10 A2, Astra

Reviewed: 5b94421c16 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

- **SHOULD-FIX — [scripts/tamber-scrub.mjs:119](/tmp/rv-rl-a2f/scripts/tamber-scrub.mjs:119):** `show()` never checks `allowed()`. With expiry cleanup pending, queued scrub updates still start planning and preview after the deadline. Reproduced: both requests returned 200. Check the deadline before starting work and after awaits.

- **SHOULD-FIX — [scripts/tamber-server.mjs:954](/tmp/rv-rl-a2f/scripts/tamber-server.mjs:954), also [903](/tmp/rv-rl-a2f/scripts/tamber-server.mjs:903):** `hold()`/`restore()` derive `revoked` only from `ended.has()`. A proposal completing after its owner’s deadline broadcasts without `revoked: true` until cleanup runs. Reproduced on SSE; `held()` only corrects snapshots. Use the pure deadline check here too.
