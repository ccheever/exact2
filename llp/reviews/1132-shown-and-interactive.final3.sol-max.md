# LLP 1132 — third adversarial review by Sol (gpt-6.1-sol, max), of b867d7a617, 2026-10-17

No critical findings. Reviewed `b867d7a617` and the cited repository code locally. External URLs were not fetched.

1. **Major — D1 App / D4 / D7: a launch interrupted before its first frame has no defined identity.**

   [D1:62](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:62) says the app visit starts “at the first frame.” [D4:208](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:208) defines the launch as the first app visit, but also requires backgrounding to cancel it before interactivity.

   Consider an app that backgrounds before its first frame, then returns ten seconds later. No initial app visit has started under D1. The foreground visit is consequently the session’s **first** app visit—and therefore the launch under D4. D7’s instruction to obtain startup TTR from the first app visit’s `shown` can then report the return frame as startup, including the background interval in Observe’s duration.

   Today Apple retains an interrupted startup outcome even without a presented frame: [Launch.swift:426](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:426).

   **Fix:** establish the pending initial app visit when the launch session starts; distinguish its existence from its `shown` mark. If it ends unshown, it remains the launch attempt, publishes no visit events, and cannot be replaced by a foreground visit. This needs clarification of the existing lifecycle, not another event.

2. **Major — D1 Cause/Sections / D6: the proposed mark handoff lacks the information needed to assign correct starts.**

   D1 requires a section’s mounting cause to distinguish an input’s own commit from an unrelated program commit. [D6:257](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:257), however, specifies only mounts, values and removals in Apple’s batch. The cited kernel `CommitReceipt` carries neither commit time nor input origin.

   This distinction cannot safely be reconstructed from the input currently being dispatched. [Runner::dispatch_at](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/advance.rs:38) first runs overdue timers, then the event’s action. If such a timer mounts a section and the input handler changes nothing, both run inside the host’s input scope. Assigning that scope’s timestamp incorrectly labels the timer-mounted section `input` and includes unrelated input delay in its duration.

   Navigation already handles this through `RouterChange::from_event`; Linux explicitly filters the current input using it at [navigation_marks.rs:57](/Users/admin/Developer/observe-lab/exact2/host/linux/src/navigation_marks.rs:57).

   **Fix:** specify that committed mark notifications retain their commit timestamp and dispatch origin, including when several commits share a native batch. Reuse the existing input-origin distinction. Add the timer-before-input ordering to the section timing test.

3. **Minor — D2: the merged hidden-visit sentence still has two timestamp readings.**

   [D2:144](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:144) selects “the moment the visit is hidden,” while requiring the expression to be true at that moment.

   Suppose shown screen A becomes hidden at 10 ms while its expression is false. At 20 ms its text changes and its expression becomes true. “The moment the visit is hidden” can mean the hiding transition at 10 ms, which fails the expression requirement, or the readiness change at 20 ms while it is hidden. The “nothing on screen changed” alternative resolves this only if readers infer that hidden content changes are excluded.

   **Fix:** say explicitly that an already hidden, previously shown visit records when readiness holds; becoming hidden while awaiting presentation records at the hiding transition. These can remain one sentence and use the same existing evaluation mechanism.

4. **Minor — D3: child-local state does not necessarily make a mark a section.**

   [D3:190](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:190) says a mark reading a child component’s state “is then a section.”

   A child component can instead render the route element itself as a direct child of the navigation root. Component expansion substitutes the child’s view before lowering, as [lower/src/lib.rs:7](/Users/admin/Developer/observe-lab/exact2/contract/lower/src/lib.rs:7) explains. Its route-root mark therefore marks a **screen visit** under D1, regardless of where its state was declared.

   **Fix:** replace the last clause with: “The mark uses the same visit-or-section classification after component expansion.” No implementation change is needed.

5. **Minor — D3/D6: carry the settled `for` refusal through the example and remove unreachable bookkeeping.**

   The screen example at [D3:172](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:172) uses `${e.id}`, matching the repository’s normal loop-generated route pattern. The navigation recipe places those route elements inside `for e in stack(nav)` at [navigation-stack.md:56](/Users/admin/Developer/observe-lab/exact2/docs/recipes/navigation-stack.md:56); Shelf does likewise at [app.contract:198](/Users/admin/Developer/observe-lab/exact2/apps/shelf/app.contract:198).

   Under the settled refusal, those screen elements—and components beneath them—cannot carry marks. This is a material authoring limitation that the example currently obscures.

   Meanwhile, [D6:254](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:254) still requires special handling for renewed marked nodes. The producer renews recycled collection rows and their descendants at [reuse.rs:342](/Users/admin/Developer/observe-lab/exact2/runner/src/instance/collection/reuse.rs:342), precisely where marks are now forbidden.

   **Fix:** show an unambiguously legal, statically declared screen example and briefly acknowledge that loop-produced router screens cannot presently be marked. Keep the compiler error unchanged. Remove the mark-specific renewal requirement and recycled-row example until that restriction changes.

NOT READY