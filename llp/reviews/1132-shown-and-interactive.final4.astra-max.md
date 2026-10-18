# LLP 1132 — fourth final review by Astra (gpt-6-astra, max), of 7704e29904, 2026-10-18

Reviewed **7704e29904**. No critical findings.

Third-review dispositions: #1’s missing selection fact is supplied; #2’s window-origin conflict and #4’s removal-frame conflict are resolved. #3’s reconciliation claim is corrected, but its replacement overstates input-triggered adoption (finding 4 below). The new test for #1 also conflicts with UIKit’s presentation sequence (finding 2).

1. **Major — D1, “Dialog”: declaratively opened dialogs have no defined visit.**

   > “A dialog visit begins when a `dialog` opens with `showModal(id)` or `commandfor=id command="show-modal"`.”

   A tap sets `confirm=true`, opening a supported native alert written as `dialog open=confirm markinteractive=true`. Neither named command occurs. Under this definition, the alert receives no visit, hence no shown or interactive time. Sections inside it also lack an owner visit.

   **Evidence:** [`schema.json:614`](/Users/admin/Developer/observe-lab/exact2/kernel/tables/schema.json:614) explicitly supports state-held `dialog open`. The shared UIKit host detects `open == true` at [`MenusIOS.swift:178`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:178) and presents it modally without an invoker at [`MenusIOS.swift:866`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:866). This is an existing opening path omitted by [RFC:71](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:71).

   **Fix:** Define the visit by the host accepting the dialog’s opening, including declarative `open`, rather than enumerating two commands. Apply the existing cause rules to state-driven openings and initially open dialogs. One definition sentence and a declarative-opening case suffice; no additional event type is needed.

2. **Major — D1 “Shown”, D2, D4 and Tests: presentation queues still let animations delay interactive.**

   > “Interactive … at or after both the visit’s shown time and the measurement’s own start.”

   > “On iOS … the first frame after UIKit begins to show the route’s view controller…”

   Suppose one commit puts sheets B and C above A, with C selected, marked true and otherwise ready. UIKit presents B first. C’s presentation waits for B’s animation to finish, so C’s `routeAppeared`, shown time and therefore interactive time all wait. C has no later cover, so D1’s covered-screen exception does not apply. The time includes another presentation’s animation despite the owner’s rule that animations never delay interactive.

   **Evidence:** [`NavigationIOS.swift:481`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:481) defers synchronization during transitions; [`NavigationIOS.swift:513`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:513) installs one additional presentation per synchronization. The presentation completion clears the hold and resumes synchronization at [`ModalIOS.swift:521`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:521). Appearance becomes true only in `viewIsAppearing` at [`NavigationIOS.swift:64`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:64). These facts conflict with the combination of [RFC:101](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:101), [RFC:172](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:172) and the settled decision at [RFC:548](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:548).

   The newly added launch test exposes a related contradiction:

   > “Both are shown at the first frame…”

   [RFC:483](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:483) requires this for the base and fullscreen route, but [`NavigationIOS.swift:480`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:480) deliberately draws initial content before presenting its cover. D4 explicitly requires the launch screen’s `routeAppeared` boundary.

   **Fix:** Extend the existing hypothetical-frame convention for covered visits to in-place routes whose presentation is queued solely by an ongoing native transition. Keep native presentation sequencing intact. Add one stacked-presentation case. Remove the universal same-frame assertion from the selection test: test which screen’s `shown` reports selection, with each screen using its specified timing boundary. This reuses an existing timing convention instead of changing UIKit behavior to satisfy telemetry.

3. **Minor — D1, “Sections”: “selected” is undefined for ordinary authored tab panels.**

   > “Inside a `role="tabpanel"`, Exact measures a section only while every enclosing panel is selected.”

   A marked section inside an ordinary authored `tabpanel` within an active route has no defined selection fact. Reading this literally can suppress its time forever or require a new mechanism for discovering arbitrary ARIA tab relationships.

   **Evidence:** The cited implementation does not manage every `tabpanel`. [`navigation.js:13`](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:13) finds panels named by a navigation root’s tablist and excludes relationships inside another route. [`navigation.js:298`](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:298) applies selection, hiding and inertness only to that managed set. [RFC:140](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:140) broadens this to every enclosing panel without defining the additional semantics.

   **Fix:** Limit this paragraph to panels managed by the navigation root. Other authored panels use the app’s mark expression. This merits a scope clarification, not a general ARIA selection mechanism.

4. **Minor — D2, runtime readiness: the revised adoption sentence still overclaims.**

   > “…input aimed at such a row adopts it first…”

   Hovering a waiting row does not adopt it first. The replacement therefore exchanges the incorrect “any commit” guarantee for an overly broad input guarantee.

   **Evidence:** [`rt.js:1340`](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1340) enumerates the adoption events; it includes presses, keydown, editing and focus, but excludes `pointerenter` and `pointerleave`. Those are the events used by hover handlers at [`rt.js:924`](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:924).

   **Fix:** Replace the phrase with “a press, keydown, edit or focus entering such a row adopts it first,” or simply restore the narrower “a press on such a row.” This is wording only; expanding adoption behavior would add machinery without helping the RFC.

NOT READY