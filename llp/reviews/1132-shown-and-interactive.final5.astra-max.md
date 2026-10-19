# LLP 1132 — fifth final review by Astra (gpt-6-astra, max), of c86d4d925f, 2026-10-18

Reviewed **c86d4d925f**. No critical findings.

The fourth review’s declarative-opening, managed-panel and adoption-event findings are addressed. The queued-screen change follows the owner’s decision, but its application still has two gaps below.

1. **Major — D1 “Dialog” / “Shown”, D2: queued dialogs still include another presentation’s animation.**

   > “A dialog visit begins when the host presents a `dialog`…”

   > “A screen whose presentation the host holds back…is shown the same way…”

   The exception covers screens only. Consider an already initialized dialog that the app closes and immediately reopens at 100 ms. UIKit accepts the reopening but queues it behind the old alert’s dismissal. A host frame occurs at 116 ms; dismissal finishes at 400 ms; the reopened alert’s first frame is 416 ms. D1 and D2 assign **416 ms**, including the preceding dismissal animation, despite the owner’s rule.

   **Evidence:** [MenusIOS.swift:895](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:895) accepts the opening, stores `pendingConfirmation` and announces it as open. Actual presentation resumes inside the dismissal completion at [MenusIOS.swift:453](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:453). [ConfirmationAlertIOSTests.swift:195](/Users/admin/Developer/observe-lab/exact2/host/apple/tests/ExactKitTests/ConfirmationAlertIOSTests.swift:195) explicitly covers a queued `showModal`. The conflicting definitions are [RFC:71](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:71), [RFC:106](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:106) and [RFC:172](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:172).

   **Fix:** Begin a dialog visit when the host **accepts its opening**, including an accepted queued opening. Apply the existing held-presentation convention to dialogs as well as screens; refused openings still produce no visit. Add the reopening case. This reuses existing host facts and the owner’s chosen convention; it needs no new event type.

2. **Minor — D4 / “What changes”: the launch’s appearance gate contradicts the new exception.**

   > “On iOS and tvOS, the launch screen is shown at the first frame after UIKit begins to show it (`routeAppeared`)…”

   > “…the launch route gated on `routeAppeared`…”

   At launch, the selected upper sheet can be held behind the initial draw or another sheet’s animation. D1 now gives it the first host frame after its visit begins. D4 still requires its actual appearance, which can occur substantially later. The implementation table lists both requirements without qualifying the gate.

   **Evidence:** [RFC:106](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:106) establishes the exception; [RFC:316](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:316) and [RFC:458](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:458) retain the unconditional gate. [NavigationIOS.swift:479](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:479) holds initial presentations, and [NavigationIOS.swift:513](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:513) installs one additional presentation per synchronization.

   **Fix:** Replace D4’s repeated timing definition with “Launch screens follow D1’s shown rules, including covered and held presentations.” Qualify the table’s gate accordingly. This merits a sentence-level correction. Removing the duplicate definition also reduces the chance of another divergence.

3. **Minor — D7: launch-screen selection has no explicit empty case.**

   > “The launch screen is the first screen in the launch session that its navigation root selects and that is shown…”

   > “A screen that a later navigation brings into place does not count…”

   Suppose an unmarked app first paints a welcome panel with no navigation root. Ten seconds later, a button mounts the navigation root and its first marked screen. That screen satisfies the first quoted definition, but the second excludes it from startup TTI. The following “screen selected at the first frame” sentence suggests no launch screen exists, without saying so explicitly. A similar ambiguity occurs when the launch ends unshown and the first selected screen is shown after foregrounding.

   **Evidence:** The competing rules are [RFC:437](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:437) and [RFC:445](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:445); foreground visits are excluded from navigation metrics at [RFC:446](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:446). Conditional mounting is supported by [rt.js:1181](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1181), and navigation projection operates on whichever roots currently exist at [navigation.js:278](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:278).

   **Fix:** Explicitly limit selection to the initial launch frame: if it selects no screen, or the launch ends unshown, there is no launch-screen fallback. A later first-ever screen cannot acquire that designation. One clarification and an empty-launch-screen case suffice; no additional mechanism is warranted.

NOT READY