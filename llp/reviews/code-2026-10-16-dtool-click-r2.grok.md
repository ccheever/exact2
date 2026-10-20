# Code review: a click's token on selected and outline (LLP 1122 T2b), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `5ef446fa5`.
- **Method:** one brief (sha256 `be9785f1c1d56ebf27106d179828f0b080ce36727f8a89d490918375e5c91f57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve.
- **Disposition:** taken. The page no longer keeps a capped set of tokens. A token is its inspect session (a random prefix, new each time inspect mode starts) and a counter, and an outline naming any click of the session stays put, however many reports are on their way. Leaving inspect mode ends the session.

---

Round 1’s two fixes hold: the page keeps the last 32 click tokens and does not retire one when its outline arrives, and `click` is `tamber/1.7` on both frames. No new defect in the token lifetime, the queue, the relay, or the version.

What I checked:

- **Refused click.** The token is inserted in `host/web/dev.js:492` before the report exists. A local `unsupported`, a 409 that does not ask again (`scripts/tamber-builds.mjs:285`), or a join with no source (`scripts/tamber-server.mjs:206`) never echoes it. A tool `outline` has no `click`, so it still scrolls. The unused token only occupies one of the 32 slots.
- **Stale and re-asked.** A 409 sends no outline. `refresh` re-sends `was.click` (`host/web/dev.js:519`). If a newer click has already replaced `inspecting.selected`, the re-ask carries that newer token. Both outlines name a token still in the set, so neither scrolls. That is the round-1 race, and not retiring the token closes it.
- **Generation change.** `outlineNone` (`host/web/dev.js:433`) does not drop tokens. A frame whose plan digest does not match returns at `:636` before the scroll. The re-asked outline names the same token and stays put, which is what this commit says an answer to either report does.
- **Two quick clicks.** Each `pointerup` mints its own token into the set before its `await`. Neither outline scrolls.
- **Tool outline between them.** `outline` from the tool passes no click (`scripts/tamber-server.mjs:548`). It scrolls and does not remove either token. A `clear` no longer wipes the set, so a selection outline still queued behind it does not scroll.
- **Leaving inspect, and a reload.** `setInspect(false)` clears the set at `host/web/dev.js:611`. A later outline hits `if (!inspecting.on) return` at `:634`. The set is only in memory. `sessionStorage` restores inspect mode, not tokens, and register replays pin and inspect, not outline (`scripts/tamber-builds.mjs:235-241`).
- **Queue.** `scripts/tamber-builds.mjs:165-174` collapses pin frames, and inspect on/off versus refresh, only. Outline frames are appended. A tool outline cannot replace a click outline, or the reverse, including when the queue is over its bound (`:172-174` only collapses `tamber-*`).
- **Validation.** `scripts/tamber-builds.mjs:93`: absent is allowed, `""` is 400 (the new test), a non-string is 400, and 1 to 64 characters pass. The page’s `${n}-${uint32}` token fits. The server copies that string onto the outline and omits it when it is null.
- **Relay.** `relayChannel` (`scripts/tamber-builds.mjs:454-456`) writes the polled frame through, and `JSON.stringify` keeps `click`. Linux parses `outline` as reserved (`host/linux/src/build_channel.rs:413`) and Apple answers any non-pin kind `unsupported` (`BuildChannel.swift:142-143`). Both ignore the extra field.
- **Version.** `tamber/1.6` stays the inspect event. `click?` is marked `*1.7*` on `selected` (`docs/tamber-protocol.md:710-712`) and on `outline` (`:738-744`). `hello` reports `tamber/1.7`.

Verdict: Approve — the click token, the queue, the relay, and `tamber/1.7` are right, and the round-1 fixes do not reopen the scroll partials.
