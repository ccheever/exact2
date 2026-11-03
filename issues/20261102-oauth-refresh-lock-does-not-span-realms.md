# OAuth refresh serialization does not span browser tabs or module realms

**Status:** Closed
**Severity:** P2
**Systems:** packages/oauth, web, session storage
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `packages/oauth/index.ts:218,261-267` serializes token work through a module-local Map. Separate realms have separate maps, although browser tabs can load the same kept OAuth session. The web Store persists secrets in localStorage (`host/web-js/rt.js:382-393`). Two tabs can therefore redeem the same rotating refresh token concurrently.

Providers with reuse detection invalidate the token family when the second request spends the old token. [Auth0 documents that behavior](https://auth0.com/docs/secure/tokens/refresh-tokens/refresh-token-rotation#automatic-reuse-detection); [RFC 9700 §4.14.2](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14.2) explains why replay ends the active refresh token.

`bun /tmp/exact-review-oauth-realms.mjs` imported the actual package under two distinct module identities, shared one synchronous store, and used the repository's Auth0 fixture. After sign-in, it expired the access token, held two refresh requests, then released both. The fixture counted refresh=2 and reuse=1; the reads returned 401 and invalid_grant. The next read returned invalid_grant and the session ended. This proves that independent package instances cannot serialize shared-session refreshes. It was a local protocol reproduction, not a full two-tab app drive or a live provider test.

Coordinate token work across all realms that share the session, with a host-supported lock and fresh store reads under that lock. Cover two independent realms spending one rotating session.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

**Resolution.** Expose optional store-owned exclusion to OAuth. Browser secret stores use a Web Lock for the shared localStorage key, reload inside that lock, and persist before releasing it. Code exchange, refresh and sign-out use the same exclusion; persistence failures fail closed.

**Verification after the fix.** All 85 OAuth Bun tests passed, including independently loaded OAuth modules with separate cached store instances and a rotating refresh token. Store tests passed fresh reload and failed persistence cases. A real Chromium two-tab probe performed one refresh and both tabs received the fresh token.


**Browser follow-up.** The wasm target now exposes store-owned exclusion in the shared prelude. The page acquires the same Web Lock for iframe and worker realms, refreshes the call’s store snapshot under it, and persists protected writes before delivering the result to the runner and releasing ownership. A waiter leaves the realm’s turn queue so the holder’s HTTP continuation can proceed. Forgetting a queued call, retiring a holder and disposing a realm release their ownership. A failed persistence removes the stale token instead of leaving it available for reuse.

**Browser verification.** The actual OAuth package passed independent iframe/iframe, iframe/worker and worker/worker regressions, plus overlapping calls within one iframe or worker. Lifetime regressions cover cancellation, grant refusal and failed persistence. A real Chromium probe shared one rotating session across three tabs: wasm iframe, wasm worker and the JS-target store helper. One refresh occurred and all three read the fresh token under the same Web Lock.

**Native resolution.** Sessions sharing a credential backend and secret name share an OS lease and durable generation fence. A holder reads the backend afresh, acknowledges protected writes before release, and prevents older ordinary retries from overwriting them. Main and worker modules retain sent refresh replies through their completion, including ordered worker groups, with the original request authority. Lock waiters and worker delivery stay off the host's ordered request lane. Abandoned or uncertain effects invalidate the credential before another holder can reuse it.


**Persistence and cancellation follow-up.** Failed browser writes no longer survive a protected update, and ordinary retries are discarded when the durable value has changed. Protected browser module writes cross the runner as acknowledged writes, so its page queue cannot replay them. An interrupted update leaves a durable marker; the next holder invalidates the uncertain credential. On the wasm module path, abandoning a sent refresh therefore requires sign-in again instead of replaying its token. The JS target retains its in-flight callback. Focused browser store and module regressions passed, including abandonment and a successor opening after an interrupted update; all 85 OAuth package tests passed again.


**Native verification.** The native JS lane passed 268 tests, including real OAuth code across main and worker modules, ordered groups, retained replies, failed persistence, cancellation and two read-only resources settling on absent secrets. Data and data-host suites passed 142 tests; retained-reply and Store checks passed 19. Host coordinator regressions passed, including stale retries and shared backend scope. Source budgets, formatting and the quick semantics comparison passed in the implementation lanes; the final integrated gate results are recorded with the delivered commit.

**Platform limit.** Production Windows uses coordinated private memory secrets. Named Windows agent scratch stores refuse protected file writes because acknowledged directory durability is not implemented there. Windows-specific tests were added but were not executed on this Mac. An abandoned wasm browser module refresh likewise requires sign-in again, as described above.
