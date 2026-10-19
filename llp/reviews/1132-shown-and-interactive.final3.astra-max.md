# LLP 1132 — third final review by Astra (gpt-6-astra, max), of ad24895c52, 2026-10-18

Reviewed **ad24895c52**. No critical findings.

Against the second review: #2 and #4–#6 are resolved. #1’s documented adoption boundary is a reasonable tradeoff, although its new explanation overstates an adoption trigger. #3 now supplies an origin, but introduces the redirect conflict below.

1. **Major — D7: the journal lacks the selection fact needed to identify a launch screen without a router.**

   > “The launch screen is the first screen in the launch session that its navigation root selects and that is shown.”

   Consider an app navigating through `state screen`, with an unmarked root. It initially selects fullscreen route B over A. Both routes are shown at the first frame under D1; A becomes interactive at 100 ms and B at 500 ms. Observe must use B.

   The proposed events do not distinguish which route the navigation root selected. Only `router` carries the top visit’s identity, and this app produces no router changes. Neither the screen labels nor `shown`’s marked flag supplies the missing fact. Selecting the first interactive screen gives an early startup `tti`; declining to select one loses it.

   **Evidence:** The event fields are specified at [RFC:398](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:398), and screen labels at [RFC:164](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:164). [router.rs:667](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/router.rs:667) returns no change without a router. State-driven navigation is an existing supported shape at [RouteSheetMacTests.swift:26](/Users/admin/Developer/observe-lab/exact2/host/apple/tests/ExactKitTests/RouteSheetMacTests.swift:26). Today the consumer receives its launch-route identification explicitly through `navigation.launch` at [ObserveService.swift:78](/Users/admin/Developer/observe-lab/exact2/modules/observe/apple/service/ObserveService.swift:78).

   **Fix:** Specify a neutral selection fact on a screen’s `shown`: whether its navigation root selected it at that presentation. Observe can then choose the first qualifying screen in the launch session. Add the state-driven fullscreen case to the mapping tests. This needs one field, not another event type or tree walk.

2. **Minor — D1, App/Start: the new Mac-window rule conflicts with the pre-frame redirect rule.**

   > “In a later Mac window, that visit and the screens in place at its first frame start when the window was asked for…”

   > “A route that replaces another before the other's first frame is timed from its own cause.”

   A second window is requested at 0 ms, boots route A, redirects to B at 80 ms, and presents its first frame at 100 ms. The new sentence gives B a 100 ms shown duration; the redirect rule gives it 20 ms.

   **Evidence:** The conflicting rules are [RFC:73](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:73) and [RFC:130](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:130); [RFC:482](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:482) explicitly expects the redirect’s commit. Later windows boot independently at [ExactMac/main.swift:444](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactMac/main.swift:444), and document delivery can follow that boot at [main.swift:483](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactMac/main.swift:483).

   **Fix:** Limit inheritance of the window-opening origin to its initial routes. State that subsequent replacements use their own causes, including replacements before the first frame. A sentence suffices.

3. **Minor — D2, runtime readiness: “any commit” does not adopt pending served rows.**

   > “…a press on such a row, or any commit, adopts it first…”

   A commit that changes an unrelated readiness flag or counter does not necessarily run the list’s effect. Its deferred rows can remain unadopted. The quoted claim understates the documented limitation precisely when a separate commit makes a mark true.

   **Evidence:** `adoptAll()` is called from the list reconciliation effect at [rt.js:1219](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1219). Writes invalidate their observers at [rt.js:63](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:63), and flushing processes queued effects at [rt.js:79](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:79). The nearby comment at line 1247 repeats “any commit,” but the implementation does not establish it.

   **Fix:** Say that input targeting the row adopts it, and list reconciliation runs pending adoptions first. Retain the chosen readiness limitation; correcting this description requires no new adoption machinery.

4. **Minor — Tests, web: the unmount expectation omits D2’s content-frame requirement.**

   > “A screen that waits only for the input module becomes interactive when the last element needing it unmounts.”

   If the last custom pressable visibly disappears at 100 ms and that change presents at 116 ms, unmounting removes the input dependency at 100 ms, but D2 assigns interactive to 116 ms. The test currently reads as an expectation of 100 ms.

   **Evidence:** [RFC:522](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:522) states the expectation; [RFC:200](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:200) requires pending content changes to reach their frame.

   **Fix:** State that unmounting clears the input wait and a visible removal becomes interactive at its presentation frame. Alternatively, explicitly make the fixture a removal for which no frame is built. This is a test clarification, not another timing rule.

NOT READY