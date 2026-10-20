# Code review: LLP 1122 T1 commit 2 (the design tool's skeleton and its connection), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `3769f3880` (commit `0beef42fd` and its round-1 fixes).
- **Method:** one brief (sha256 `93314156363e53d503840b424dfc135aae643423847292d0a94d0fec146340ea`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 2 low).
- **Disposition:** all taken: the proxy reads a body inside its error boundary; a server already at the tool's port is reused only if it serves this checkout's build of the tool; the tool's build is accepted only whole (`builtAppMatches`, `jsTargetBuild`), as verify accepts one; a backpressure wait removes both its listeners.

---

I found **two Medium and two Low issues**. This was a read-only code review; I did not run builds or tests.

1. **Medium — Disconnecting during an upload escapes the proxy’s error handler.** [scripts/tamber-server.mjs:788](/tmp/rv-dt-2r2/scripts/tamber-server.mjs:788)  
   The request-body iterator runs outside `try`. A client that sends part of a POST body and disconnects makes that iterator reject. The rejection escapes the async request listener at line 655, becoming an unhandled rejection rather than a handled cancellation and potentially terminating the proxy.  
   **Fix:** include body reading in the error boundary, install cancellation handling before reading, and quietly finish when the downstream connection is gone. Add a partial-upload disconnect test.

2. **Medium — Reusing a server from another checkout can serve an absent or stale tool.** [scripts/tamber-server.mjs:729](/tmp/rv-dt-2r2/scripts/tamber-server.mjs:729)  
   Reuse checks only the canonical app directory. However, `buildTool()` builds this checkout’s `target/design-tool-web`, while the existing server serves its own checkout’s directory. For example, start `tamber serve <external-app> --port 7740` from checkout A, then run `tamber tool <external-app>` from checkout B. B builds successfully and prints the URL, but A can return “the design tool is not built,” or serve an older implementation.  
   **Fix:** verify the existing listener serves the tool build just validated. Otherwise refuse reuse with a clear diagnostic identifying the other checkout.

3. **Low — The cache check accepts an incomplete build.** [scripts/tamber-server.mjs:693](/tmp/rv-dt-2r2/scripts/tamber-server.mjs:693)  
   Matching the marker’s app ID and finding no changed sources does not establish that the built files exist or match the marker. A partial cache restore or cleanup that leaves `.exact-build.json` but removes `app.js` causes every subsequent `tamber tool` invocation to skip rebuilding and open a broken page.  
   **Fix:** use the existing `builtAppMatches()` and `jsTargetBuild()` checks before accepting the cache, as `verify.mjs` already does.

4. **Low — Backpressure handling accumulates listeners throughout a stream.** [scripts/tamber-server.mjs:801](/tmp/rv-dt-2r2/scripts/tamber-server.mjs:801)  
   Each backpressure wait installs both `drain` and `close` listeners. When `drain` resolves it, the `close` listener remains attached. A long-lived subscription repeatedly encountering backpressure therefore accumulates callbacks until disconnect, causing listener warnings and growing memory use.  
   **Fix:** whichever event resolves the wait should remove both listeners.

The round-1 fixes otherwise address the reported production paths: literal-loopback upstreams, redirect refusal, query stripping, bounded bodies, canonical app comparison, checkout-independent tool resolution, startup token-request sharing, first-poll backlog handling, history clearing on `missed`/epoch changes, digest refresh, and the overlapping-poll guard. Persistent server failure now returns 503; an already-held poll can deliver the terminal event once before subsequent requests back off. The static-serving extraction retains its locality and path guards.

Regression coverage remains partial. The tests now assert digest updates and history clearing, and the glue test selects the third reply. However, it does not assert that reply has `settled: true`; the stub restart keeps the same token; and automatic failure retry plus proxy disconnect/backpressure behavior remain untested. The canned CORS token endpoint remains as explicitly recorded in the disposition, with stub mutations now requiring the driver’s bearer.

Verdict: **Request changes.**
