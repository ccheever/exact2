# LLP 1132 r5 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. r5 resolves activation gating, foregrounded dialogs, launch-screen chasing, and cold/warm history. The split-view question now states the missing measurements fairly; I would leave that decision to Charlie.

1. **Major — D1/D2: shell parts can report interactive beneath a modal route.**  
   Parts outside routes now have a stable app owner, but the app is “current” whenever no **dialog** is open. A `navigationPresentation="modal"` or fullscreen route therefore permits an underlying shell part to report while the platform blocks its input. [ModalIOS.swift:245](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:245) explicitly makes views outside that presentation inert. D3’s exclusion of *authored* `inert` does not cover this platform-imposed state.  
   **Fix:** preserve stable ownership, but gate each part on the host’s presentation eligibility, including modal routes. Test a pending shell part becoming true under a loading sheet and fullscreen route.

2. **Major — D3/D6: receipts lose declarations whose initial value is invalid.**  
   D3 permits unknown types and treats invalid runtime values as false. But [instance.rs:1048](/Users/admin/Developer/observe-lab/exact2/runner/src/instance.rs:1048) emits `ClearProp`; clearing an initially absent prop leaves no property and produces no corresponding touch ([txn.rs:734](/Users/admin/Developer/observe-lab/exact2/kernel/src/txn.rs:734)). D6 adds created nodes only when they *carry* `interactive`. Such a declaration is indistinguishable from no declaration: its part can disappear from measurement, and D4 can release services using the wrong policy.  
   **Fix:** register authored binding **presence** at mount from plan/site information, independently of its current property value. Use receipts for subsequent values, renewal and destruction. This needs no recurring tree walk.

3. **Major — D6/§R: the lazy-adoption fix states an outcome without supplying a registration path.**  
   The proposed emitter registration runs when element code executes. Deferred server-rendered rows explicitly skip their builder at [rt.js:1245](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1245); that builder executes later at [rt.js:1351](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1351). Their DOM is already attached. Saying registration happens “when it attaches” supplies neither the binding’s live effect nor its scope and cleanup. Waiting for adoption still produces late or missing measurements.  
   **Fix:** specify eager adoption for declaring rows, with its cost measured, or concrete rendered metadata plus an early binding-registration path. Include declarations inside nested components. The existing literal and renewal changes otherwise address their cited gaps.

4. **Major — D7: Observe cannot implement the root-versus-screen startup policy from these events.**  
   Suppose a screen reports interactive at 100 ms. With no root declaration, Observe should use it. With a root declaration still false, “from the root’s interactive time, or else” appears to require waiting. D7’s events expose no declaration-presence field, so those cases have identical histories until a later root event—possibly forever. The current consumer records startup metrics once ([service.js:90](/Users/admin/Developer/observe-lab/exact2/modules/observe/web/service.js:90)).  
   **Fix:** choose an explicit policy. If root declaration presence overrides screens, include `declared` in the app’s initial timing facts. Otherwise specify first eligible root-or-screen event wins. Keep that choice in Observe.

5. **Major — D1/D4: web startup “shown” has conflicting definitions.**  
   D1 uses a rendering opportunity **after commit** for every web visit; D4 says launch shown remains the app’s first frame “as built.” Today startup accepts a server-rendered page’s FCP **before** JavaScript’s commit ([marks.js:225](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:225)). Following D1 would inflate startup TTR by script download/adoption time. It could also discard an initial route that was visibly painted before a JavaScript redirect.  
   **Fix:** distinguish initial app presentation from subsequent rendering estimates. Preserve earlier paint evidence, clamped to page activation, and define how that evidence establishes the initial screen’s shown time. Test a rendered page painted substantially before boot and then redirected.

6. **Major — D1/D7: part events lack an explicit owning-visit reference.**  
   Every part measurement gets its own sequence identity, but its stated label is only its `id`; D7 does not carry the owner’s identity. An app-shell `status` part and a screen’s `status` part can coexist in one session, with matching starts and causes. Distinct sequence numbers distinguish records without telling a consumer which fragment each describes.  
   **Evidence:** D1’s identity/label definition, lines 90–98, and D7’s event fields, lines 220–223.  
   **Fix:** include the owning visit identity on every part event, with the relevant route/dialog context. This is necessary for usable independent fragment metrics.

7. **Minor — Question 5/D7: the literal-ID trade-off overstates event volume and understates complexity.**  
   The question says measuring each mount requires “two events per row per visit.” D7 gives parts no `shown` event, and D2 emits cancellation only while interactive is pending. A successful mount therefore needs **one terminal event**, containing its start and end. The cited 4,096-event capacities are correct; the asserted doubling is not required.  
   **Fix:** present the comparison accurately: per-mount events can exhaust the ring sooner, while grouping by literal ID conflates instances and adds membership, conjunction, last-member removal and reopening rules. Measuring mounts with IDs as labels would remove the literal-ID refusal and those aggregation rules. Authors wanting one list-level measurement can declare it on the containing element. These are concrete simplicity cuts that preserve the owner’s requested capabilities.

8. **Minor — D1/D2/Tests: pre-presentation cancellation still has contradictory outcomes.**  
   D1 says anything ending before its visit’s first frame leaves “no times and no `cancelled`.” D2 says a visit starting after poison or activation failure “is cancelled at once”; the test list explicitly expects that cancellation. A new screen in a poisoned runner satisfies both rules.  
   **Fix:** state precedence once. Either suppress all unshown measurements, including these failures, or explicitly exempt terminal runtime failures. Align the tests.

The new name is openly presented for approval, and removing the literal-`false` compiler exception improves consistency. The Observe separation is substantially cleaner; its remaining blocker is the missing policy input in finding 4.

**NOT READY.**