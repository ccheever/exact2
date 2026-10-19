# LLP 1132 r3 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. r3 resolves several r2 issues: launch-session ownership, dialog syntax, declaration deltas, literal bindings, renewal intent, and removing per-screen settlement. The remaining problems are substantial.

1. **Major — D1/D3, “Not in this RFC”: router-top visits still disagree with usable UI.**  
   r3 excludes split views instead of resolving the eligibility finding. On a direct launch into detail, the usable list pane and its parts can receive no times. [`head.rs:109`](/Users/admin/Developer/observe-lab/exact2/runner/src/head.rs:109) explicitly preserves that pane as live; [`navigation.js:314`](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:314) likewise makes both routes active. Conversely, D3 ignores `hidden`, `inert`, and `display:none`, so a mounted, hidden part can receive **shown**, despite never appearing.  
   **Fix:** use the host’s presented and input-eligible subjects, including simultaneous panes. An author’s predicate defines readiness; it should not manufacture presentation.

2. **Major — D1/D2: retained parts lose their measurements permanently.**  
   D1 measures each part mount once, while a push or tab selection ends its visit. D2 cancels pending measurements when that visit ends. Consider a comments part mounted in A: push B before comments load, then Back to A. The retained part was cancelled and has no new mount, so it cannot report readiness on return. Backgrounding creates the same problem; newly mounted parts after foregrounding also inherit an ended visit. Retained tabs are explicit in [`navigation.js:297`](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:297).  
   **Fix:** distinguish mount identity from measurement occurrence. Start another occurrence for retained declared parts when their containing screen is presented again.

3. **Major — D1/D4: parts outside routes cease to work after startup.**  
   Such parts belong to the launch and are “not measured after the launch ends.” A route-free app therefore cannot measure a panel opened by a tap after startup, although D1 expressly defines that panel’s input-based cause. Persistent shell components have the same limitation. This does not answer the owner’s request for independently measured page fragments.  
   **Fix:** give these parts a session/page owner whose lifetime survives startup. Launch timing can use a separate starting anchor without owning their lifetime.

4. **Major — D7: event identity remains insufficient.**  
   The event’s “visit” contains a part’s optional authored `id`, yet that ID is expressly only a label. Two unnamed parts mounted together have indistinguishable events; duplicate IDs, renewal, reopening a dialog, and returning to the same stack entry repeat the problem. Duplicate authored IDs are legal: [`symbols.rs:271`](/Users/admin/Developer/observe-lab/exact2/contract/cli/tests/it/symbols.rs:271). Saying “a consumer correlates by visit” supplies no usable correlation key.  
   **Fix:** include session identity and an internal occurrence identity derived from runtime node/mount and visit/opening lifetime. Keep authored IDs as labels.

5. **Major — D6: the proposed web registration misclassifies detached rows.**  
   Registration occurs when the emitter creates an element, then finds its visit by following parent links once. But ordinary repeated rows are constructed inside a detached `DocumentFragment` at [`rt.js:1266`](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1266), and virtualized rows build inside detached wrappers at [`list.js:425`](/Users/admin/Developer/observe-lab/exact2/host/web-js/list.js:425). Neither chain reaches the owning route then. These declarations would acquire the launch owner or no owner.  
   **Fix:** carry the logical owner through emitter/runtime scopes, or resolve ownership after attachment at commit completion. Test fresh rows, nested components, adoption, and renewal.

6. **Major — D4: “launch end” has two incompatible meanings.**  
   “The launch ends … 30 s after launch end” uses the same term for both the lifecycle anchor and measurement completion. D7 additionally emits `launch.end`, while D4 journals raw “launch end.” The existing Apple deadline is anchored to `didFinishLaunching`, not measurement completion: [`Launch.swift:123`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:123). Also, “times reached after the launch ends are still journaled” conflicts with D1’s visit ending and D2’s “nothing is filled in later,” unless timeout releases services without ending measurement.  
   **Fix:** name the OS launch anchor separately. Define timeout as service release, or as terminal cancellation; specify exactly which measurements survive it.

7. **Major — D7: preserving today’s launch-route mapping restores the early-frame bug.**  
   D4 correctly separates the app’s first frame from a deferred launch sheet’s first frame. D7 nevertheless maps the launch route “as the module does today.” Today that mapping uses startup’s `present`, not the route’s presentation: [`launch.rs:794`](/Users/admin/Developer/observe-lab/exact2/modules/observe/linux/launch.rs:794). It also derives route TTI from startup TTI, which becomes wrong when the root declaration is true before that route is usable. Mapping every screen’s events as well risks duplicate rows.  
   **Fix:** map navigation metrics exclusively from the corresponding screen occurrence; use launch events exclusively for startup metrics.

8. **Major — D2/D4: activation failure is still not handled consistently.**  
   D2 forbids interactivity after activation failure, but acknowledges no web failure signal and assumes data-reading declarations therefore stay false. Baked answers disprove that assumption: [`rt.js:529`](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:529) preserves the baked value while activation waits, so `feed.loaded` can remain true after loading fails. On native hosts, activation failure blocks readiness but is absent from the cancellation/end reasons; existing Apple startup instead terminates immediately with `activation_failed` at [`Launch.swift:175`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:175).  
   **Fix:** propagate activation failure explicitly and give pending measurements a terminal failure outcome.

9. **Major — D7: moving frame policy lacks the required facts.**  
   D7 moves Observe’s slow/frozen thresholds and frame parameters into the module, but its event schema supplies no frame intervals or target periods. Today these are reduced inside framework-owned `FrameStats`: [`Launch.swift:594`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:594). Because the service loads after startup, it cannot reconstruct earlier frames by starting observation then.  
   **Fix:** specify an early module-owned collector or a bounded handoff of raw frame facts. Otherwise explicitly drop those parameters; retaining threshold-derived counters retains Observe policy.

10. **Minor — D6: constant work per change is impossible as stated.**  
    A visit becoming current can release thousands of waiting declarations, requiring thousands of evaluations/events. Rust hosts “read the set” also leaves full-set enumeration possible.  
    **Fix:** promise O(affected declarations), with indexed O(1) individual value updates, and benchmark visit changes and presentation fan-out separately. The delta approach removes tree walks, but does not establish the stronger complexity claim.

11. **Minor — D3/§R: the image idiom still cannot compile as written.**  
    `load=heroLoaded` requires an action, but the later readiness expression uses `heroLoaded` as a boolean. Handlers require actions ([`handlers.rs:51`](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/handlers.rs:51)); actions and states share one namespace ([`component.rs:60`](/Users/admin/Developer/observe-lab/exact2/contract/types/src/component.rs:60)).  
    **Fix:** show `onHeroLoad` and `onHeroError` actions updating distinct boolean states.

12. **Minor — D5/D7: accuracy corrections remain.**  
    D5’s “first presented frame” definition is not the built settlement rule: Apple can mark immediately when unchanged ([`Launch.swift:346`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:346)), as can Linux ([`launch_marks.rs:349`](/Users/admin/Developer/observe-lab/exact2/host/linux/src/launch_marks.rs:349)). The launch formula cited at `Launch.swift:564–567` is actually at line 568. The supposedly complete Observe-name inventory misses the web’s `exact: observe marks` output at [`marks.js:45`](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:45).  
    **Fix:** correct the definition, citation, and rename inventory.

NOT READY