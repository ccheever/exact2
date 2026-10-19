# LLP 1132 r8 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. Three major gaps remain.

1. **Major — D1/§R: the new screen trigger misses replacements that retain the route key.**

   D1 starts a screen visit when the navigation root’s `navigationKey` names a different route. But `replace` preserves the entry ID while changing its URL and parameters. The existing test explicitly asserts this at [router.rs:148](/Users/admin/Developer/observe-lab/exact2/runner/tests/it/router.rs:148); the implementation preserves `old.id` at [router.rs:182](/Users/admin/Developer/observe-lab/exact2/route/src/router.rs:182).

   Existing timing deliberately recognizes `(entry, URL)` changes, including replacements with the same entry ([NavigationMarks.swift:407](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:407)). Under r8, replacing `/t/1` with `/t/2` can leave the old measurement running, omit the destination’s visit, or retain an already-recorded interactive time for different content. This also affects launch redirects implemented with `replace`.

   **Fix:** derive visit changes from effective route selection **and** router destination changes. A changed `(entry, URL)` must end and restart the screen measurement even when the selected element and key remain unchanged. Test a replacement both before and after the first frame.

2. **Major — D1: state-driven visits now start, but their predecessors have no matching end rule.**

   The r8 start rule supports navigation without a router. The end rule still says a screen ends “when the router puts another visit on top.”

   The supported fixture changes `state screen` directly ([RouteSheetMacTests.swift:32](/Users/admin/Developer/observe-lab/exact2/host/apple/Tests/ExactKitTests/RouteSheetMacTests.swift:32)), and without a router there is no router-change notification ([router.rs:667](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/router.rs:667)).

   Thus A → B starts B but never explicitly ends A or cancels A’s pending parts. Returning to A starts another visit while the original remains unresolved. Depending on implementation, that produces missing cancellations, stale measurements or duplicate interactive events.

   **Fix:** use the same effective selection change to end the previous screen and start its successor. Router metadata supplies labels and causes; it must not determine whether the previous visit ends. Extend the state-navigation test to assert cancellation and fresh identities across A → B → A.

3. **Major — D2/§R: input-load failure handles existing measurements only.**

   R8 says a failed input load “cancels pending times.” Unlike poisoning and activation failure, it does not define what happens to visits or parts created **after** that failure. D1 also omits input failure from its visit-ending conditions.

   This is reachable: the web retains the failed `Inputs` promise, prevents another load attempt, and only logs the failure ([rt.js:1140](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1140)). Native buttons and links can still navigate afterward. A newly declared screen then permanently fails D2’s readiness condition, but has no specified immediate failure outcome.

   **Fix:** retain the executor’s failed state and apply the same post-failure rule already specified for activation failure: a subsequent declared subject is shown and cancelled with `input_failed`. Define the treatment of parts mounted later in an existing visit. Test failure followed by navigation through a native control and by a later part mount.

4. **Minor — D1: the first-paint selection is still inaccurately described as built.**

   D1 describes “Element Timing … else First Contentful Paint.” The implementation instead installs both observers; whichever calls `take` first wins. It also has a rendering-opportunity fallback whose reporting waits 200 ms ([marks.js:231](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:231), [marks.js:242](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:242)).

   R8 correctly labels the initial **screen-history** change as proposed, resolving r7’s specific attribution error. Its remaining description of the app’s built paint-selection policy is nevertheless stronger than the cited implementation.

   **Fix:** describe the observer race and fallback accurately, or explicitly propose an ordered preference and its implementation change.

Questions 4 and 5 state their central trade-offs fairly. The split-view omission is explicit, including the missing list-pane measurement on direct detail launch; I would not repeat it as an unresolved finding.

For simplicity, choose Question 5’s **per-mount measurements with IDs as labels**. That removes the literal-ID placement error, duplicate membership and conjunction, last-member cancellation, and group-reopening rules. Authors can declare on the list container for one list measurement. The journal-volume concern is real, although “one event per ID per visit” assumes the group stays mounted: D1 permits reopening within that visit.

D6’s proposed receipt/index paths credibly cover clears, invalid initial values, renewal, viewless Apple elements, detached rows and component declarations during eager adoption. They support avoiding recurring tree walks; actual costs remain unproved until implementation and measurement. The benchmark’s approval requirement is appropriately acknowledged.

The declaration’s ordinary boolean typing fits Contract. The new name is openly presented as an exception for Charlie’s decision. D7’s inventory matches the cited Observe machinery, and its proposed mapping keeps Observe policy in the module. The default of no interactive time without a declaration preserves application-defined readiness.

External platform references were not fetched under the local-only constraint.

NOT READY.