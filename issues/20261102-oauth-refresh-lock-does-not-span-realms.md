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
