# LLP 1132 — second final review by Astra (gpt-6-astra, max), of 7284451c6b, 2026-10-18

Reviewed `7284451c6b`. No critical findings.

Against the prior review: #1’s frame rule is corrected, although its new test needs qualification; #2 follows the owner’s ruling; #3, #5, #6 and #7 are resolved. #4 correctly changes cancellation to measurement creation, but still incorrectly equates creation with mounting.

1. **Major — D2/D6: deferred adoption can hide required web input handling from the counts.**

   > “The emitter flags each element that needs a part, and its mount counts it in every enclosing mark.”

   Consider a served list inside a screen marked `true`, with activation complete. Its early rows contain plain text; a later row conditionally contains the list’s only custom pressable. That row is already in the document, but adoption has been deferred. Its generated code has not run, so the proposed registration has counted no requirement for the input module. The screen can receive its sole interactive time before that module even starts loading.

   **Evidence:** [rt.js:1245](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1245) queues existing DOM rows without executing their row function; [rt.js:1351](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1351) executes it during later adoption. Custom pressables request input handling at [rt.js:914](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:914), and installation is asynchronous at [rt.js:1146](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1146). Adoption on first input therefore cannot make the earlier timestamp correct.

   **Fix:** Explicitly account for served rows awaiting adoption in each ancestor mark’s input readiness. Use the existing lazy enqueue/adoption/cleanup hooks to retain pending installation until adoption establishes the actual part requirements. Add one served-list test whose only required control occurs in a deferred row. This closes a correctness gap in the owner’s mounted-subtree rule; it does not require a tree scan.

2. **Minor — D7: the new exclusion also describes a completed UIKit back transition.**

   > “A router change that leaves its top’s visit in place … gives no navigation metric.”

   An ordinary push ended A’s visit. A back gesture creates A’s next visit at 100 ms; A is shown at 116 ms; the router confirms the completed back at 600 ms. At that router change, A’s visit is already in place. The quoted exclusion therefore conflicts with the preceding bullet, which requires sending the held events, and with the completed-swipe test.

   **Evidence:** [NavigationIOS.swift:958](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:958) observes the transition’s beginning; [NavigationIOS.swift:1079](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1079) dispatches the router change after completion. The conflicting RFC rules are at [RFC:441](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:441).

   **Fix:** Qualify the exclusion: the destination visit must have existed **before this navigation began**. A visit created by the same UIKit transition qualifies when the router confirms it. This needs one sentence, not another event or lifecycle rule.

3. **Minor — D1/D4: later Mac sessions have no defined initial cause and timing origin.**

   > “A session’s first app visit begins with the session…”

   > “Those sessions are not the launch.”

   Opening a second window ten minutes after launch creates an app visit that is neither launch nor foreground. The Start table defines `program` only through a commit putting a route in place or opening a dialog. It does not define the new app visit’s origin, or what its initial screen inherits. Using process start would produce a ten-minute duration; using session creation is sensible but unstated.

   **Evidence:** [RFC:73](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:73), [RFC:85](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:85), and [RFC:308](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:308). Later windows actually create and boot separate sessions at [ExactMac/main.swift:444](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactMac/main.swift:444); an opening timestamp already exists at [main.swift:180](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactMac/main.swift:180).

   **Fix:** Define later sessions’ initial origin explicitly: the initiating input when available, otherwise `program` at session creation. State whether their initial screens inherit that origin. This is a sentence-level completion using an existing timestamp.

4. **Minor — D2: measurement creation does not necessarily involve a mount.**

   > “A section is cancelled when its measurement is created, at its mount.”

   A section can remain mounted in an unselected panel while its required input module fails. Selecting the panel creates its measurement without mounting it. Likewise, foregrounding creates another measurement for a still-mounted section. “At its mount” would backdate cancellation, potentially before the failure or the new measurement.

   **Evidence:** [RFC:140](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:140) creates measurements on selection; [RFC:156](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:156) recreates them for still-mounted sections. [navigation.js:297](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:297) confirms that unselected panels remain mounted.

   **Fix:** Delete “at its mount.” Cancel at measurement creation, retaining the independently defined start. No additional mechanism is needed.

5. **Minor — Tests: the new frame-order test permits an interactive time before input installation.**

   > “The interactive time is the frame that presents the content change, not the module’s arrival.”

   The test specifies that content changes before the input module arrives, but never says the module arrives before presentation. Content can change at 120 ms, present at 133 ms, and input handling arrive at 150 ms. The quoted expectation gives 133 ms, although D2 correctly requires 150 ms.

   **Evidence:** The test is [RFC:476](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:476). [RFC:203](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:203) selects the readiness instant when no content remains pending; [RFC:207](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:207) requires every condition to hold at the chosen time.

   **Fix:** Add “the module arrives before that frame.” The reverse ordering should expect the module’s arrival. The normative rule needs no further machinery.

6. **Minor — D2/Tests: the cited Linux code does not support the covered-change exception.**

   > “Linux can do this beneath a cover (`host/linux/src/display.rs:423`).”

   “This” means changing content without scheduling a frame because nothing changed visibly. Today, a covered text or structure change still marks Linux’s presenter dirty. A pending flip delays the next frame; it does not eliminate it. The citation establishes submission gating, not visibility-based frame suppression.

   **Evidence:** [host.rs:1261](/Users/admin/Developer/observe-lab/exact2/host/linux/src/host.rs:1261) requests paint for created, destroyed or touched nodes without a visibility condition. [presenter.rs:1383](/Users/admin/Developer/observe-lab/exact2/host/linux/src/presenter.rs:1383) propagates that result; other commits unconditionally dirty the presenter at [presenter.rs:715](/Users/admin/Developer/observe-lab/exact2/host/linux/src/presenter.rs:715). [display.rs:423](/Users/admin/Developer/observe-lab/exact2/host/linux/src/display.rs:423) then builds when submission becomes available.

   **Fix:** Remove the Linux attribution and the corresponding no-frame test assumption. Test its actual post-change frame behavior. The generic fallback can remain conditional; implementing visibility-based frame suppression merely to satisfy this example would add unnecessary scope.

NOT READY