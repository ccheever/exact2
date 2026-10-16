# Code review: stage 4 commit 5: the build channel, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change commit and the first-round commit.
- **Method:** one brief (sha256 `7ab1f729495ada05cf721d239a5ba5a0f81fee3170afdd91ce7a212fe1557230`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (the build cap bypassed through one nonce; an aborted report body ending the loop; oversize relayed batches; streams not expiring on their own; duplicated first-visit tabs; reports crossing registrations; forwarding after a disconnect; stale design digests; JS status after live edits; Linux losing a failed snapshot request; the protocol doc).
- **Disposition:** applied in the second-round commit.

---

I found blocking issues in the two-commit change.

1. **P1 — The build cap is bypassable with one nonce.** [tamber-builds.mjs:115](/tmp/rv-s4-7/scripts/tamber-builds.mjs:115)  
   Re-registering one stream under different IDs leaves every previous record pointing at its still-open nonce. The eviction loop considers them connected and removes none. A read-only probe registered **1,000 builds on one nonce**. An unauthenticated LAN peer can grow this table and its emitted build lists without bound.

2. **P1 — Aborting a report request can terminate the dev loop.** [dev.mjs:1153](/tmp/rv-s4-7/host/web/dev.mjs:1153), [JS dev.mjs:258](/tmp/rv-s4-7/host/web-js/dev.mjs:258)  
   Both body-reading loops let request-stream errors escape their async HTTP handlers. An aborted body produced an unhandled rejection and exited Bun 1.4.2 with status 1 in a probe. This happens before nonce validation. Catch aborted/erroring bodies.

3. **P2 — Relayed JS batches exceed the native transport limits.** [JS dev.mjs:145](/tmp/rv-s4-7/host/web-js/dev.mjs:145)  
   The JS loop broadcasts complete batches without the resident loop’s 32 KiB notice fallback. Apple caps SSE input at 64 KiB; Linux caps the entire commands response at 64 KiB. A probe produced an **85,944-byte response**, then an empty next poll: Linux can permanently lose those drained frames, while Apple disconnects. Bound frames and aggregate poll responses.

4. **P2 — Polled streams do not expire autonomously.** [tamber-builds.mjs:61](/tmp/rv-s4-7/scripts/tamber-builds.mjs:61)  
   `sweep()` runs only through channel activity; neither `list()` nor the server’s presence timer invokes it. When the last Linux device stops polling, it can remain listed indefinitely, contrary to the five-second ruling. The expiry test masks this by calling `report()` before checking the list.

5. **P2 — First-visit tabs still share IDs when duplicated.** [dev.js:312](/tmp/rv-s4-7/host/web/dev.js:312), [JS dev.mjs:232](/tmp/rv-s4-7/host/web-js/dev.mjs:232)  
   The BroadcastChannel responder is installed only when storage already contains an ID. A first-visit tab generates its ID without listening; its duplicate receives no answer and retains that ID. Reproduced in a probe. Registration then displaces the original tab, whose reports fail and whose existing pin may never be cleared.

6. **P2 — Reports can cross registration lifetimes.** [dev.js:325](/tmp/rv-s4-7/host/web/dev.js:325), [JS dev.mjs:232](/tmp/rv-s4-7/host/web-js/dev.mjs:232)  
   A report awaits the old registration promise, then `post()` reads the current nonce. Reconnecting during that wait sends old report data under the replacement nonce—even before its registration completes. Reproduced with the replacement registration still pending. Capture and recheck the nonce and registration together.

7. **P2 — Forwarding can survive a client disconnect during resident startup.** [JS dev.mjs:367](/tmp/rv-s4-7/host/web-js/dev.mjs:367)  
   The disconnect listener is attached only after `residentLoop()` resolves. If `/__dev` closes while startup is pending, the callback subsequently creates an upstream stream without noticing that closure. A probe created that upstream without destroying it. Install cancellation handling before awaiting startup and check whether the response has closed.

8. **P2 — Apple and wasm report stale design digests after full generations.** [LiveDesign.swift:32](/tmp/rv-s4-7/host/apple/Sources/ExactKit/LiveDesign.swift:32), [dev.js:231](/tmp/rv-s4-7/host/web/dev.js:231)  
   Their cached digests update through live-table responses, but a full generation normally makes `/__dev/design` return 204. The old digest therefore survives the new plan. Apple also initially reports `null` until a live frame supplies a digest. Read the running design after each boot rather than relying on snapshot traffic.

9. **P2 — JS live edits never report their new status.** [JS dev.mjs:231](/tmp/rv-s4-7/host/web-js/dev.mjs:231)  
   `live()` applies batches and recovery snapshots without calling `status()`. Updating `exact.designDigest` alone emits nothing. A probe changed the running digest and observed **zero reports**, leaving the design tool’s build status stale until another reporting event.

10. **P2 — Linux loses a failed snapshot request.** [build_channel.rs:110](/tmp/rv-s4-7/host/linux/src/build_channel.rs:110), [fetch.rs:160](/tmp/rv-s4-7/host/linux/src/fetch.rs:160)  
    `take_design()` clears the request before fetching. A transport failure or 5xx does not restore it. With a healthy commands poll and no further edits, the host never retries and remains on the wrong design. Clear the request only after a successful snapshot/204 response.

11. **P3 — The protocol reference still describes plan structure on the wire.** [tamber-protocol.md:436](/tmp/rv-s4-7/docs/tamber-protocol.md:436)  
    The snapshot acceptance description should name the design’s shape, matching HEAD’s ruling and `design::shape`.

The nonce entropy, production guards, explicit pin/null replay, command dispatch, protocol version and changed-file line caps look correct. In-memory ownership checks passed for long-poll presence and clearing only after the last subscription closes.

Verification remained read-only: two supplied channel tests passed, supplemented by the probes above. The relay test could not bind a socket in this sandbox; native builds and tests were not run.
