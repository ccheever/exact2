# Code review: Video Player conformance, round 1 (708a18e14), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `708a18e14`.
- **Method:** one brief, shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The brief and its sha256 were lost with /tmp in the machine's reboot; the reviews themselves are transcribed from the author's session.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken in round 2 (the hold only on an app's drive), then moot: the harness hold was dropped once main froze every page's media clock itself (2a690c1cb, agent-launch.mjs parityScript, which keeps ratechange). 2 taken: the wait moved into media-glue.js's `installMedia(el, send, later)`; a waiting report is sent only if its load (bumped by a changed src, the load command and `emptied`) is unchanged; a unit test covers a src change, `emptied` and removal before readiness.

---

DO NOT LAND

1. **MATERIAL — Synthetic JS media is frozen while its wasm reference remains live.** [conform.mjs:114](host/web-js/conform.mjs:114) injects the hold unconditionally. Synthetic JS pages use that server; only synthetic wasm pages use the agent’s server ([routing](host/web-js/conform.mjs:285)). Consequently, the media fixture’s `faster` action changes wasm’s rate to 2 and emits `ratechange`, while JS stays at 0 and emits none; `rates` diverges. Cross-browser mode instead freezes both sides, masking that coverage. **Fix:** explicitly disable injection for synthetic targets on both sides. Verify `synthetic-media`, including the `faster` counter.

2. **MATERIAL — Deferred reports survive source replacement.** [glue.js:31](host/web/glue.js:31) checks element identity but retains payloads from its previous source. The [activation batch](host/web/glue.js:1262) can change `src` before readiness. An in-memory execution of the actual glue queued `durationchange\n10` and `timeupdate\n4`, changed the source and fired `emptied`, then delivered both old values after readiness. If the replacement fails, its duration can remain incorrectly 10 indefinitely. [The `emptied` listener](host/web/media-glue.js:106) clears only duplicate suppression. **Fix:** bind deferred reports to a load generation and invalidate them on source replacement/reload; reject superseded time reports. Add delayed-readiness tests for replacement, failure, seek and removal.

Seven existing media-glue tests pass, but bypass the changed readiness wrapper. The ownership probe correctly dropped reports after element replacement. No idle-tick skip changed; its dispositions remain intact. Both files meet the 1,500-line cap.

A harness-only hold is reasonable: [HTML permits rate zero without pausing](https://html.spec.whatwg.org/multipage/media.html#playing-the-media-resource). Actual Chrome/Firefox/WebKit autoplay and cached-load timing remain unverified here; the full browser suite was not run in this read-only checkout.
