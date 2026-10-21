# Code review: a smooth list correction is one retargetable motion on iOS (5220497f8), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `5220497f8`.
- **Method:** one brief (sha256 `293ec6973a6211cd6c6d2c244712e2bc1747443aac195f7ed42a4a808df41d48`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** Findings 1-3 and 5 are resolved by the redesign: the offset is driven per frame on the model, so wheel, drag handoff, hit-testing, occlusion, scroll events and stickies read what shows, and UIKit's delegate stale check compares the real offset again. 4 taken: pending starts keyed by serial per list, cancelled by stopAnimation (which reset and retirement call). 6 in part: a test retargets mid-flight from an interior offset and checks continuity and landing, and a drag stops a moving one; DEFERRED: wheel, two-list and reset cases; run-loop timing remains wall-clock.

---

Static review only; no files changed or tests run.

1. **Should-fix — Wheel and drag handoff use the destination before stopping.** [AgentIOS.swift:662](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:662) computes both scroll eligibility and `target` from the model offset, then calls `haltOffsetAnimation`. If presentation is at 400 and the destination is 1000, a −100 wheel lands at 900, not 300. A wheel toward the destination can incorrectly chain outward because the model already appears at the edge. [ScrollViewIOS.swift:69](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/ScrollViewIOS.swift:69) makes the same mistake when deciding gesture handoff, before `willBeginDragging` can halt anything.

   **Fix:** Determine eligibility from the presented offset. Once the receiving scroller is selected, halt it, reread its adopted offset, and calculate the wheel destination. Use presented offsets for nested drag handoff too.

2. **Should-fix — Interactive hit testing and agent occlusion now disagree with the screen.** The new animation explicitly allows interaction at [CollectionIOS.swift:182](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:182), but [ScrollViewIOS.swift:24](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/ScrollViewIOS.swift:24) and `NodeView.hitChildren` use model coordinate conversion. Meanwhile, [AgentIOS.swift:204](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:204) computes the target’s box through that same model hierarchy, and `tap` checks that model point with `win.hitTest`. During motion, this can accept a destination row that has not appeared, reject a currently visible row, or deliver a real touch to a different row. The node inspection’s own-layer presentation adjustment does not account for an animated ancestor’s bounds.

   **Fix:** Incorporate animated scroll ancestors’ presentation offsets into coordinate conversion and hit testing, or adopt the presented offset before resolving an interaction. Apply the same rule to agent boxes and occlusion checks. `clock settle` protects subsequent settled reads; it does not protect direct mid-flight taps.

3. **Should-fix — The new motion bypasses the existing per-frame scroll observation path.** At [CollectionIOS.swift:183](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:183), the setter publishes the destination immediately. [NodeViewIOS.swift:629](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:629) consequently forwards that destination to `onScrolled` and scroll-event delivery. Core Animation’s subsequent presentation movement does not repeatedly execute this setter. Runner `frame`/`measure` observations therefore get the destination early. Text visibility also continues using model bounds/conversions at [TextRasterIOS.swift:344](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/TextRasterIOS.swift:344).

   The deferred setter additionally runs outside `collections.correcting`, so [ScrollPumpIOS.swift:124](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/ScrollPumpIOS.swift:124) can sample the model jump as velocity. Although `collections.motion` masks it, the pump’s text and fill calculations consume `velocity` directly.

   **Fix:** Keep destination geometry specifically for collection planning, but sample presentation geometry while the animation runs for actual viewport observations and painting. Exclude correction assignments from user-travel sampling and rebase that sampling when motion stops.

4. **Should-fix — Queued starts have no cancellation identity.** [CollectionIOS.swift:169](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:169) checks only whether that *ID* is currently animating. A smooth → ordinary → smooth sequence before the next turn leaves two queued closures; both can start the replacement target and replace its serial. The second assignment may be a no-op because the first already set the model target, undermining completion ownership.

   Reset/recreation has the same ownership gap: an old closure can use a retained old scroll view with the replacement ID’s target. Weak capture prevents retention, not stale execution. [Collection.swift:266](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:266) does not invalidate `startOwed`, and [CollectionIOS.swift:195](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:195) clears *every* list’s coalescing marker without cancelling any closure. A later correction to another pending list can therefore enqueue another start.

   **Fix:** Give each pending start a stable token, retained across same-turn target updates. Invalidate it on stop, retirement and reset; verify the token and current scroll-view identity before starting. Clear only the affected list. Ignore unchanged targets while an existing motion owns their completion.

5. **Should-fix — The old delegate can still finish the new animation without its serial.** [NodeViewIOS.swift:596](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:596) still calls `collections.animationEnded`. Its stale-callback discriminator compares `contentOffset` with the target. Under the new implementation, that comparison succeeds throughout the animation.

   An end/cancellation callback from a preceding UIKit scroll can therefore retire the current collection animation. UIKit scrolling remains reachable for these nodes through `applyPendingScroll`; that path is not excluded for collection-owned lists.

   **Fix:** Make the serial-guarded UIView completion the sole end authority for these collection corrections. Keep the UIKit delegate’s end-follow bookkeeping for ordinary scrollers, but do not let an unrelated UIKit callback retire a CA-owned collection motion.

6. **Should-fix — Tests do not exercise the claimed in-flight behavior and introduce timing sensitivity.** In [SmoothCollectionIOSTests.swift:61](/tmp/x17-review/host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:61), both targets arrive before the first run-loop spin: this tests coalescing, not retargeting. The interruption/replacement tests likewise interrupt before startup. Assertions check animation-key presence and eventual model offsets, never presentation continuity, actual mid-flight takeover, or stale completion ownership.

   The fixed 30 ms and 600 ms spins—and [SettleScrollAnimationIOSTests.swift:45](/tmp/x17-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:45)—depend on transaction/display scheduling meeting wall-time assumptions.

   **Fix:** Synchronize on animation startup and completion with bounded expectations. Retarget after observing an interior presentation offset; check continuity and final landing. Add running-animation wheel/drag cancellation, queued stop/restart, two-list cancellation, and reset/recreation cases. Check native work remains in flight before actual completion.

Other requested checks:

- **Presented-range realization:** No. `geometry` reports the destination, `changed` bypasses rescue while animating, and coverage helpers use model bounds. This is the existing, explicitly declared spacer-crossing limitation in [LLP 1070.000:107](/tmp/x17-review/llp/1070.000-scroll-into-view.rfc.md:107), not a new finding.
- **Capture/restore and clamps:** Normal batch capture/restore excludes collection-owned lists, so `followedTop` and `followingEndAnimated` remain coherent for ordinary scrollers. `correct` clamps the authored main-axis destination; its other-axis read is not inherently wrong. `shift` deliberately retargets the destination.
- **Removal keys and touch takeover:** The filter examines `CAPropertyAnimation.keyPath`, so arbitrary additive animation *names* do not defeat matching `bounds`/`bounds.origin`. It also matches size animations and does not descend into groups. Static review cannot certify UIKit’s emitted animation shape or drag-baseline continuity. Adopting presentation into the model is appropriate; do it without implicit animation and test a real ongoing pan.
- **Clock, frozen timing and macOS:** `animating` covers the queued turn; `nativeGeometryInFlight` detects running bounds animations. `agentFreezes` is immutable and ordinary correction entry bypasses animation. iOS no longer normally populates `owedTargets`, leaving `landAnimation` largely inactive there. The shared change adds only unused macOS storage; I found no macOS behavioral regression.
- **Retarget continuity:** `beginFromCurrentState` is intended for in-flight retargeting, so I would not flag it alone as a positional-jump defect. It does not establish a velocity-continuity guarantee, and §6.8 does not require one. Actual continuity still needs the missing presentation-level test. [Apple documentation](https://developer.apple.com/documentation/uikit/uiview/animationoptions/beginfromcurrentstate)

Verdict: LAND WITH FIXES
