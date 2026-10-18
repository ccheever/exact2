# LLP 1132 r7 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. Three major gaps remain.

1. **Major — D1/D6: supported navigation without a router has no screen visits.**  
   D1 defines a route structurally by `navigationKey`, but starts its visit only when the **router** selects it. Contract also supports navigation driven directly by state: [RouteSheetMacTests.swift:25](/Users/admin/Developer/observe-lab/exact2/host/apple/Tests/ExactKitTests/RouteSheetMacTests.swift:25) uses `state screen` and `navigationKey=screen`, including modal routes. The web emitter explicitly supports navigation roots without a router ([emit.rs:562](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/emit.rs:562)); without a router, the runner produces no router change ([router.rs:667](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/router.rs:667)).

   Adding `interactive` to those screens satisfies D3, but supplies no defined visit to measure. Their nested parts inherit the same missing owner.

   **Fix:** start screen visits from the host’s selected/presented route changes, with router metadata when available. Observe can continue restricting its navigation metrics to router-driven changes.

2. **Major — D1/D2: app-owned parts can report interactive inside an unselected tab panel.**  
   Put `section id="summary" interactive=true` directly inside a `tabpanel`, beside its routes. Under D1 it belongs to the app visit because no route encloses it. With another tab selected and no modal open, the app visit is current, so D2 reports the part interactive.

   The platform actually hides and disables that entire panel: [navigation.js:297](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:297) sets its visibility and `inert`. This is platform-imposed inactivity, so D3’s exclusion of **authored** `inert` does not justify ignoring it.

   **Fix:** distinguish measurement ownership from presentation eligibility. At registration, retain the enclosing presentation/tabpanel context; update eligibility from host selection changes. This can use the proposed index without recurring tree walks or another subject kind.

3. **Major — D2: activation does not guarantee the web’s interaction executors are ready.**  
   A TypeScript app can satisfy all four conditions with `interactive=true` at its first frame while its required input code is still downloading. Non-native pressables request the deferred input module ([rt.js:914](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:914)); that module loads after a rendering opportunity ([rt.js:1146](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1146)). Enter/Space activation is installed only by that module ([input-glue.js:155](/Users/admin/Developer/observe-lab/exact2/host/web/input-glue.js:155)). Swipe handling similarly waits for the motion executor.

   These controls can therefore receive an early interactive time while required interactions do nothing. An input-module load failure merely logs an error, so data activation and runner viability also miss permanent failure.

   **Fix:** include required interaction-executor readiness in the host’s viability condition, using their existing completion signals. Keep unrelated resources and image loading outside this gate. Test a deliberately delayed or failed input-module load.

4. **Minor — D1/D7: the initial web screen’s paint handling is proposed, not “as built.”**  
   D1 says both the app and initial screen are shown at the earlier served-page paint, “as built,” and therefore a subsequent redirect is a navigation. The cited code preserves that earlier paint for **startup’s** `marks.present`. Screen history instead remains in `launching` until a later rendering opportunity ([marks.js:179](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:179)); redirects during that interval remain launch replacements. `presentFrom` does not update that screen history ([marks.js:225](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:225)).

   **Fix:** label this screen behavior as a change to implement, and use the earlier paint evidence before classifying launch replacements. The proposed served-page redirect test is appropriate. Also change the header’s stale “Draft r6” to r7.

5. **Minor — D4: waiting for both interactive and settled is unnecessary service gating.**  
   D4 loads services at the **later** of these times. A declared root can become interactive promptly while an unrelated stream never answers; D5 explicitly permits that stream to prevent settlement forever. Services then wait until 30 seconds despite the completed interactive measurement. A crash before service loading loses that measurement—the existing consumer documents this consequence ([README.md:31](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:31)).

   **Fix:** for a declared root, release services after its interactive time or cancellation, retaining the bound. Keep settled as an independent diagnostic. Removing the conjunction answers the owner’s “still loading but interactive” requirement more directly and eliminates one cross-metric coordination rule.

The five specific r6 corrections are coherently addressed in r7: presentation ancestry includes pushes inside sheets; part anchors include input-to-mount work; foreground visits are excluded from Observe navigation metrics; pre-frame failure cases are distinguished; and the web image-probe exception is documented.

Questions 4 and 5 now state their trade-offs fairly; neither should be repeated as an unresolved finding. For simplicity, I recommend Question 5’s per-mount alternative: it removes the literal-ID compiler refusal, duplicate membership/conjunction, and group reopening rules. Authors can mark a list container for one list measurement. The journal-volume cost is accurately acknowledged.

D6’s receipt and registration paths credibly cover invalid initial values, literals, renewal, viewless Apple elements, detached rows, and component declarations during eager adoption. Its costs still require measurement. D7’s Observe-policy inventory otherwise matches the cited code and provides a reasonable separation. The `interactive` naming exception is openly presented for Charlie’s decision; ordinary boolean typing fits Contract’s binding model.

External sources were not fetched under the local-only constraint.

**NOT READY.**