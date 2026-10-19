# LLP 1132 r6 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. r6 supplies concrete fixes for declaration presence, eager adoption, owner identities, and the root-policy input. Questions 4 and 5 now state the split-view and literal-ID trade-offs fairly.

1. **Major — D1/§R: the sheet-pause fix misses pushes inside a sheet.**  
   The app visit is current when “no sheet route is on top.” But a plain pushed route can become router-top while remaining inside a modal presentation. A pending shell part could then report interactive while its input remains blocked.

   **Evidence:** [NavigationRules.swift:39](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationRules.swift:39) explicitly keeps later pushes inside the latest presentation’s stack; [ModalIOS.swift:248](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:248) checks the actual presentation boundary. The existing fixture exercises “Detail in the sheet” at [app.contract:668](/Users/admin/Developer/observe-lab/exact2/apps/native-fixture/app.contract:668).

   **Fix:** gate app-owned parts on whether a modal presentation covers them, rather than the top route’s presentation attribute. Test a shell declaration becoming true after a push inside a sheet.

2. **Major — D1/Tests: a part’s start excludes the input-to-mount work.**  
   D1 starts a part at “the later of the visit’s start and the first mount.” The test instead requires “a part mounted by a tap starts at the tap.” These differ whenever the handler does work before mounting the fragment: recording the input as its *cause* does not restore the omitted duration.

   **Evidence:** [RFC:89](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:89) versus [RFC:279](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:279).

   **Fix:** start at the later of the owner visit’s start and the mounting operation’s causal input timestamp, falling back to its commit timestamp. Distinguish that measurement anchor from physical mount time.

3. **Major — D7: foreground visits become spurious Observe navigation metrics.**  
   D1 creates a new screen visit on every foreground return. D7 emits navigation TTR from **each** screen visit’s `shown`. Consequently, hiding and restoring an unchanged app produces `warm_ttr`, despite no navigation occurring. It can also produce navigation TTI for a screen that first becomes ready after return.

   **Evidence:** [RFC:62](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:62), [RFC:251](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:251). The built foreground path only updates shown history; it emits no navigation measurement ([NavigationMarks.swift:132](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:132)). The consumer documents navigation as a router change ([README.md:39](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:39)).

   **Fix:** let Observe distinguish navigation visits from foreground visits using the generic cause/router facts. Map navigation metrics only for navigations and the explicitly specified launch-screen case.

4. **Minor — D1/D2/§R: pre-frame failure still has two outcomes.**  
   Consider a visit created, then poisoned, then first presented. D1 ends it at poisoning and says anything ending before its first frame leaves “no times and no `cancelled`.” D2 says a declared visit shown after poisoning is cancelled at shown. Delaying the cancellation until presentation does not reconcile those outcomes.

   **Evidence:** [RFC:75](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:75), [RFC:77](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:77), [RFC:110](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:110).

   **Fix:** distinguish a visit that **ended before presentation** from one **created after an existing terminal failure**. Suppress the former; explicitly define the latter’s cancellation. Test both sequences.

5. **Minor — D5: “images outside a canvas” is not an accurate built exclusion.**  
   On the JS target, a cross-origin tinted image with image-event handlers can start a CORS probe. That probe increments `inflight.n`, and `outstanding()` counts it as loading. Such an image can therefore delay the built settled time.

   **Evidence:** [rt.js:892](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:892), [marks.js:56](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:56).

   **Fix:** document this existing exception, or exclude image probes from settlement accounting if the intended rule is unconditional.

For simplicity, I would choose Question 5’s per-mount alternative. It removes the literal-ID compiler refusal, duplicate membership and conjunction rules, last-member cancellation, and group-reopening semantics. Authors can declare on a list container when they want one list measurement. r6 now fairly acknowledges the journal-volume cost; that choice is for Charlie, not a repeated unresolved finding.

D6’s proposed indexing paths address the previously identified missed values and mounts, including renewed nodes and viewless Apple elements. Its no-recurring-tree-walk design is credible; eager adoption’s actual cost still needs measurement. The `interactive` naming exception is openly presented for approval, and the proposed framework events otherwise separate Observe policy appropriately.

External references were not fetched under the local-only constraint.

**NOT READY.**