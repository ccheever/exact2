# LLP 1132 r9 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. R9 resolves r8’s four specific findings, but the following gaps remain.

1. **Major — D1/D2: programmatic presentations can report interactive while their controls remain unavailable.**

   D1 marks a dialog shown at the first frame after presentation begins. D2 waits for transition completion only for transitions “that UIKit makes itself.” Consequently, an already-activated app opening a native alert with `interactive=true` can report interactive during its presentation animation.

   The host explicitly refuses activation then: [MenusIOS.swift:727](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:727) requires `!inTransition`; that state includes `isBeingPresented` and `isBeingDismissed` at [MenusIOS.swift:687](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:687). Programmatic navigation also defers focus through its transition ([PresenterIOS.swift:521](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:521)).

   **Fix:** require the subject’s presentation to accept input, regardless of who initiated the transition. Reuse the host’s existing transition state. Keep shown at the first frame. Test both opening a declared alert and programmatically dismissing a sheet over a pending screen.

2. **Major — D1/D2/D6: selecting a tab need not reevaluate an app-owned part.**

   Consider a shell part beside the routes in an initially unselected tab panel. Its declaration is already true. D1 correctly makes it ineligible while that panel is unselected. Selecting its tab changes neither its declaration nor its owning app visit: the app was current throughout.

   D2’s evaluation triggers and D6’s cost model cover declaration changes and **visit** transitions, but omit a part’s enclosing-panel eligibility changing independently. Evaluating only the newly selected screen’s declarations therefore misses this app-owned part. The panel’s actual visibility and input eligibility change at [navigation.js:297](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:297).

   **Fix:** index parts by their enclosing panel context and reevaluate affected parts on selection changes, even when their owner remains current. Extend the test at RFC line 292 to switch into the panel without any binding or mount change. This preserves the promise of no recurring tree walk; the present trigger list does not yet establish it.

3. **Major — D4/D5: undeclared roots lose immediate service release on interruption or failure.**

   D4 releases services at interactive or cancellation when the root declares. Otherwise, it specifies only settled or the 30-second bound. But D5’s built settlement measurement produces no settled time after poisoning, activation failure or interruption.

   Thus an undeclared app that backgrounds or fails during startup can leave its services waiting until the bound. A failure before the first frame also leaves no visit cancellation under D1. Existing behavior releases services on terminal startup outcomes: [Launch.swift:366](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:366) and [marks.js:68](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:68). Observe’s README documents that a kill before service loading loses the measurements.

   **Fix:** release services on every terminal startup outcome independently of declaration presence and event suppression. For an undeclared root, use settled **or interruption/failure**, retaining the bound. Test backgrounding without a root declaration and activation failure before first frame.

4. **Major — D1/D7: the launch-screen mapping does not cover a redirect before its first frame.**

   Under D1, initial screen A disappears without events if replaced before presentation. Replacement B starts from its own causal commit. D7 classifies router-started visits with cause `program` as navigations, while identifying the launch screen through cause `launch`. No rule transfers that classification to B.

   Following those rules makes the first screen actually shown an ordinary navigation, or leaves launch-screen classification unspecified. The built code explicitly keeps redirects within launch until a screen is shown ([NavigationMarks.swift:207](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:207)).

   **Fix:** make Observe identify the launch screen from the first screen actually shown in the launch session, rather than requiring cause `launch`. Its start can remain the replacement commit as the proposed test requires. Assert the launch classification as well as the timestamps; keep this policy in Observe.

5. **Minor — D1: “a dialog … always has an `id`” is an inaccurate as-built claim.**

   The cited [lint.rs:145](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/lint.rs:145) rejects `open`; it does not require `id`. The menu checker explicitly permits the absence of a literal ID ([menus.rs:201](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/menus.rs:201)).

   **Fix:** say that opening a dialog through the listed commands requires an addressable ID. Do not add another compiler refusal merely to make the sentence true.

Questions 4 and 5 state their central trade-offs fairly. I would not repeat the split-view omission or duplicate-ID aggregation as unresolved findings.

For simplicity, choose **per-mount parts with IDs as labels**. That removes the literal-ID placement error, membership/conjunction bookkeeping, last-member cancellation and group reopening without losing independent fragment measurements. The journal-volume cost is real and is now stated accurately. Also remove the new public `launch.settled` event unless a consumer needs it: D4 expressly keeps service release internal, and Question 2’s recommended declarations-only policy leaves no named consumer for that event. Settlement can remain a diagnostic.

The receipt paths otherwise credibly account for invalid initial values, clears, renewal, viewless Apple nodes and component declarations during eager web adoption. Ordinary boolean typing fits Contract; the naming exception is openly put to Charlie. D7’s Observe inventory matches the cited machinery. External platform references were not fetched under the local-only constraint.

**NOT READY.**