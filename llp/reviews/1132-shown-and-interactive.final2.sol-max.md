# LLP 1132 — second adversarial review by Sol (gpt-6.1-sol, max), of 21da132b8, 2026-10-17

No critical findings. The macOS sheet distinction, web fallback correction, and host-specific test qualification check out. The compiler error is treated as settled. External links were not fetched.

1. **Major — D2: runtime readiness still omits functional input dependencies.**

   The revised inventory includes shortcuts, pans and motion, but misses keyboard handling for `radiogroup`, `tablist` and `menu`. The emitter explicitly loads input for these roles ([emit.rs:986](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/emit.rs:986)); their arrow-key behavior lives in the deferred module ([input-glue.js:71](/Users/admin/Developer/observe-lab/exact2/host/web/input-glue.js:71)). A marked tablist containing native buttons could therefore report interactive before its keyboard navigation works.

   “Native hosts install all of it at boot” also overlooks native module views. Apple completes data activation before scheduling their attachment ([Session.swift:1277](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Session.swift:1277), [NativeModule.swift:451](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NativeModule.swift:451)). On the web, those elements remain empty until their adapter and artifact load ([rt.js:800](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:800)).

   **Fix:** derive dependencies from every functional input installation path, including roving keyboard groups and native view attachment. Use their existing readiness/failure states, scoped to the measured subject. Test delayed keyboard-group initialization and a marked native widget whose attachment follows data activation.

2. **Major — D2/D6: covering a visit during its frame wait can still stall its interactive time.**

   The hidden-visit fix handles a readiness change applied while already hidden. It does not specify this sequence:

   - A is shown.
   - A’s mark becomes true and its text changes; it waits for presentation.
   - Before that frame, fullscreen B hides A.

   No frame now shows A’s change. Its mark, lifetime and entrance remain unchanged. D6 evaluates a starting visit’s own marks; it does not require reevaluating A when B changes A’s visibility. D2’s “moment the host applies that change” also leaves unclear whether to backdate to A’s earlier, still-visible commit.

   **Evidence:** [D2’s presentation rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:139), [D6’s evaluation costs](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:262).

   **Fix:** make host-imposed coverage changes reevaluate affected pending visits. When coverage makes a presentation wait unnecessary, record eligibility at that coverage change, provided readiness still holds. Add this ordering to the covered-screen test.

3. **Major — D1/D2: foreground renewal reinstates the presentation wait beneath fullscreen covers.**

   D1 ends all visits on backgrounding and creates new visits for **every screen in place** on return. Suppose A was shown, fullscreen B covers it, and the app backgrounds and returns. A’s replacement visit has never been shown. D2 requires that visit’s shown time, while its hidden-update exception also requires “the visit has been shown.”

   A can consequently wait indefinitely for B to close, even if A’s expression becomes true while B remains visible.

   **Evidence:** [foreground renewal](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:81), [interactive prerequisites](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:123).

   **Fix:** retain a previously presented route’s presentation eligibility across foreground renewal, keep the new foreground start, and explicitly exempt such covered visits from requiring another visible presentation. Genuinely never-presented routes should continue waiting. Test background/foreground with fullscreen B retained over pending A.

4. **Major — D1 Start/D4: the web launch has no valid “process start” definition.**

   D1 gives `launch` the start “process start”; D4 requires hosts to journal it. The web implementation’s `marks.process = 0` is a document-relative convention, not the browser process’s start ([marks.js:260](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:260)).

   This matters for prerendering: a page activated ten seconds after creation can have shown clamped to activation yet still acquire approximately ten seconds of generic launch duration. Clamping shown does not correct start. Observe’s separate activation-based formula does not repair Exact’s own event.

   **Fix:** define the web launch start as page activation, using the navigation timing origin for an ordinary, non-prerendered page. Keep document creation separately only if a consumer needs it. Assert the generic event’s start and duration for a prerendered page.

5. **Major — D4 Services/Tests: the shutdown fix promises more than the web host can guarantee.**

   “At the end of the process the services load and flush before it exits” is stated across hosts, and the test similarly requires flushing before an app quits.

   Apple has an owned termination callback and synchronous service loading. The web service loader uses asynchronous `import()` ([marks.js:335](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:335)); a closing page cannot ensure that an unloaded service downloads, initializes and flushes before destruction. Its existing mechanism instead preserves journal events for recovery ([marks.js:329](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:329)).

   **Fix:** qualify the guarantee by platform. Require synchronous loading and flushing at supported native orderly shutdowns. On the web, preserve the journal at the available lifecycle notification and recover it later; flush an already loaded service through its supported exit path. Split the acceptance assertion accordingly.

6. **Minor — D1 Sections/D2: pre-presentation section cancellation has no defined publication rule.**

   D1 says a visit ending before its first frame “leaves nothing.” D2 journals cancellation when a section’s measurement ends.

   Consider B mounting a pending section, removing that section, then itself disappearing—all before B’s first frame. The section’s `unmounted` cancellation can already have been journaled, although B subsequently leaves no visit event. The text does not say whether “leaves nothing” includes its sections or permits these orphaned cancellations.

   **Evidence:** [pre-frame suppression](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:100), [section unmount](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:112), [cancellation publication](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:141).

   **Fix:** withhold section events until their owner qualifies for publication. Discard them if the owner ends unshown; otherwise publish retained cancellations when it is shown. Add the sequence above as a test.

7. **Minor — D3/D6/Tests: the compiler error leaves unreachable adoption machinery and fixtures.**

   D3 now refuses marks anywhere inside a `for` row, including through components. D6 nevertheless requires immediate adoption of rows containing marks, a temporary measurement of 20,001 rows “each with a section,” and an adoption-cost measurement. The web tests still require marked rows beyond the adoption budget.

   **Evidence:** [compiler refusal](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:180), [eager adoption](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:256), [measurement fixture](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:266).

   **Fix:** remove the row-specific eager-adoption requirement and corresponding tests. Measure supported cases: a marked container over many unmarked rows, plus independently mounted component copies. This removes machinery without changing the accepted compiler error.

8. **Minor — D1 Sections: the start formula and cause rule disagree after tab selection.**

   For an already mounted section in an unselected panel, selection correctly becomes its new start. The next bullet nevertheless assigns “the visit’s cause when the section was already mounted.” A tab selected by a tap can therefore produce a section event starting at that tap but labelled `launch`.

   “Once per visit of its owner” also conflicts with starting another measurement after deselection and reselection within the same owner visit.

   **Evidence:** [section start and cause](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:108).

   **Fix:** take the cause from the operation supplying the winning start timestamp, with explicit tie precedence. Describe measurements as one per contiguous eligible mount interval within an owner visit.

9. **Minor — D5/D7: the agent cannot show the promised settled time on Apple.**

   D5 promises that “the agent’s `timing` state shows” settled. D7 retains the exclusion of Apple startup timing under `EXACT_AGENT=1`. Today both initial timing setup and subsequent evaluation are disabled there ([Launch.swift:121](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:121), [Launch.swift:285](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:285)). Renaming the state section cannot produce the missing diagnostic.

   **Fix:** remove the agent-time promise, or explicitly specify a diagnostic-only settlement calculation under the agent’s clock. State which behavior is intended.

NOT READY