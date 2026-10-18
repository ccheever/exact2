# LLP 1132 — consensus check of 50f1c16317 by Sol (gpt-6.1-sol, max), 2026-10-18

No critical findings.

1. **Major — The new covered-screen rule can backdate `shown` before the route comes into place.**

   D1 selects **“the first frame the host presents after its visit starts”** for a covered screen. But Start explicitly says **“A visit caused by an input therefore starts before its commit.”** ([Shown](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:111), [Start](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:96))

   Suppose an input occurs at 100 ms, an already submitted frame presents at 116 ms, and the handler commits a new base route beneath a fullscreen route at 200 ms. The new rule selects 116 ms for the base route, although it was not in place then. Linux obtains presentation timestamps independently of when its loop processes the flip, so a handler’s work does not exclude this sequence. ([Flip handling](/Users/admin/Developer/observe-lab/exact2/host/linux/src/display.rs:518))

   Alternatively, “starts” means placement here but the measurement anchor elsewhere. That ambiguity also fails the writing requirement.

   **Fix:** Keep the input timestamp as the duration’s anchor, but select covered `shown` from the first eligible frame after the route **comes into place**. Apply the corresponding anchor to UIKit and foreground visits. Add a test with handler work and a frame presented between input and placement. The covered-launch test alone misses this case.

2. **Major — UIKit’s earlier visit start is not carried through to Observe’s mapping.**

   D1 now puts the destination in place **“from when UIKit begins it (`willShow`)”**, while D7 admits only **“screen visits started by a router change.”** ([D1](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:55), [D7](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:443))

   A destination visit can now start, be shown and become interactive during a held swipe. The router changes only after completion. At that later commit, the destination remains in place: no new visit starts. Literally applying D7 therefore excludes the completed native back navigation from Observe’s navigation metrics. Starting another visit at the router commit would instead lose the very readiness interval this revision preserves.

   The source confirms the separation: transition timing begins in `willShow`; `goBack()` is dispatched after completion. ([Beginning](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/iOS/NavigationIOS.swift:946), [Dispatch](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/iOS/NavigationIOS.swift:1079))

   **Fix:** Explicitly associate a completed UIKit navigation’s router event with its existing destination visit, preserving that visit’s start, shown and interactive times. Specify how the module handles milestones received before confirmation and a cancelled transition. Add Observe assertions for completed and cancelled swipes; the current test checks only framework visits.

3. **Minor — The revised Linux test still contradicts the pending-frame rule.**

   The test says **“a covered change that schedules no frame takes the moment the conditions held.”** D2 permits that fallback only when the host **“has no frame pending and schedules none.”** ([Test](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:475), [Rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:206))

   If a frame is already pending when covered content becomes ready, scheduling no additional frame does not select readiness time: D2 selects the pending frame’s presentation. Thus the prior minor finding is only partly resolved.

   **Fix:** Add “has no frame pending” to the fallback test. Separately test a visual change with an already pending frame, and an expression-only change without visual changes.

The prior future-dependent fallback is resolved by making the scheduling decision at the change. The state-driven cause correction and the other writing changes match the commit message. The cited Apple frame estimate, clock facts and 512-event web recovery limit check out. D2 still expressly requires `interactive ≥ shown`; the text retains no entrance or foreground-active condition.

NOT READY