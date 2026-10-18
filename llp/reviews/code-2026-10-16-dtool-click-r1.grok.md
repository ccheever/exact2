# Code review: a click's token on selected and outline (LLP 1122 T2b), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a34b3b4ab`.
- **Method:** one brief (sha256 `24a73699da0d4278ebb5e9fa3e2b8c052f7607e8694b4a6379f0c9bfac4d1119`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken.
  - The page keeps the last 32 click tokens it made in an inspect session and no longer retires one when its outline arrives. An answer naming any of them leaves the page where it is: a refresh that re-sends a token while its first report is in flight, or both reports accepted. Abandoned tokens are bounded.
  - `click` is tamber/1.7 (the PROTOCOL constant, the tests, the version list, the `*1.7*` marks), and the `outline` shape lists `click?`.

---

The token paths hold. One protocol-version mistake does not.

**Medium — `docs/tamber-protocol.md:46` (rule at `:15`, `hello` still `"tamber/1.6"` at `:14` and `contract/cli/src/tamber_serve.rs:67`).** A new field was written into a minor that already landed. `tamber/1.6` is `HEAD~1`: `hello` reports it, and that commit added the `inspect` event and `inspect?` on `selected`. This commit adds `click?` on `selected` and on `outline` and rewrites the 1.6 bullet instead of bumping. The rule on the same page is that a minor version is how requests or fields are added, and a client cannot tell two wires apart except by `hello`'s `protocol`.

Scenario: a peer that implemented the landed 1.6 ignores unknown fields. It accepts a `selected` report that carries `click` and drops it, and it scrolls every `outline` because the frame's `click` means nothing to it. The new page does the opposite with the same version string. Nothing in `hello` distinguishes them.

Fix: ship `click` as `tamber/1.7` (`PROTOCOL`, the version list, and the `*1.7*` marks). Put `click?` on the `outline` shape at `:737` as well; that line still says `{sites, plan}`.

What I checked and did not find a defect in:

- **Refused click.** The token is added at `host/web/dev.js:491` before the report exists. A local `unsupported`, a 409 that does not ask again, or no source map (`scripts/tamber-server.mjs:206` outlines only when `selection.source` is set) never echoes it. A tool `outline` has no `click`, so it still scrolls. Leaving inspect mode clears the set at `:610`.
- **Stale and re-asked.** A 409 sends no outline, so the token stays outstanding. `refresh` re-sends `was.click` (`:518`). The outline that names it does not scroll, then the token is retired (`:642`).
- **Generation change.** `outlineNone` (`:433`) does not drop tokens. A frame whose plan digest does not match returns at `:635` before `delete`, so the token is still there for the re-asked outline. A later refresh, after that outline has already taken the token, scrolls. That matches "tokens it is waiting on."
- **Two quick clicks.** Each `pointerup` mints its own token into the set before either `await`. Neither outline scrolls.
- **Tool outline between them.** `channel.outline` from the tool passes no click (`scripts/tamber-server.mjs:548`). It scrolls and does not retire the click's token.
- **Reload.** The set is only in memory. `sessionStorage` restores inspect mode, not tokens, and register replays pin and inspect, not outline.
- **Queue.** `scripts/tamber-builds.mjs:165-167` collapses pin frames and inspect on/off versus refresh only. Outline frames are appended. A tool outline cannot replace a click outline, or the reverse, including when the queue is over its bound (`:172-174` only collapses `tamber-*`).
- **Relay.** `relayChannel` (`:454-456`) writes the polled frame through. `click` stays on the JSON. Apple and Linux ignore the extra field; they still answer `outline` as unsupported, which is unchanged.
- **Validation.** `scripts/tamber-builds.mjs:93`: absent is allowed, `""` is 400 (the new test), a non-string is 400, and 1 to 64 characters pass. The page's `${n}-${uint32}` token fits.

Verdict: Request changes — the click token, the queue, and the relay are right; the field has to be a new minor, not a rewrite of landed `tamber/1.6`.
