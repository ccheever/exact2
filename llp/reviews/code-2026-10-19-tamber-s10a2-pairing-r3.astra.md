# Review round 3, LLP 1121 stage 10 A2, Astra (gpt-6-astra, max), security review

Reviewed: 0dd5c28d8d (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

Two **BLOCKING** issues remain.

- **BLOCKING — expiry checks still depend on the timer.** [tamber-server.mjs:290](/tmp/rv-rl-a2c/scripts/tamber-server.mjs:290), [611](/tmp/rv-rl-a2c/scripts/tamber-server.mjs:611): the scrub-release guard and resumed long poll check `ended`, not `pairs.live()`. Reproduced with the injected clock: expiry during `released()` still dispatched a transaction and returned **200**; a waiting poll awakened after its deadline also returned **200**. Recheck the deadline before these actions.

- **BLOCKING — nested review data poisons subsequent reads.** [tamber-server.mjs:967](/tmp/rv-rl-a2c/scripts/tamber-server.mjs:967), [623](/tmp/rv-rl-a2c/scripts/tamber-server.mjs:623): a propose client can submit `hold:true`, `ops:[]`, and 100,000 nested arrays inside `review.extra`—about 200 KB. The proposal is stored before serialization throws. The new catch returns **400**, but leaves the poisoned proposal/event stored. Reproduced subsequent requests from a separate **read** client: both `/events` and `/subscribe` throw uncaught `RangeError`. Reject excessive nesting before mutation and contain serialization failures across stream handlers.

- **SHOULD-FIX — reserved client names remain usable on streams.** [tamber-server.mjs:551](/tmp/rv-rl-a2c/scripts/tamber-server.mjs:551): the `method === 'revoke'` exception also applies to `/subscribe` and `/events`. A local request carrying `{method:"revoke", client:"remote:…#1"}` is accepted under that paired identity. Restrict the exception to the actual revoke RPC.

The other round 2 fixes check out: mid-body deadline enforcement, explicit revocation during scrub release, restored proposals retaining `revoked`, LAN/local-token rejection, removal of ended subscriptions before later broadcasts, admission before body reading, and late `onEnd` reasons. Forwarded deep requests and ordinary handler exceptions are contained; stored review data is the remaining exception.

Also sound: 50-bit CSPRNG codes, two-minute/single-use/five-failure behavior, per-peer limiting, constant-time code comparisons, memory-only hashed tokens, default 12-hour TTL, header-only authentication, paired attribution, local-only administration, and revoke cleanup including the gateway hook. No credential leaks found.

Verification used read-only, in-memory Bun probes. The HTTP suites create fixtures/build artifacts, so I did not run them. Existing tests miss both blocking cases; notably, the release test substitutes a boolean `allowed` callback rather than exercising the server’s expiry guard.
