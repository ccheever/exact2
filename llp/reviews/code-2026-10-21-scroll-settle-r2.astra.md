# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9deb6cfc1b`.
- **Method:** one brief per round (sha256 `d73970a16846d29ce2497e5221d3a247d1754c75cee69118664bd48c86630a2b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 3 SHOULD-FIX (descendant events through capture; round 16 settled with work; conformance ignoring the reply), 1 NIT (the diagnostic)
- **Disposition:** all taken in a2eff5013d

- **SHOULD-FIX — [host/web/collection-glue.js:467](/tmp/iosfix-rv-ss2/host/web/collection-glue.js:467):** The capture acknowledgement also clears debt for **descendant** scroll events. Capture listeners receive descendant events even without bubbling ([DOM Standard](https://dom.spec.whatwg.org/#concept-event-dispatch)). If the port’s handler jumps from 300 to 400, then a queued descendant scroll fires, that descendant clears the still-owed 400 event. Filter with `event.target === s.ackTarget`. This also affects the document listener.

- **SHOULD-FIX — [host/web-js/agent.js:387](/tmp/iosfix-rv-ss2/host/web-js/agent.js:387):** On the sixteenth round, `fired: true, owed: 0` takes `continue` out of the loop. The final reply then reports `settled: true` whenever requests are idle, even if that event’s handler started an animation or armed work needing another round. An in-memory probe returned `{clock: 0, settled: true}` with an animation pending at 1000 and no advance performed. Exhaustion needs an unsettled reply unless a quiet round was verified.

- **SHOULD-FIX — [host/web-js/conform.mjs:501](/tmp/iosfix-rv-ss2/host/web-js/conform.mjs:501):** Conformance discards both settle replies. The new `settled: false, reason: 'scroll'` therefore still proceeds into comparisons while reader handlers remain owed. Reject that result before comparing; otherwise reaching the bound can still produce the original race or a misleading pass.

- **NIT — [scripts/agent.mjs:1232](/tmp/iosfix-rv-ss2/scripts/agent.mjs:1232):** The existing `'scroll'` diagnostic claims a four-second wait for a finger’s fling/bounce. Web replies now use it for pending programmatic events with a frame-based bound. Make the diagnostic distinguish these cases.

No BLOCKER found. The wasm guard covers its successful-return path. No files changed; the probes used mocked DOM/control flow, not a browser run.
