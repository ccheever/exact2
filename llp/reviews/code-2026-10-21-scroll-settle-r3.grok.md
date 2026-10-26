# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 3, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `614159b9e1`.
- **Method:** one brief per round (sha256 `5fb781cf96130bdc6d4d06b2d310ff8592151dc36094b2a14f131f46dde5b0b6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 SHOULD-FIX (the settle failure's step and pages), 1 NIT
- **Disposition:** taken in 7b3cb2e06f

**SHOULD-FIX** — A settle that still owes a scroll is recorded against the previous step, and in a cross-browser run against the wrong engine. `host/web-js/conform.mjs:502`

`fail(driveAt, …)` runs inside `settle`, but `driveAt` is set to `boot settle` only for the boot call. `compare` then leaves it on that step's screenshot (`boot screenshot`, then each scripted line's screenshot). The `settle()` after `tap say`, `wheel`, or an automatic tap therefore files the failure on the previous step. The same call always labels `replies[0]`/`replies[1]` as wasm and js. With `--browser`, those sessions are Chrome and Firefox or WebKit (`reference` / `other` are already computed for that). The failure still counts, but the report names the wrong step and, cross-browser, the wrong page.

**NIT** — The `scroll-event` diagnostic says the browser never fired the event. `scripts/agent.mjs:1232`

The JS host also returns `scroll-event` when round 15 did run the reader's handler and there was no quiet round after (`lastScroll === 15` with nothing still owed). The text says the event has not fired, and it ends with "`clock settle` again waits again." Retrying settle is the right recovery either way. The wasm host only uses this reason while a scroll is still owed, so the sentence matches that path.

The ack, the 16-round and 8-frame bounds, and the device-before-scroll order do not spin or leave a mark set once the port's own scroll event has been delivered. The browser test covers a reader registered first, a jump from that handler, and a bubble-phase ack. It does not cover a viewport port (ack on `document`) or a descendant scroll that must not clear the mark.
