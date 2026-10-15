# Code review: LLP 1121 stage 3, commit 4, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `06a5448b65e0d7385cf0228519dd8e1c08ef626d6fecb778db98bd79b4a661ee`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, nine medium, three low; Grok: three medium, five low.
- **Disposition:** fixed in round 1 (no redirects for the token, writes split from hand edits, the file set asked again, a bounded bridge, one failure path, a long poll for buffered clients); exactStream's coalescing is LLP 1016 D4's, not changed

---

Reviewed `69a806ba1` read-only. I found **one high, nine medium, and three low** issues.

1. **High — Token discovery follows redirects across origins.** [scripts/tamber-server.mjs:221](/tmp/rv-s3c4/scripts/tamber-server.mjs:221)  
   If `op` targets a hostile server, that server can redirect token discovery to a real loopback Tamber endpoint. `op` then sends the returned credential to the original hostile origin. Use `redirect: 'error'` for discovery and authenticated requests, and verify the response origin before using its token.

2. **Medium — A hand edit can be attributed to a client write.** [scripts/tamber-server.mjs:65](/tmp/rv-s3c4/scripts/tamber-server.mjs:65)  
   Transaction replies contain every file’s digest. If `brand.tamber` changes by hand before a transaction edits `app.tamber`, `publish()` advances both baselines and announces both under that client. The watcher subsequently finds nothing different. The in-memory probe reproduced this. Return the actual written paths and their before/after digests; announce intervening disk changes separately. This matters for commit 5’s foreign-edit guard.

3. **Medium — Watch coverage is frozen at startup.** [scripts/tamber-server.mjs:107](/tmp/rv-s3c4/scripts/tamber-server.mjs:107)  
   Rescans only visit initial `hello` keys, so newly created or checked-out files never appear. A failed snapshot returning `files: null` leaves no watchers at all. The `.tamber` suffix filter also misses a differently named design allowed by `app.json`. Reconcile the file inventory with the Rust owner and watch actual design paths, including creation and recovery.

4. **Medium — An accepted request can permanently stall the stdio bridge.** [scripts/tamber-server.mjs:95](/tmp/rv-s3c4/scripts/tamber-server.mjs:95)  
   Adding the internal ID makes a valid 1,048,576-byte HTTP body exceed Rust’s line limit. Rust replies with `id: null`; the numeric waiter never resolves. For a transaction, `writing` also remains positive, suppressing subsequent watcher announcements. The probe produced a 1,048,583-byte line and a pending request. Bound the serialized line before enqueueing, and handle uncorrelated protocol errors and request deadlines.

5. **Medium — Child failure does not terminate the whole wire.** [scripts/tamber-server.mjs:87](/tmp/rv-s3c4/scripts/tamber-server.mjs:87)  
   There is no child `error` handler: missing Cargo produced an unhandled `ENOENT` and unresolved readiness. Ordinary exit resolves request waiters, but leaves existing subscribers open; new subscriptions still receive HTTP 200. Use one terminal failure path that settles readiness and requests, closes subscribers, and rejects subsequent subscriptions with 503.

6. **Medium — Oversized uploads are rejected only after EOF.** [scripts/tamber-server.mjs:128](/tmp/rv-s3c4/scripts/tamber-server.mjs:128)  
   Crossing 1 MiB merely sets `over`; a sender withholding EOF retains the request and buffered data until the HTTP timeout. The probe received no rejection until ending the body. Reject and cancel immediately on overflow, reject oversized declared lengths early, and handle aborts and body deadlines explicitly.

7. **Medium — HTTP clients share `hello` identity.** [scripts/tamber-server.mjs:164](/tmp/rv-s3c4/scripts/tamber-server.mjs:164)  
   All requests share one Rust process, whose `hello.client` becomes the fallback writer identity. After A’s hello and B’s hello, A’s transaction without an explicit client is attributed to B. Require `client` on every HTTP write, or implement separate client-session state instead of forwarding a shared identity.

8. **Medium — Standalone serving survives its launcher’s SIGKILL.** [scripts/tamber-server.mjs:207](/tmp/rv-s3c4/scripts/tamber-server.mjs:207)  
   The app CLI supplies `EXACT_LAUNCHER_PID`, but `serveAlone()` only handles signals. Killing the wrapper leaves the server and design lock alive. Both dev loops already use `watchLauncher`; apply the same ownership handling here and await writer shutdown.

9. **Medium — The intended JS data-module client cannot consume every event.** [scripts/tamber-server.mjs:145](/tmp/rv-s3c4/scripts/tamber-server.mjs:145)  
   Ordinary JS-target `fetch()` [buffers through EOF](/tmp/rv-s3c4/host/web-js/admission.js:121), so it never returns this open-ended response. Switching to `exactStream` alone is insufficient: its SSE reader [keeps only the last event in each chunk](/tmp/rv-s3c4/host/web/http-body.js:67). Both behaviors reproduced in memory. Provide and test a lossless JS-source streaming path or explicit event batches; Bun’s unrestricted fetch tests miss this integration gap.

10. **Medium — The dev-loop test hangs after startup failure.** [scripts/tamber-server.test.mjs:189](/tmp/rv-s3c4/scripts/tamber-server.test.mjs:189)  
    If the child exits before printing its URL, the startup promise rejects, then `finally` waits for a second `exit` event that will never occur. Check `exitCode`/`signalCode` before waiting, handle spawn errors, drain stderr, and bound shutdown.

11. **Low — Watcher assertions depend on fixed sleeps.** [scripts/tamber-server.test.mjs:143](/tmp/rv-s3c4/scripts/tamber-server.test.mjs:143)  
    The 400 ms pause does not prove the watcher processed the server’s write; the 300 ms pause before checkout does not prove the preceding hand edit was observed. Await the specific digest event and use controlled watcher completion for the negative assertion.

12. **Low — CLI subscription parsing assumes transport chunk boundaries.** [scripts/tamber-server.mjs:226](/tmp/rv-s3c4/scripts/tamber-server.mjs:226)  
    Splitting `data: ` across chunks leaves the SSE prefix in stdout. Subscription HTTP errors also return exit code zero. Both reproduced. Buffer complete events, as the test helper already does, and check `res.ok`.

13. **Low — Help advertises a `.tamber` operand that `serve` rejects.** [scripts/tamber-server.mjs:190](/tmp/rv-s3c4/scripts/tamber-server.mjs:190)  
    Only `.contract` operands become their parent directory; `serve apps/theme-fixture/app.tamber` searches beneath the file and reports no design. Support that operand or narrow the help text.

I found no additional problem with token logging/HTML exposure, timing-safe comparison, the direct access gate, or resident forwarding: `--serve-as` disables the resident Tamber mount. The non-async `main` correctly returns promises that its caller awaits. Changed source files remain below 1,500 lines; `host/web/dev.mjs` is 1,207.

Validation used read-only inspection and in-memory probes. No dev loop or repository test suite ran; socket binding was unavailable in the sandbox.
