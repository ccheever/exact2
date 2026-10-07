# Code review: Video Player conformance, round 1 (708a18e14), 2026-10-07 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, edits denied, a detached worktree at `708a18e14`.
- **Method:** one brief, shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The brief and its sha256 were lost with /tmp in the machine's reboot; the reviews themselves are transcribed from the author's session.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken in round 2 (the hold only on an app's drive), then moot: the harness hold was dropped once main froze every page's media clock itself (2a690c1cb, agent-launch.mjs parityScript, which keeps ratechange). 2 taken: the wait is in media-glue.js's `installMedia` and `host/web/tests/media-glue.test.mjs` pins it (a report before readiness is sent once, in order, after it).

---

LAND WITH CHANGES

1. BLOCKING — `serve()` freezes the JS synthetic page and leaves the wasm synthetic page running. `host/web-js/conform.mjs:102-114` injects `HOLD_MEDIA` into every HTML response, and `conform.mjs:287` opens the JS page at `js.url` from that server. For a `t.contract` drive, `conform.mjs:285` opens the wasm page as `{ webDist, plan }`, which `scripts/agent.mjs:121-134` and `:254-274` serves itself and then `exact.reload`s, with no hold. The comment at `conform.mjs:102-103` describes only the wasm page.

`host/web-js/conformance/media.contract:35-36` (`faster`, the third press button at `:92`) sets `rate = 2`. `:70-71` increments `rates` on `ratechange`, and `:89` prints `rates ${rates}`. The auto-tap loop (`conform.mjs:70`, `:434-446`) reaches `faster` inside the default 10 steps and stops on the first divergence. On the wasm page the glue writes `playbackRate = 2` and `ratechange` fires, so `rates` becomes 1. On the JS page the overridden setter writes 0; the rate is already 0, so no `ratechange` fires and `rates` stays 0. `slots.rate` is 2 on both (the action assigns it). `slots.rates` and the `seen` text diverge.

Headless Chrome, the injected script, `apps/video-player/assets/motion.mp4`, autoplay allowed: at `loadstart` the rate was 1 and the hold wrote 0; after 1s of autoplay `currentTime` was still 0, `paused` was false, and `play`/`playing` had fired with no `timeupdate`; assigning `playbackRate = 2` stayed 0 and fired no `ratechange`; seeking to 1.5 fired `timeupdate` at 1.5. Descriptors are configurable and the override did not throw.

`--browser firefox|webkit` opens both sides on `js.url` (`conform.mjs:280-282`), so those two pages match each other while neither counts `ratechange`. App drives stay symmetric: both URLs come from `serve()` (`:259`, and `:465` for `app.test.contract`). No `app.contract` other than video-player follows `timeupdate`, and video-player does not bind `playbackRate`.

Fix: hold only when `!t.contract`. `serve(dir, hold)` and pass `!t.contract` at the three call sites (`:259` both, `:264`, `:465` keeps the hold). Rewrite the comment so it says the JS synthetic page is served here.

2. MINOR — nothing fails in a unit test if `syncMedia` drops the early report again. `host/web/glue.js:30-32` defers until `moduleReady`, matching the image path at `:519-521`. The lost opening `durationchange` (wasm duration 0 for the session) shows up only as a loaded conformance flake. A seam that calls the defer predicate with `inputReady` false, then true, and asserts one `exact_dispatch` of that payload, would pin it.

Change 2 is sound on the paths that can actually run. The payload is the string captured at emit. Each report registers one `moduleReady.then(go)`, and those run in order. `activateData` sets `inputReady` (`glue.js:1266-1267`) and `resolveModuleReady()` runs in the same turn after that returns (`:1476-1479`), so the flush is microtasks before the next media task. `go` sends only when `views.get(id) === el && inputReady`, which drops a destroy or a reused id (`views.delete` is synchronous in that apply). `emit` already refuses a retired or disconnected element before queueing, so a report is not deferred for an element that is already retired. A later `timeupdate` is a later task and lands after the flushed 0. `exact.reload` leaves `inputReady` true, so the defer does not re-arm. `host/web-js/media.js` has no such gate. `addSession` (`conform.mjs:206-211`) does not copy `position`, and `playbackState` follows `paused`, which rate 0 leaves false (`media-glue.js:192`, `:200` publishes `el.playbackRate || 1` to the platform).

Holding both app pages is the right fix under LLP 1042 §3: media stays a real-time executor, seek still moves time, and the screenshot diff already omits Video rects. The synthetic plan is written to compare `ratechange`, so it has to keep real time.

This diff does not widen a presenter skip. The idle-tick hole closed in `llp/reviews/code-2026-10-04-idle-tick-r3` (prepare / carried row) is untouched. No prior `code-2026-10-05-video-player-conformance` review exists. `glue.js` is 1494 lines. Firefox and WebKit were not run here; both cross-browser pages get the same script, and a Safari-internal reset of rate back to 1 after play (WebKit 294663) would be outside the setter override.
