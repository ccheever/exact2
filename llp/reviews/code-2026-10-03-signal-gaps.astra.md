# Code review: the Signal Clone's exact2 gaps (41a0e1688..abe4e18e9), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `abe4e18e9`.
- **Method:** one brief (sha256 `c82cec0d4001a8fd282d3817c49e69d20e5767ef262bdc6bf2a4949e5c2b924c`), the same one sent to grok; round 1; blind to the other review. Requested by Charlie through the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** every finding checked in the source; all fixed in `5137f5379` (and the caps moves before it), except #5, which is argued:
  1. *Web retargeting.* Fixed. The target comes from the physical port (`port[name] + correction.offset - g.raw`) and is held in `s.owed` until the animation lands. Tested in `collection.test.mjs`: "a smooth correction that arrives mid-flight is held, reported, and taken where it lands" ends at 960.
  2. *Animation motion as travel.* Fixed. On macOS, `collections.motion` returns nil for an animating list, and `sampleListTravel` skips it and drops its travel baseline. On the web, `s.scrolled` does not `sample` during an animation, and the test asserts velocity 0 in the mid-flight report.
  3. *Authored jump, and a no-op request.* Fixed. `jumpTo` calls `stopAnimation` first. A request within 0.5 px of the port is placed instantly and never sets `s.animating`. Covered by "an ordinary correction, an authored jump or a zero-length request ends a smooth one".
  4. *macOS correction to the current position.* Fixed, and on iOS too. There the same early return left the list marked animating after UIKit's animation had been stopped, which was a worse bug on main. Tests: `testAStoppedAnimationsCallbacksDoNotEndTheNextOne` (macOS) and `testAnOrdinaryCorrectionStopsASmoothOne` (iOS).
  5. *Cold smooth jumps cross unbuilt rows.* Argued, declared, queued. Reporting the physical port mid-flight is exactly what stopped the end-follow (the bug found driving the clone): the runner reads a mid-flight offset as the reader leaving the end. Realizing what shows while anchoring the destination needs both values in the report, which is a `CollectionFacts` wire change and out of this series' scope. LLP 1070.000 §11 now says so instead of promising fling-like realization, and QUEUE has "Smooth collection jumps cross unbuilt rows". A send's end-follow moves within the mounted window and never shows the gap.
  6. *Caps.* Fixed before the round finished. The snap delegate moved to `IOS/ScrollSnapIOS.swift`, and the bake budget moved to `js/bake` (`js/src/lib.rs` is untouched). `bun scripts/caps.mjs` passes.
  7. *Animation identity.* Fixed. `CollectionHost.beginAnimation` numbers each animation. AppKit's completion and the deferred owed-target continuation both check the number.
  8. *calc whitespace.* Fixed. `leading_length` requires whitespace on both sides of `+` and none between the number and `px`; the malformed positive example became three refusals.

---

1. **Must-fix — Web retargeting mixes current and destination coordinates.** [collection-glue.js:487](/tmp/rv-gaps1/host/web/collection-glue.js:487). With the list beginning at 60 px, an animation targeting 860 px, and the current position at 160 px, a new correction requesting list offset 900 should target 960 px. The actual controller instead requests **260 px** in an in-memory reproduction. Compute the absolute destination from physical geometry (`port[name] + correction.offset - g.raw`). Test both smooth and instant corrections arriving mid-animation.

2. **Must-fix — Animation motion still becomes user travel.** The guard at [PresenterMac.swift:367](/tmp/rv-gaps1/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:367) is bypassed: `clipScrolled()` subsequently calls `scrolled()`, which unconditionally samples every list at line 398. Unlike iOS, macOS’s `collections.motion` does not exclude animating lists, so a pending request can cancel itself after two velocity-bearing reports. Web [collection-glue.js:394](/tmp/rv-gaps1/host/web/collection-glue.js:394) also samples animation ticks; end-follow lacks the `seeking` exemption. An in-memory run reported **2,500 px/s without input**, which can reject horizontal corrections. Suppress animation sampling and published velocity, reset the travel baseline afterward, and test multiple ticks and feedback reports.

3. **Must-fix — Web animation state survives superseding authored jumps.** [collection-glue.js:239](/tmp/rv-gaps1/host/web/collection-glue.js:239), [collection-glue.js:490](/tmp/rv-gaps1/host/web/collection-glue.js:490). During an animation toward 800, `controller.jump(1, 400)` physically reaches 400 but subsequent feedback still reports **800**; reproduced with the controller. A no-op smooth request also leaves `s.animating` set indefinitely because unchanged positions produce no `scrollend`. [CSSOM View](https://drafts.csswg.org/cssom-view/#perform-a-scroll). Clear/cancel animation state before authored jumps and finish zero-distance requests synchronously. Add regressions for both cases.

4. **Must-fix — A macOS correction to the current position fails to stop the animation.** [CollectionMac.swift:351](/tmp/rv-gaps1/host/apple/Sources/ExactKit/Mac/CollectionMac.swift:351). If the clip is at 300 while animating toward 1200, an instant correction to 300 returns before cancellation and before clearing `owedTargets`. The old animation continues against the newer correction. Handle cancellation and replacement before the equal-position early return; test an instant correction equal to the current bounds.

5. **Must-fix — Cold smooth web jumps never realize the viewport traversed during animation.** [collection-glue.js:164](/tmp/rv-gaps1/host/web/collection-glue.js:164), [collection-glue.js:239](/tmp/rv-gaps1/host/web/collection-glue.js:239). Intermediate ticks enqueue no feedback, and any other feedback substitutes the destination. The runner has already replaced the mounted window with the destination’s rows, so a distant smooth jump traverses empty spacers. This contradicts [LLP 1070.000:107](/tmp/rv-gaps1/llp/1070.000-scroll-into-view.rfc.md:107), which promises realization of what shows mid-flight. Keep destination bookkeeping separate from physical-viewport realization, and test row coverage during a distant smooth jump.

6. **Must-fix — The existing blocking cap check fails.** [js/src/lib.rs:797](/tmp/rv-gaps1/js/src/lib.rs:797), [NodeViewIOS.swift:619](/tmp/rv-gaps1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:619). Running the read-only `bun scripts/caps.mjs` exits **1**, reporting **1,511** and **1,506** lines respectively against the binding 1,500-line limit. Reduce or extract code and rerun the existing check.

7. **Should-fix — macOS completion callbacks lack animation identity.** [CollectionMac.swift:315](/tmp/rv-gaps1/host/apple/Sources/ExactKit/Mac/CollectionMac.swift:315) identifies an animation only by its destination. A cancelled animation’s delayed completion can match a replacement targeting the same point and clear its state; cancellation also invokes completion handlers. [Apple documentation](https://developer.apple.com/documentation/appkit/nsanimationcontext/completionhandler?changes=_3). Additionally, the deferred continuation at [Collection.swift:247](/tmp/rv-gaps1/host/apple/Sources/ExactKit/Collection.swift:247) checks only set membership, so it can consume a newer animation. Carry an animation identity through both callbacks and test cancel/restart before completion delivery.

8. **Should-fix — The new parser deliberately accepts invalid CSS.** [env.rs:480](/tmp/rv-gaps1/kernel/src/style/env.rs:480) accepts `calc(2.5px+env(...))`, and [tests.rs:37](/tmp/rv-gaps1/kernel/src/style/tests.rs:37) enshrines that acceptance. CSS requires whitespace on both sides of binary `+` and `-`. [CSS Values](https://drafts.csswg.org/css-values-4/#calc-syntax). Preserve valid commuted addition while rejecting missing operator whitespace and whitespace between the number and `px`; replace the positive malformed example with rejection tests.

Verdict: DO NOT LAND

## Round 2, 2026-10-03

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `16da1d643`; brief sha256 `2844e246243d0a1021adb03ea1860c8dd555ac22a5bdc0c1f4a3ab355048c80e` (the fixes `abe4e18e9..16da1d643` against round 1's findings and dispositions). Blind to grok's round 2.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three fixed in `150042a84`:
  1. *The predecessor's scrollend clears its successor.* Fixed. A successor starts only from the predecessor's `scrollend` (from the arriving scroll where the browser has no `scrollend`). A `scrollend` neither at the target nor at an edge lands nothing. The retarget test now reports during the second leg and asserts its destination, velocity 0 and an unmoved sequence.
  2. *The authored-jump test missed the path.* Fixed. A fixture option `own` makes the list its own scrollport, so `jump` goes through `jumpTo` and `stopAnimation`. The test asserts it reaches 400 and stays there, and the report is the port's.
  3. *Serials kept forever.* Fixed. One host-wide counter, with an entry only while an animation runs. `stopAnimation`, retirement in `beginBatch` and `reset` drop it, and the iOS test asserts nothing is kept after the end.

---

1. **Must-fix — The first animation’s `scrollend` clears its replacement.** [collection-glue.js:190](/tmp/rv-gaps2/host/web/collection-glue.js:190) calls `landed()`, which immediately starts `s.owed`. The subsequent `scrollend` calls `landed()` again at [line 447](/tmp/rv-gaps2/host/web/collection-glue.js:447), clearing the replacement’s state while its browser animation continues. This final-`scroll`/`scrollend` ordering follows [CSSOM View’s event queue](https://drafts.csswg.org/cssom-view/#scrolling-events).

   An in-memory controller reproduction targeting 860, then 960, reports content offset **900 → 800**, followed by **1,250 px/s** and two sequence increments without input. This reintroduces request cancellation and lost end-follow. Start the owed animation only after consuming the original completion event, and test reports throughout the second animation.

2. **Should-fix — The authored-jump regression test never exercises the fix.** [collection.test.mjs:347](/tmp/rv-gaps2/host/web/collection.test.mjs:347) calls `jump(1,400)`, but [the fixture](/tmp/rv-gaps2/host/web/collection.test.mjs:51) puts view 1 inside a separate scrollport. Consequently, [collection-glue.js:536](/tmp/rv-gaps2/host/web/collection-glue.js:536) takes the plain-assignment fallback and never calls `jumpTo()` or `stopAnimation()`. The assertion also never requires reaching 400; natural animation completion can satisfy it. Use a fixture where the collection owns its scrollport, and assert that it reaches—and remains at—400.

3. **Should-fix — Animation serials retain every animated collection indefinitely.** [Collection.swift:249](/tmp/rv-gaps2/host/apple/Sources/ExactKit/Collection.swift:249) inserts into `animationSerial`, but completion, collection retirement, and [reset](/tmp/rv-gaps2/host/apple/Sources/ExactKit/Collection.swift:311) never remove entries. [View IDs are never reused within a runner](/tmp/rv-gaps2/runner/src/instance.rs:210), so repeatedly mounting, animating and retiring collections grows this dictionary without bound. Use a host-wide increasing serial with a map containing only active animations; remove entries on completion, cancellation, retirement and reset.

Round-1 dispositions:

- **Astra #1 — resolved:** absolute destinations now use physical geometry.
- **Astra #2 — not resolved:** the direct sampling guards work, but finding 1 exposes replacement-animation ticks as travel.
- **Astra #3 — resolved:** `jumpTo()` cancels animation state and zero-distance requests finish synchronously; its regression test needs finding 2’s correction.
- **Astra #4 — resolved:** ordinary corrections cancel before the equal-position return on both Apple hosts.
- **Astra #5 — not resolved:** distant smooth jumps still cross unbuilt spacers. The separate physical/destination reporting requirement makes deferral reasonable for this scoped work; [§11](/tmp/rv-gaps2/llp/1070.000-scroll-into-view.rfc.md:107) and [QUEUE](/tmp/rv-gaps2/QUEUE.md:18) now acknowledge it explicitly.
- **Astra #6 — resolved:** `bun scripts/caps.mjs` passes.
- **Astra #7 — resolved:** AppKit completion and the deferred continuation check animation serials.
- **Astra #8 — resolved:** `leading_length` enforces the reported whitespace requirements.
- **Grok #1 — resolved:** the incorrect coordinate conversion is fixed.
- **Grok #2 — not resolved:** an old `scrollend` still ends the replacement; finding 1.
- **Grok #3 — resolved:** input explicitly stops the browser animation.
- **Grok #4 — resolved:** macOS fits the document before handling an animated shift, including zero delta.
- **Grok #5 — resolved:** AppKit completion checks serial identity.

Validation: caps passed; controller reproductions ran entirely in memory. No files modified, native builds or browser-suite runs.

Verdict: LAND WITH FIXES
