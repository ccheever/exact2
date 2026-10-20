# Review round 2, LLP 1121 stage 10 A2, Astra (gpt-6-astra, max), security review

Reviewed: 166b775c43 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

**Round 2: changes required.** Most round-1 fixes hold, but revocation and expiry still have gaps.

- **BLOCKING — expiry re-check ignores the deadline.** [tamber-server.mjs:543](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:543) uses `pairs.has()`, which checks only map membership. If expiry passes before its timer runs, a body completing then still dispatches. Reproduced with the real clock: expired token, transaction dispatched, HTTP 200. Re-check expiry synchronously.

- **BLOCKING — scrub release can write after revoke.** [tamber-scrub.mjs:221](/tmp/rv-rl-a2b/scripts/tamber-scrub.mjs:221) clears `open`, awaits `released()`, then writes. Revocation’s `scrubs.absent()` cannot cancel this state. Reproduced revoke during that await: no immediate restoration, then a newly dispatched transaction and 200. This exceeds the documented exception for writes already dispatched.

- **BLOCKING — restored proposals lose the revoked mark.** [tamber-server.mjs:894](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:894): `accept` removes the proposal before waiting; revocation consequently misses it. A non-conflict failure restores the original, unmarked object. Reproduced accept → revoke → failure → held proposal without `revoked`. `restore()` needs the same check added to `hold()`.

- **BLOCKING — actual nonlocal requests can retain local authority.** [tamber-server.mjs:492](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:492) ignores `access.local`; it checks only `remote`/`tunnel`. Existing listeners pass `developmentGate`’s `{allowed:true, local:false}` without those flags. A LAN request carrying the local token therefore gets 200 from `clients`. Require affirmative local access, or supply the flags at every listener.

- **BLOCKING — revoking a slow subscription can crash the server.** [tamber-server.mjs:558](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:558) ends the response but leaves it registered until `close`; [broadcasting](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:198) continues writing meanwhile. Reproduced `ERR_STREAM_WRITE_AFTER_END` using Bun’s `ServerResponse`. Production installs no response error handler. Remove the subscriber when ending it.

- **SHOULD-FIX — request DoS bounds remain incomplete.** [tamber-server.mjs:372](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:372): a 200 KB deeply nested `hello` from a read client causes an uncaught `RangeError`. Separately, [pair admission](/tmp/rv-rl-a2b/scripts/tamber-server.mjs:630) buffers/parses before limiting; malformed JSON bypasses the limiter entirely. Bound request processing and contain handler failures.

- **SHOULD-FIX — tests overstate coverage.** [The revoke test](/tmp/rv-rl-a2b/scripts/tamber-pairing-server.test.mjs:101) checks subscription/poll termination and pin clearing, but never asserts scrub restoration; canvas coverage is only an `onEnd` callback. [Expiry coverage](/tmp/rv-rl-a2b/scripts/tamber-pairing-server.test.mjs:139) checks only authentication and that callback. The races above are absent. [The limiter test](/tmp/rv-rl-a2b/scripts/tamber-pairing.test.mjs:281) would also pass with an unbounded, refilling map.

What checks out: 50-bit CSPRNG codes, two-minute expiry, single use, five-failure burning, constant-time comparisons, scoped memory-only tokens, cookie refusal, attribution/namespacing, explicit remote/tunnel overrides, long-poll TDZ repair, timer re-arming, and secret-free generated telemetry. Ordinary revoke/expiry cleanup and revoked-client self-accept refusal passed my checks. C/B interfaces are documented.

Validation was read-only: four existing pure pairing cases plus in-memory wire reproductions with a stub compiler. The HTTP suite was inspected, not run because it creates files/builds. Working tree unchanged.
