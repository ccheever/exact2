# Code review: the Signal Clone's exact2 gaps (41a0e1688..abe4e18e9), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, one fresh session with `--cwd` a detached worktree at `abe4e18e9`. `--always-approve` because plan mode cancels shell reads; the brief instructs read-only, and the worktree was clean afterwards.
- **Method:** the shared brief (sha256 `c82cec0d4001a8fd282d3817c49e69d20e5767ef262bdc6bf2a4949e5c2b924c`); round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first three sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all five fixed in `5137f5379`:
  1. The in-flight target is absolute, held and taken on landing (the same fix as astra #1, tested).
  2. There is no smooth `scrollTo` while one runs. A retarget or shift goes to `s.owed`, so no cancelled animation fires `scrollend`. `scrollend` lands only the current animation, without advancing the sequence.
  3. Input stops the browser's animation (`stopAnimation`: an instant scroll to where it is) before clearing the state.
  4. On macOS, a shift under an animation fits the document to the new extent first, including a zero delta (`fit`, shared with `correct`).
  5. Each animation has a serial, and AppKit's completion compares it (the same fix as astra #7, tested).

---

I'll review only the five commits from `41a0e1688` to `HEAD`, starting with the review criteria and the diff itself.The diff is large around collection scrolling. I'll read the macOS, web, and kernel changes against the existing iOS behavior and the LLP notes.The animation state machines are the riskiest part. I'll compare the new macOS and web paths with the iOS implementation they claim to match.The web host mis-applies a second absolute correction while a smooth scroll is still running. That is the path this change is for: a follow-up end-follow or a converging `scrollIntoView`.

1. **Must-fix** — A smooth correction that arrives while one is already running is turned into the wrong scroll offset. `host/web/collection-glue.js:487`

`correction.offset` is an absolute content offset (`runner/src/instance/collection/api.rs:92`). While `s.animating` is set, the host reports that destination as the port (`collection-glue.js:239`: `g.raw + (s.animating - scrollTop)`), and the runner plans the next offset from that report. The new arm then does `scrollTop + correction.offset - reportedDestination`. That is a delta from the visual position, short by however far the animation still has to go.

On the collection fixture (content offset = `scrollTop - 60`): the animation is headed to scrollTop 860 (content 800), the port is visually at 400, and the runner sends absolute offset 900. The code scrolls to 500. The content offset that matches 900 is 960. iOS and macOS store that absolute point and take it when the animation lands (`CollectionIOS.swift:169`). The web test only commits one smooth correction (`host/web/collection.test.mjs:300`), so this never runs.

Fix: convert with the same expression as a correction at rest, `port[name] + correction.offset - g.raw`, and keep that target until the current animation ends instead of calling `scrollTo({behavior:'smooth'})` again. Add a test that commits a second absolute `smooth` correction midway and expects the final scroll offset.

2. **Should-fix** — Retargeting restarts the browser animation, and `scrollend` then looks like the reader took over. `host/web/collection-glue.js:460` and `:488`

Every later smooth correction, and every anchor shift while `s.animating` is set, calls `place(..., true)`. Restarting the ease on each converging correction is the creep iOS avoided by holding `owedTargets` (`CollectionIOS.swift:169`). `scrollend` (`collection-glue.js:420`) treats any end while `s.animating` is set as terminal: it clears the flag, sets `s.offset` to `NaN`, and `scrollChanged` advances the sequence. A browser that fires `scrollend` when a new `scrollTo` cancels the old one (WebKit did this until it matched Chromium's retarget) drops the in-flight correction and moves the sequence past the ones still to come.

Fix: adjust `s.animating` in place and call `scrollTo` smooth only when no animation is running. Ignore `scrollend` unless it belongs to the animation that is current.

3. **Should-fix** — Pointer, key, or wheel input clears `s.animating` and leaves the smooth scroll running. `host/web/collection-glue.js:395`

`touched` sets `s.animating = null` and does not stop the browser animation. Later `scroll` events take the normal path: each frame increments `s.sequence` and `sample` records velocity. Two such reports cancel a `scrollIntoView` (`into_view.rs:220`) and move the sequence past an end-follow. A `pointerdown` that does not itself scroll (a tap on a row) is enough. macOS stops the animator before it records the gesture (`NodeViewMac.swift:1009`).

Fix: on those input events, `scrollTo({behavior:'instant'})` to the current offset first, then clear `s.animating`.

4. **Should-fix** — On macOS, an anchor shift during a smooth correction never grows the document to the new extent. `host/apple/Sources/ExactKit/Mac/CollectionMac.swift:287`

iOS always `fit`s the scroll view to `extent` before it moves `owedTargets` (`CollectionIOS.swift:86`). The macOS branch writes `owedTargets` and returns, so `correct`'s `setFrameSize` does not run. `takeShift` still calls `shift` when the delta is 0 (`Collection.swift:63`), which is an extent-only revision. The owed point can sit past the document; `landAnimation` then clamps short of the anchor. LLP 1070.000 §11 says macOS matches iOS here.

Fix: resize the document to `extent` before updating `owedTargets`, as `correct` already does, including when the delta is 0.

5. **Should-fix** — The macOS completion handler identifies the running animation by point equality. `host/apple/Sources/ExactKit/Mac/CollectionMac.swift:318`

The handler bails unless `animationTargets[id] == target`. An ordinary correction or a wheel clears that entry and starts another animation (`CollectionMac.swift:360`, `NodeViewMac.swift:1010`). If the replacement is headed to the same point — normal for a vertical list, where x stays put and y comes back to the same offset — the stale completion runs `animationEnded` and clears `animating` while AppKit is still scrolling. Those frames are then sampled as the reader's travel (`PresenterMac.swift:367`). `SmoothCollectionMacTests` only checks flags in the same turn as `apply`; it never lets a completion run.

Fix: stamp each animation with a generation and ignore a completion whose generation is no longer current.

Verdict: LAND WITH FIXES

