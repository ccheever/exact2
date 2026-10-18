# LLP 1132 — final adversarial review by Sol (gpt-6.1-sol, max), 2026-10-17

No critical findings.

1. **Major — D1 End / D4 Services: orderly shutdown has no specified outcome.**

   D1’s end conditions omit session destruction and termination; D4’s service triggers omit quitting. A Mac app with `markinteractive=false` that quits two seconds after its first frame therefore reaches neither the service deadline nor another listed terminal outcome. Applying these rules loses measurements that today’s implementation persists: [Launch.swift:447](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:447) finishes startup as `terminated`, loads services synchronously, and flushes. Closing one window also destroys its session independently of process backgrounding ([Session.swift:1475](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Session.swift:1475)).

   **Fix:** define session destruction and orderly termination as visit endings, cancel pending measurements, and release and flush services before process exit. Specify that closing one window ends only that window’s session.

2. **Major — D1 End / D2: the presentation rule can reintroduce pausing beneath fullscreen covers.**

   D1 expressly permits interactivity “even though the host … hides” the underlying page. D2 instead requires “the presentation of the frame that shows the change.”

   Concrete case: A was shown, fullscreen B covers it, then an answer changes A’s text and makes A’s mark true. The web makes A `visibility: hidden` ([navigation.js:313](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:313)); Linux likewise hides the predecessor under fullscreen presentation ([navigation.rs:53](/Users/admin/Developer/observe-lab/exact2/host/linux/src/navigation.rs:53)). No frame visibly shows A’s change until B closes. Following D2 delays A’s interactive time, contrary to D1.

   **Fix:** distinguish the initial presentation prerequisite from subsequent update timing. Explicitly say how applied updates become eligible while a previously shown visit is covered, without waiting for it to become visible again. Extend the covered-screen test to change its pixels as well as its mark.

3. **Major — D1 Sections / D2: tab-gated sections include time spent waiting for the user to select them.**

   A section outside routes but inside an initially unselected tab panel belongs to the app. Under [D1’s start formula](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:106), a section mounted at launch starts then. If its expression is already true and the user selects its tab ten minutes later, D2 records approximately ten minutes of interactive time.

   The panel was deliberately hidden and inert throughout those ten minutes ([navigation.js:297](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:297)). This is user absence, not the section becoming ready. The existing test checks that an event eventually exists, but does not catch its incorrect duration.

   **Fix:** start the section’s measurement when its enclosing panels first become selected if that occurs after mounting and owner-visit start. Define what deselection does to a pending measurement, and assert the duration in the tab-selection test.

4. **Major — D2 Runtime readiness: the native-button exemption is false, and the dependency inventory is incomplete.**

   “A screen of native buttons waits for neither” conflicts with existing keyboard shortcuts. A native `button` carrying `aria-keyshortcuts` requires the deferred input module: both static and dynamic attributes call `input()` ([rt.js:616](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:616), [rt.js:688](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:688)). A literal-true screen could therefore report interactive while its declared shortcut still does nothing.

   Pans also require input independently of custom pressables; height and transform drags and reorder handles require motion independently of swipes ([rt.js:1031](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1031), [rt.js:1050](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1050)).

   **Fix:** derive the required executors from the measured subtree’s actual event binders and capabilities. Replace the blanket native-button exemption. Test a delayed or failed input load with a shortcut-bearing native button and a pan-only section.

5. **Major — D1 Shown / D2 Entrance: macOS sheet timing has two different endpoints.**

   “First presented frame” and “the first frame after `beginSheet`” are not equivalent under the behavior the RFC cites. [SheetsMac.swift:32](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:32) records that `beginSheet` returns after the approximately quarter-second entrance animation; the call is at [line 228](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:228).

   If “after” means after the call returns, a sheet whose first visible frame arrives near 16 ms gets a shown mark around 250 ms or later. That folds the entrance into shown, while D2 separately waits for entrance completion.

   **Fix:** explicitly anchor shown to the start of presentation and observe its first frame independently of the call’s return. Use return/arrival only for entrance completion. Add a macOS assertion that shown precedes entrance completion for an animated sheet.

6. **Minor — D1 Shown: the web fallback’s 200 ms description does not match the cited code.**

   The RFC says “a rendering opportunity taken 200 ms later is the fallback.” [marks.js:249](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:249) captures the next opportunity immediately, then waits 200 ms before using that captured timestamp. Implementing the sentence literally adds approximately 200 ms to fallback TTR.

   **Fix:** write: “The fallback captures the next rendering opportunity and uses its timestamp if neither observer has supplied one after 200 ms.”

7. **Minor — Tests: “Each host gets these tests” requires unsupported presentations.**

   The unconditional test list includes an interactive alert and a live split view. Yet D1 correctly says Linux DRM/headless refuses dialogs, and the cited Linux route projection has no split-pane behavior. [commands.rs:99](/Users/admin/Developer/observe-lab/exact2/host/linux/src/presenter/commands.rs:99) explicitly refuses `showModal` without windows.

   **Fix:** distinguish shared timing-state tests from presentation tests on supporting hosts. Linux should verify the refusal and absence of a dialog visit; this RFC should not accidentally require implementing alerts or split views there.

8. **Minor — D3 / D6: the required compiler warning is removable machinery.**

   D3 mandates a `for`-row warning even though independent per-copy measurements are settled semantics, and journal-volume guidance already explains the consequence. The warning adds compiler behavior and maintenance without changing any requested metric.

   **Fix:** retain the guidance to mark the list container for one measurement, and remove the mandatory warning and its acceptance assertion. Keep performance verification focused on the implementation’s actual costs.

9. **Minor — Decisions / Status: the document retains conversational history that obscures its operative rules.**

   “Let’s go with B” has no standalone meaning. The rejected terms and six naming alternatives recount a discussion already preserved in the review files. “Everything else … proposes” is also awkward status language. These conflict with CLAUDE.md’s instruction to avoid history and make the reader understand on first reading.

   **Fix:** remove the rejected-name list and conversational quotations. Keep the settled rules in D1–D7 and one link to their rationale. State plainly: “The loading ledger is implemented; the remaining behavior and ledger changes are proposed.”

NOT READY