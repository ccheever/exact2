# LLP 1132 r4 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

No critical findings. r4 fixes retained-part revisits, occurrence identities, terminal activation failure, detached-fragment ownership, the image example, and launch/service terminology. The remaining findings are below.

1. **Major — D2: successful activation is missing from the readiness conditions.**  
   The summary requires a “viable” runtime, but D2 checks only currentness, transition completion and the declaration. A baked `feed.loaded=true`, or literal `interactive=true`, can therefore report before activation completes—and before a subsequent activation failure. Apple deliberately activates after drawing ([Session.swift:1251](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Session.swift:1251)); the web preserves baked answers while its source is unavailable ([rt.js:529](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:529)).  
   **Fix:** explicitly require successful activation. Activation completion must trigger evaluation independently of declaration changes.

2. **Major — D1: foregrounding loses still-open dialogs.**  
   Backgrounding ends every visit. Foregrounding starts app and screen visits only. A pending dialog remains open but gets no replacement visit, so neither its declaration nor its retained parts can report after return. The existing background/foreground hooks do not close dialogs ([Launch.swift:426](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:426)); dialog openness is retained separately ([DialogSheetIOS.swift:145](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/DialogSheetIOS.swift:145)).  
   **Fix:** restart occurrences for still-presented dialogs on foregrounding, with eligibility determined by their presentation order.

3. **Major — D1/D2: parts outside routes still have inconsistent ownership.**  
   A persistent shell part mounted during screen A belongs to A’s visit. Navigation cancels it, but the rule never assigns that still-mounted element to B; “the next visit of its element” has no defined meaning outside a route. Mounting while a dialog pauses every screen supplies no *current* screen owner. Conversely, in a route-free app the owner is the always-current app visit, allowing a covered part to report beneath a modal. This leaves §R’s outside-route fix incomplete.  
   **Evidence:** D1’s ownership sentence and “app visit, always,” versus the host’s actual modal input exclusion ([ModalIOS.swift:245](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:245)).  
   **Fix:** give shell parts a stable session/root owner and independently gate their measurement on presentation eligibility.

4. **Major — D1/D3: duplicate-ID measurements have no boolean semantics.**  
   Two component instances can declare the same literal ID with different values. They now “count once,” but which declaration controls the result: first, any, or all? Removal and renewal make that choice observable. Duplicate IDs are explicitly accepted ([symbols.rs:271](/Users/admin/Developer/observe-lab/exact2/contract/cli/tests/it/symbols.rs:271)). Moreover, unmounting and reopening a part within one screen visit permanently consumes its one measurement.  
   **Fix:** use runtime mount plus visit identity, with IDs as labels. That removes the literal-ID restriction, duplicate aggregation and special “last element” cancellation rule. If aggregation is retained, its truth and reopening semantics must be defined.

5. **Major — D1, “Not in this RFC”: split views still produce missing times.**  
   On direct launch into detail, the list pane can remain visible and usable indefinitely, yet its declaration and parts cannot report because they are never router-top. Deferring this explicitly does not resolve r3’s finding. The existing eligibility logic already recognizes the live pane ([head.rs:109](/Users/admin/Developer/observe-lab/exact2/runner/src/head.rs:109)); the web activates both routes ([navigation.js:314](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:314)).  
   **Fix:** derive currentness from the host’s presented, input-eligible routes, permitting simultaneous screen visits.

6. **Major — D4/§R: launch-screen selection depends circularly on service release.**  
   The launch screen remains replaceable by no-input navigation until `launch.services`, which itself depends on that screen’s interactive time. Suppose A becomes interactive while an unrelated request keeps settlement pending; a later timer redirect to B still qualifies B as the launch screen. Does that replace A’s already-reached first passage? If A was pending instead, D1 cancels its visit on navigation while D4 says only input navigation cancels the launch fallback. These rules do not yield one answer.  
   **Evidence:** D1’s unconditional screen end; D4’s “before `launch.services`,” cancellation exceptions and later-of formula.  
   **Fix:** remove launch-screen chasing from the framework. Journal root and screen occurrences; let Observe choose its first-marker startup policy. Alternatively, freeze ownership at a specified presentation boundary independent of service loading.

7. **Major — D6: recording `SetProp` emissions is insufficient to maintain the index.**  
   Invalid computed values emit `ClearProp`, not `SetProp` ([instance.rs:1048](/Users/admin/Developer/observe-lab/exact2/runner/src/instance.rs:1048)). The compiler accepts `Ty::Unknown` ([values.rs:1223](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/values.rs:1223)), so this path remains reachable despite D3’s blanket boolean claim. Renewal can also retain unchanged bindings without another emission ([reuse.rs:325](/Users/admin/Developer/observe-lab/exact2/runner/src/instance/collection/reuse.rs:325)). An emission-time index additionally needs transaction rollback discipline ([commit.rs:112](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/commit.rs:112)).  
   **Fix:** specify updates from successful commits, including clears, descendant destruction and renewal with retained values. Existing receipt fields can support this without recurring tree scans; the proposed hook alone does not establish that guarantee.

8. **Major — D6: deferred web adoption still gives late part measurements.**  
   Resolving ownership after attachment fixes fresh detached rows. It does not fix server-rendered rows whose emitter code has not run: ordinary rows can enter `lazy` without invoking their row builder ([rt.js:1245](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1245)). Registration happens only when later adoption invokes that builder ([rt.js:1351](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1351)). A visible, ready part thus starts at adoption rather than presentation.  
   **Fix:** eagerly register/adopt timing bindings for declaring rows, or carry their timing metadata through the adoption path. Test a rendered list that exceeds the adoption budget.

9. **Minor — D7: Observe’s cold/warm mapping names the wrong history.**  
   “History of `router` events” includes destinations superseded before presentation. Those must remain cold. Existing `ScreenHistory` marks a screen shown separately ([NavigationMarks.swift:418](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:418)).  
   **Fix:** update shown history from screen `shown` events; use router removals for eviction. Keep both decisions in Observe.

10. **Minor — D7: the corrected Apple timing-gate claim remains too broad.**  
    `Launch.swift` disables startup timing under the agent and for suppressed launches. Navigation timing is dispatched independently, without that agent guard ([Session.swift:1063](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Session.swift:1063)).  
    **Fix:** describe startup and navigation gates separately. Preserve or explicitly relocate prewarm/background-launch suppression; “every app except under the agent” is inaccurate.

11. **Minor — D1/D2: pre-presentation termination has conflicting outcomes.**  
    D1 says a replaced or closed visit before its first frame “leaves nothing”; D2 journals cancellation whenever a pending visit ends. A pre-frame replacement also discards the original input start, timing the destination from its replacement commit.  
    **Fix:** define one pre-presentation outcome rule and specify whether an unseen replacement inherits the original start. Test timestamps and events, not merely that only the destination survives.

12. **Minor — D3/D4/D7: further simplification is available.**  
    Refusing literal `interactive=false` adds a compiler exception without resolving the name’s ambiguity; a bound expression permanently returning false is equivalent. The public `launch.services` event has no identified metrics consumer. The web’s new 30-second bound also lacks a defined launch-end anchor.  
    **Fix:** remove the literal-false error; keep service release internal unless a consumer needs its event; anchor the web bound explicitly to activation. Together with removing duplicate-ID aggregation and launch-screen chasing, these cuts preserve the owner’s requested measurements while reducing mechanisms.

**NOT READY.**