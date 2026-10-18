# LLP 1132 — consensus check of d5a92f375f by Sol (gpt-6.1-sol, max), 2026-10-18

No critical findings.

1. **Major — The covered-screen anchor can still backdate `shown` before the current visit.**

   > “the first frame the host presents after its route comes into place”

   This fixes the handler-work counterexample, but a new visit does not always involve new placement. A foreground visit starts while its route is already in place; a URL-changing `replace` can also retain the same route element. The quoted rule can therefore select an earlier visit’s frame. Its explicit inclusion of foreground visits contradicts the test requiring the first frame **after the return**. ([Covered rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:111), [foreground](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:100), [replace](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:67), [test](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:483))

   **Fix:** Distinguish the duration’s start from the moment the current visit comes into existence. Select the first eligible frame after placement, the replacement commit, or the foreground notification, respectively. Preserve the earlier input timestamp as the duration’s anchor. Test both a retained covered route across backgrounding and a covered replacement that retains its element.

2. **Major — Observe’s swipe-discard rule lacks the required journal signal.**

   > “The module holds that visit’s events until the router change names it … and drops them if the user cancels the transition.”

   A swipe destination can now journal both `shown` and `interactive` before cancellation. D2 journals `cancelled` only when a measurement ends **before becoming interactive**, so this cancellation produces no specified event. D7 defines no other transition-outcome or visit-end event, and its `router` event carries an entry and URL rather than the confirmed visit identity. The consumer consequently lacks the information needed to discard that completed measurement or safely associate a later navigation to the same entry and URL. ([Mapping](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:443), [cancellation rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:212), [event contract](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:401))

   UIKit supplies the cancellation fact, but currently passes it directly to `platformCancelled()`; no corresponding consumer signal is specified here. ([Callback](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/iOS/NavigationIOS.swift:959))

   **Fix:** Specify a transition outcome keyed by visit identity, available even after that visit becomes interactive, and an unambiguous association between the confirming router event and its visit. Test an already-interactive destination whose swipe is cancelled, followed by navigation to the same entry and URL.

3. **Minor — The new tab-selection test omits the owner’s shown prerequisite.**

   > “A section in a panel that a tab selection shows … is interactive at the selection, without waiting for the frame that shows the panel.”

   This holds for an already-mounted section whose **owner visit is already shown**, such as the app-owned section beside routes in the existing section test. It does not hold generally for a section inside the selected tab’s route: selection starts that screen’s new visit, whose shown time is still a frame away. D2 expressly requires interactive at or after the owner visit’s shown time. ([New test](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:487), [existing fixture](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:501), [D2](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:175))

   **Fix:** Qualify the immediate-selection test with an already-shown owner. Add the route-owned counterpart, asserting that visibility adds no additional frame wait beyond the owner’s shown time.

NOT READY