# LLP 1132 — consensus check of d3095d8654 by Sol (gpt-6.1-sol, max), 2026-10-18

No critical findings.

1. **Major — A never-shown covered visit still waits for the cover to close.**

   D1 says **“a cover never delays its times”**, but D2 requires interactive to occur **“at or after … the visit’s shown time.”** Native shown rules still require a frame carrying the route, UIKit appearance, or a page flip showing it. ([D1](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:56), [Shown](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:105), [D2](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:174))

   Consider an initial stack containing base route A and selected fullscreen sheet B. A is in place as its presentation’s top route. Its expression and runtime become ready while B remains open. Linux hides A; it cannot get the required page flip showing A until B closes. The synthetic foreground shown rule applies only to a route previously shown, so it does not resolve this case. The new interactive frame rule cannot bypass its shown prerequisite.

   **Fix:** Define the initial shown milestone for an in-place covered route independently of uncovering, consistent with the existing foreground exception. Release pending marks at that milestone while preserving `interactive ≥ shown`. Add tests for an initially covered route and a cover arriving before the route’s first frame.

2. **Major — The visit definition still makes a held UIKit back swipe delay measurement.**

   D1 says the stack **“extends up to the route that the root selects”**, and only its presentation’s top route is in place. D2 nevertheless promises that **“a back swipe that the user holds does not delay the screen it reveals.”** ([Stack](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:47), [D2](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:186))

   During a native pop from B to A, UIKit begins revealing A before Contract’s selected route changes. The source records the transition beginning in `willShow`, but dispatches `goBack()` only after the transition finishes. Under D1’s logical stack rule, A therefore has no new visit during the hold. Removing the entrance condition cannot make that nonexistent visit interactive. Backdating its start after completion also cannot reconstruct readiness changes during the hold. ([Transition begins](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/iOS/NavigationIOS.swift:946), [Router dispatch](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/iOS/NavigationIOS.swift:1079))

   **Fix:** Explicitly define when UIKit’s destination comes into place before the router commit, evaluate its marks during that interval, and specify what a cancelled swipe does to those visits. Apply this to D1, D6 and “What changes.” Test readiness both before and during a held swipe, including cancellation.

3. **Major — The no-frame fallback depends on an unspecified future.**

   D2 chooses **“the first frame after the change”**, but uses readiness time for **“a host that presents no frame after a change it does not draw.”** ([Frame rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:197))

   A covered visit becomes ready at 100 ms while the host schedules no drawing. If the user later causes an unrelated frame at 500 ms, that frame is still the first frame *after* the change. If no later frame occurs, the fallback selects 100 ms. The text supplies no point at which the host can choose the fallback without waiting for future input.

   The hosts have usable presentation mechanisms: Apple’s global refresh estimate, the web’s rendering opportunity, and Linux’s submitted-frame flip timestamps. Linux, however, produces frames only when dirty; an automatic next flip is not guaranteed. ([Apple](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:644), [Web](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:61), [Linux](/Users/admin/Developer/observe-lab/exact2/host/linux/src/display.rs:423))

   **Fix:** Define the fallback through an observable scheduling decision for the change, including how an already pending unrelated frame counts. Later input must not revise that decision. Test a covered change with a pending frame, with no scheduled frame, and with a later input that schedules one.

4. **Minor — The revised covered-route test requires a frame unconditionally.**

   The test says **“A change that makes its expression true gives it … the first frame after the change.”** D2 instead selects readiness time when the visit’s visual content does not change, and also supplies the no-frame fallback. ([Test](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:465), [Rule](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:200))

   **Fix:** State that this test changes visual content and has a scheduled frame. Add separate assertions for an expression-only change and the clarified no-frame case. Replace “the time it would have uncovered” with “the same time as an uncovered visit.”

NOT READY