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


## Round 2, 2026-10-03

- **Method:** grok as round 1, `--cwd` a detached worktree at `16da1d643`; the shared round-2 brief (sha256 `2844e246243d0a1021adb03ea1860c8dd555ac22a5bdc0c1f4a3ab355048c80e`). Blind to astra's round 2. The first three sentences of the transcript are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three fixed in `150042a84`:
  1. *scrollend lands the successor.* Fixed: one landing path, the same fix as astra round 2 #1, tested on the second leg.
  2. *Velocity mid-flight.* Fixed. `velocity()` returns 0 while `s.animating` is set, and `animate` drops `s.travel`. Test: "a mid-flight report after a flick carries no velocity".
  3. *UIKit's end has no identity.* Fixed. `animationEnded(_:at:)` ignores an end away from the running animation's target unless the list is clamped at an edge on its own axis, and a stop drops the serial. Test: `testAStoppedAnimationsEndLeavesTheNextOneRunning`.

---

I'll review the round-2 fixes against the round-1 findings and the new diffs, read-only. First I'm loading the review brief, the dispositions, and the commit range.The review skill would write files and post a review; this request is a read-only round-2 check with a fixed verdict format, so I’ll stay in the repo and read the briefs, dispositions, and the fix range.Round-1 dispositions are in the review notes. Next I’ll read the fix diff and the collection, kernel, and test code those notes claim to close.The held-target handoff on the web is still wrong, and that is the path this round is for. The Apple serial, the document fit, the calc whitespace, and the line caps check out.
1. **Must-fix** — A `scrollend` for the animation that just arrived clears the owed animation that arrival already started. `host/web/collection-glue.js:187` and `:445`.
`scrollChanged` lands when the port is within 0.5 px of `s.animating`. If `s.owed` is set, `landed` (`:172`) clears it and `scrollTo`s the successor. Scroll steps then deliver the `scrollend` that was already pending for the animation that finished. `scrollEnded` only checks `s.animating != null`, so that event calls `landed` again and drops the successor's bookkeeping while the browser keeps scrolling. Those frames are sampled and advance `s.sequence`. The runner reads the physical mid-point, so a follow that was held for the second leg is dropped the same way a mid-flight report dropped it before. The new test waits for that intermediate `scrollend` (`collection.test.mjs:328`) and then expects the original sequence (`:335`).
Fix: one landing path. From the scroll listener, do not start the successor; let `scrollend` do it. If the scroll listener must start it, ignore the single `scrollend` already queued for the predecessor (a one-shot flag set only on that path), and give each `animate` a generation so a late `scrollend` cannot land a newer one. Add the retarget case to the test with an assertion that the sequence does not move on the second leg.
2. **Should-fix** — A smooth correction still publishes the reader's last velocity. `host/web/collection-glue.js:69` and `:163`.
`velocity()` does not look at `s.animating`, and `animate` does not clear `s.travel`. Tick sampling is skipped (`:419`), but a commit during the animation still reports `s.travel` for 150 ms (`:271`). End-follow is not `seeking`, so `authored()` does not zero it. A send that starts while a flick is still in that window reports the flick, and `lead()` builds the wrong way. macOS returns nil from `motion` and drops the baseline (`PresenterMac.swift:121`, `:343`).
Fix: clear `s.travel` in `animate`, and make `velocity()` return 0 while `s.animating != null`. Assert a mid-flight report after a real flick is velocity 0.
3. **Should-fix** — Stopping an animation does not retire its serial, and the iOS end callback never reads one. `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:554`, `Collection.swift:252`, `IOS/CollectionIOS.swift:176`.
`scrollViewDidEndScrollingAnimation` calls `animationEnded` with no serial. An ordinary correction removes the list from `animating` and leaves `animationSerial` unchanged (`:176`). `beginAnimation` for the next smooth correction bumps the serial and puts the list back in the set. A `didEnd` delivered for the cancelled animation then takes the non-owed path and clears `animating` while UIKit is still scrolling the new one. The macOS completion handler checks the serial before it calls `animationEnded` (`CollectionMac.swift:321`); this entry point does not. The deferred block's check does not run if `owedTargets` was cleared by the stop.
Fix: bump the serial in the stop (ordinary correction, drag, wheel), and ignore `animationEnded` unless the serial is still the one that animation started with.
Round 1, this review:
1. Resolved. The in-flight target is `port[name] + correction.offset - g.raw`, held in `s.owed`.
2. Not resolved: retargets no longer call `scrollTo`, but `scrollend` still has no identity and lands the successor the arrival just started (finding 1).
3. Resolved. `stopAnimation` instant-scrolls to the current offset before clearing state, from input, `jumpTo`, and an ordinary correction.
4. Resolved. `shift` calls `fit` before updating `owedTargets`, including a zero delta.
5. Resolved. `beginAnimation` stamps a serial; the AppKit completion and the deferred continuation both compare it.
Round 1, the other review:
1. Resolved. Same absolute target as above.
2. Not resolved: ticks are not sampled, and macOS publishes nil, but a web report during the animation still publishes `s.travel` (finding 2). The mid-flight test starts from rest.
3. Resolved. `jumpTo` stops the animation first. A request within 0.5 px of the port, when nothing is animating, is placed instantly and does not set `s.animating`.
4. Resolved. An ordinary correction stops the running animation before the equal-position return, on macOS and iOS.
5. Resolved by the disposition. Reporting the physical port is what made `capture_anchor` drop `follows_end`. Keeping the follow means the report stays on the destination; painting the traversed rows needs both values, which is a `CollectionFacts` change. `llp/1070.000-scroll-into-view.rfc.md:107` says that, and `QUEUE.md` has "Smooth collection jumps cross unbuilt rows". A send's end-follow stays inside the mounted window.
6. Resolved. `bun scripts/caps.mjs` passes. The snap delegate is in `IOS/ScrollSnapIOS.swift`. The bake sets the budget in `js/bake/src/lib.rs:799` after `Module::inspect`; `js/src/lib.rs` does not.
7. Resolved for the two callbacks named there (AppKit completion, deferred continuation). The iOS delegate is finding 3.
8. Resolved. `leading_length` requires whitespace on both sides of `+` and rejects a space before `px`. The old acceptance is three refusals in `kernel/src/style/tests.rs:46`.
Verdict: LAND WITH FIXES


## Round 3 (the last), 2026-10-03

- **Method:** grok as before, `--cwd` a detached worktree at `ff87e269e`; the shared round-3 brief (sha256 `ce513171bce9af2cf88d856ac0c17fa7f5926d9d6841a1e16e00fa7241534cd8`). Blind to astra's round 3. The first five sentences are progress narration, and the run compacted its own context once ("Auto-compacting conversation") before answering.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** both fixed in `795d12cf2`, which is astra round 3's fix. No fourth round (rules/RULES.md, three rounds):
  1. *An edge scrollend lands a running correction.* Fixed: target-clamped-to-content or moved, never an edge.
  2. *UIKit's edge exception.* Fixed. The callback reports whether it is at the clamped target, and `animationEnded` otherwise requires `animationMoved`. The suggested serial check is not possible: UIKit's callback carries no identity, and it is delivered inside the call that stops an animation, before any replacement moves, which the moved rule covers. The test now delivers the stale end at the top edge, the case named here.

---

I'll review the round-2 fixes only: check each finding from both families against the code, then look for bugs those fixes introduce. No files will be changed.The request is a last-round read-only check of the round-2 fixes, so I’ll read both prior reviews and the fix commit before judging whether each finding holds.Round-2 findings are in both review notes. Next I’ll read the fix commit and the scroll paths those notes claim to close.The fix diff is in. I’ll read the landing paths and tests next, looking for a failure the new `scrollend` and serial rules can actually hit.I'll re-check the round-2 landing paths and the edge rules before writing the verdict, so the citations match the current tree.The edge rules are in the current tree. I'll run the line-cap check, then write the verdict.1. **Must-fix** — A `scrollend` while the port is on an edge lands the smooth correction that is still running. `host/web/collection-glue.js:450-456`.

The reader flings and comes to rest at `scrollTop` 0 or at the maximum. The last `scroll` enqueues a frame; that frame's report is synchronous, and the runner commits a smooth correction to a target still inside the range (the retarget test's 800). `animate` clamps and stores that target (`collection-glue.js:166-170`) and calls `scrollTo({behavior:'smooth'})` while the offset is still the edge. The gesture's `scrollend` is already queued and runs before the smooth scroll moves. The handler treats every edge as this animation's end (`at > 0.5 && at < max - 0.5` fails), so `landed` clears `s.animating` and starts `s.owed` if one is set. The frames that follow take the user path: `s.scrolled` samples velocity and `scrollChanged` advances `s.sequence`, which drops the held follow. `animate` already stored a target inside `[0, max]`, so a real clamp is only a later shrink that puts `s.animating` outside that range. The retarget test (`host/web/collection.test.mjs:319`) uses mid-list targets (860, then 960) and never rests on an edge. `llp/1070.000-scroll-into-view.rfc.md:156-157` states this rule.

Fix: land a `scrollend` when `abs(at - s.animating) <= 0.5`, or when `s.animating` is outside `[0, max]` and `at` is the nearest edge. A port sitting on an edge is not identity for a target that is still in range.

2. **Should-fix** — UIKit's end callback skips the distance check whenever the offset is on an edge, so a late end stops the animation that replaced it. `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:554-562`, `host/apple/Sources/ExactKit/Collection.swift:262-264`.

The list rests at the top. A smooth correction runs, an ordinary correction returns it to offset 0 (`IOS/CollectionIOS.swift:175-178`: `stopAnimation`, then `setContentOffset(_:animated: false)`), then another smooth correction to 900 calls `beginAnimation` before the offset leaves 0. A `scrollViewDidEndScrollingAnimation` that arrives while `contentOffset` is still that edge — the delivery `testAStoppedAnimationsEndLeavesTheNextOneRunning` is written to simulate — sets `edge` and passes `at: nil`. `animationEnded` only compares to `animationTargets` inside `if let at`, so the nil skips it and `stopAnimation` clears the replacement while UIKit keeps scrolling. The test writes `contentOffset.y = 300` before the callback (`SmoothCollectionIOSTests.swift:102-104`), which is not the edge the ordinary correction to 0 leaves the list on. The iOS callback still does not read `animationSerial`. `llp/1070.000-scroll-into-view.rfc.md:161-163` states the edge exception.

Fix: pass the offset on every end. In `animationEnded`, accept an edge as a clamp only when the stored target is outside the current content range and the offset is on that edge. Ignore the callback unless `animationSerial` is still the value `beginAnimation` returned for this run.

Round 2, this review:
1. Resolved. With `onscrollend` on the port, the arriving scroll no longer starts the successor (`collection-glue.js:194`); that `scrollend` starts it. The retarget test covers the mid-list double-land. The edge false positive is finding 1.
2. Resolved. `velocity()` is 0 while `s.animating != null` (`collection-glue.js:70`) and `animate` clears `s.travel` (`:169`). The flick test expects a mid-flight report of 0 (`collection.test.mjs:376-387`).
3. Not resolved: `stopAnimation` drops the serial, and an end more than 1 pt from the target is ignored, but an edge passes `at: nil` (`NodeViewIOS.swift:562`), so `animationEnded` (`Collection.swift:264`) still stops the current animation. The new test only drives `y = 300`.

Round 2, the other review:
1. Resolved. Same landing path as finding 1 above; the edge false positive is new.
2. Resolved. The jump test uses `own: true`, and the list reaches 400 and stays there, reported as 390 (`collection.test.mjs:361-374`). `jump` calls `jumpTo` when the list is its own port (`collection-glue.js:543-545`).
3. Resolved. The serial map holds only a running animation; `stopAnimation`, retirement via `animating.subtracting(live)` (`Collection.swift:343`), and `reset` drop it. The iOS test asserts `animationSerial[1] == nil` after a real end.

`bun scripts/caps.mjs` exits 0.

Verdict: LAND WITH FIXES
