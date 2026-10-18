# Code review: stage 4 commit 6: uses, resolve and scrub sessions, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change commit.
- **Method:** one brief (sha256 `354c2c9bb40f661dca87e3052f6f426d1d3a2e99c28d3b64bfbde13f41beeead`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a hand edit during a preview leaving the opening design; the first preview not a scrub; release before the write; `uses` counting style strings that read no token; idle; other design files; the overlay on unwind).
- **Disposition:** applied in the first-round commit, with theme2's conditions.

---

No P0. Two P1s break the scrub invariant that every build ends on the latest committed design.

[P1] A hand edit during an in-flight preview leaves builds on the design the scrub opened on — `scripts/tamber-scrub.mjs:41` and `host/web/dev.mjs:468`

`end` fires `restore()` and does not wait. On the wasm loop the compiler finishes the scrub command, the restore, and the poll of the already-saved file in one iteration, then flushes one stdout chunk (`host/web/src/dev.rs:418`). The stdout reader pushes the `design` line (preview to the new file) immediately, but a `scrubbed` reply only resolves a promise (`host/web/dev.mjs:471`), so the preview push and the restore push run as microtasks afterward. Restore was computed against `self.design` from before the poll, so the last frame applied is the opening design. `/__dev/design` is left on that snapshot too, and a build that connects or answers `stale` loads it. The file on disk is the hand edit.

[P1] The first preview is not a scrub yet, so a competing write does not cancel it — `scripts/tamber-scrub.mjs:34`

`open` is set only after `preview` returns (`scripts/tamber-scrub.mjs:65`). Until then `compete`, `absent`, and `lost` return without incrementing `ends`. A hand edit or transaction in that window (the opening `propose` plus the compile) changes the file, then the preview lands, `show` opens the scrub, and nothing calls `restore`. Builds show the gesture; the file is the other write. The same window covers a disconnect or the design server dying: the preview still opens, and server loss does not restore until the 30s timer.

[P2] Release drops the session before the write, and does not count as an end — `scripts/tamber-scrub.mjs:111`

`open = null` happens before `await write`, and `ends` is not incremented. `write` calls `compete` first, which is then a no-op. Another `scrub` is accepted and previewed for the whole transaction. A conflict then `restore()`s that new preview away. A successful write makes `Session::build` take `self.scrub` (`host/web/src/dev.rs:242`) and batch from the new preview back to the released file, while the wire still says the new scrub is open.

[P2] `uses` counts style strings that do not read the token — `contract/cli/src/tamber_uses.rs:91`

Every `BindingKind::Style` is a site when any string its code pushes contains the name (`names`, whole-word) or contains a synthetic expression (`s.contains(e)` at line 96). The kernel only resolves a token when the whole value is that name, or a colour-function expression. A fixed row does too: every `button` pushes `text-align: center` (`contract/lower/src/tags.rs:122`), so a token named `center` is a site on every button. `font-family="brand sans"` is a site for `brand`. `oklab(...)`.contains(`lab(...)`) is true, so `via` can name the wrong synthetic. The test only checks a `text` node, which is not a style binding.

[P2] The idle timer runs during the next preview — `scripts/tamber-scrub.mjs:45`

The 30s timeout is reset only after a preview finishes (line 67), not when the update is accepted. An update that starts just before the deadline still fires `idle` while that preview is in flight, and the gesture is taken back.

[P2] A scrub of any design file other than the app's own previews nothing — `host/web/dev.mjs:296`

`scrubPreview` (and `host/web-js/dev.mjs:270`) returns success when `texts` does not include the primary design path. `propose` still returns the other file's text, the scrub opens, and release writes it. The running builds never show the gesture.

[P3] A panic during a scrub compile leaves the overlay in place — `contract/cli/src/sources.rs:158`

`with_design_text` restores the thread-local only after `f` returns. There is no drop guard. On the resident compiler that is the normal-build thread; an unwind skips the restore and the next `poll` reads the scrub text as the design file. The JS loop is a short-lived process and does not have this leak. Its scratch file is `app.target/web-js-scrub.tamber`, not the design path.

Scope is not taken from the request body. `scopeOf` defaults to `write`, and the dev loops do not pass one, which matches the commit ("the local token's today"). A `propose`-scope client is 403 only when a loop supplies `scopeOf`. Anyone holding that token can `scrub-release` or `scrub-cancel` another client by copying the `client` string off the `scrub` event; that is the same unauthenticated client string as `transaction`.

The tests match the claims they actually execute: `uses` through an alias and a synthetic, the text node not counted, `Session` batch chaining and "a save ends the scrub" in-process, latest-wins while a stub preview is blocked, and a competing write or disconnect after the preview has already returned. They do not cover a hand edit or disconnect while a preview is in flight, release overlapping a new scrub, a non-color style string, or the stdout ordering above. No second write on the release path when nothing else interleaves; release sends one transaction on the opening digests.
