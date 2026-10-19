# LLP 1132 — final review by Astra (gpt-6-astra, max), of 9392b64a2f, 2026-10-18

Reviewed `9392b64a2f` against the local source. No critical findings.

1. **Major — D2: the last readiness change can bypass content awaiting presentation.**

   > “the interactive time depends on whether the change that made them hold altered the visit’s content”

   Consider an already-shown web screen. At 120 ms, a resource updates its content and makes its expression true; input support is still loading. At 121 ms, input support finishes without changing content. The updated content’s frame is at 133 ms. D2 gives **121 ms**. Had input support finished slightly earlier, D2 would give **133 ms**. Slower initialization produces an earlier interactive time.

   The adjacent instruction to use a frame “the host already had pending” also permits a Linux frame built **before** the content change. That frame cannot show the change.

   **Evidence:** [marks.js:109](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:109) retains dirty state and checks frame generation. [display.rs:423](/Users/admin/Developer/observe-lab/exact2/host/linux/src/display.rs:423) prevents submitting another frame while a flip is pending; [launch_marks.rs:123](/Users/admin/Developer/observe-lab/exact2/host/linux/src/launch_marks.rs:123) checks which tree the presented frame contains.

   **Fix:** Make presentation eligibility depend on outstanding content changes, rather than only the last readiness trigger. A readiness-only change must preserve an existing frame wait; a visible change must use a frame built after it. Retain the specified exception for covered changes that schedule no frame. Existing dirty/generation tracking supplies the mechanism. Add the two ordering cases above to the timing tests.

2. **Major — D1/D7: dismissing a sheet loses the return navigation’s timing.**

   > “Navigation metrics come only from navigations: screen visits that a router change brings into place”

   > “`cold_ttr` and `warm_ttr` come from each such visit’s `shown`”

   Screen A is shown, sheet B opens, then the user dismisses B. D1 correctly keeps A’s visit throughout. Consequently, the dismissal creates no A visit and no new A `shown`. D7 either emits no `warm_ttr`, or reuses A’s old shown time and origin. Neither measures the return navigation.

   This also occurs when popping back to a split view’s first route that remained in place.

   **Evidence:** [NavigationMarks.swift:205](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:205) currently recognizes selection changes independently of visit continuity. [ScreenHistory.change:407](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:407) treats B → A as a change. Observe’s [init.ts:74](/Users/admin/Developer/observe-lab/lexy/node_modules/expo-observe/src/integrations/expo-router/init.ts:74) measures subsequent focuses from their navigation actions.

   **Fix:** Define Observe navigation eligibility by a change of selected entry/URL. Preserve the host’s navigation start and first subsequent presentation as raw facts associated with the continuing visit. The module can then emit `warm_ttr` while honoring the owner’s unchanged visit and once-per-screen TTI decisions. This needs a presentation fact beyond the visit’s first `shown`.

3. **Minor — D7: initial navigation metrics need an explicit exception.**

   > “with cause `input` or `program`”

   > “The launch screen … Its metrics come from its own visit, first frame and cause.”

   An ordinary launch screen has cause `launch`, which the eligibility rule excludes. The later sentence appears to intend an exception without stating it. Implementers can reasonably disagree about whether initial `cold_ttr` and marked-screen `tti` exist.

   **Evidence:** D1’s launch origin is [RFC:89](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:89). The existing module explicitly emits launch navigation metrics in [ObserveService.swift:195](/Users/admin/Developer/observe-lab/exact2/modules/observe/apple/service/ObserveService.swift:195); Observe likewise distinguishes initial focus in [init.ts:78](/Users/admin/Developer/observe-lab/lexy/node_modules/expo-observe/src/integrations/expo-router/init.ts:78).

   **Fix:** Say explicitly that the launch screen also qualifies, using its D1 start and its own shown/interactive events. This merits one sentence.

4. **Minor — D2/Tests: cancellation time confuses start with creation.**

   > “A section is cancelled at its start.”

   The corresponding test says:

   > “A section that needs the failed module and mounts later is cancelled at its mount.”

   With input support already failed, a native button’s input at 100 ms can spend 100 ms mounting a section. D1 makes its measurement start 100 ms; the test makes cancellation 200 ms; D2 makes cancellation 100 ms, before the section exists.

   **Evidence:** [RFC:144](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:144), [RFC:225](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:225), and [RFC:506](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:506) prescribe these different timestamps.

   **Fix:** Cancel when the measurement is created, retaining its earlier start as the duration origin. One sentence resolves this; no additional state is needed.

5. **Minor — D4: service deferral does not always protect Observe’s startup TTI.**

   > “so that service work does not delay what they measure”

   An unmarked root can settle while its marked launch screen remains pending—for example, its predicate awaits an application timer, which D5 excludes. Services then load before the screen’s interactive time, although D7 uses that screen for startup TTI. Service initialization can therefore delay the metric it measures.

   **Evidence:** The release rule is [RFC:321](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:321); fallback is [RFC:437](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:437). [ObserveLaunch.swift:1](/Users/admin/Developer/observe-lab/exact2/modules/observe/apple/launch/ObserveLaunch.swift:1) documents roughly 5 ms of main-thread loading work.

   **Fix:** Qualify the guarantee and disclose that an unmarked root’s early settlement can let service work contribute to the fallback TTI. This warrants a caveat, not another service-loading policy.

6. **Minor — D6: the citation does not establish an existing commit timestamp.**

   > “Router changes already carry these facts”

   The cited `RouterChange` carries event provenance, but no timestamp. Linux adds a timestamp at the host boundary; Apple samples time when Swift handles the router batch. These are different facts when a batch waits before application.

   **Evidence:** [RouterChange:20](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/router.rs:20), [navigation_marks.rs:59](/Users/admin/Developer/observe-lab/exact2/host/linux/src/navigation_marks.rs:59), and [NavigationMarks.swift:199](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:199).

   **Fix:** State that provenance already exists and that carrying the actual commit timestamp through batching is new work. This is a sentence-level accuracy correction.

7. **Minor — D5: delete the competing usability criterion.**

   > “To decide what counts toward settled, the question is whether loading still prevents use of the screen.”

   That criterion contradicts the explicit inclusion of resources no open view reads. An unused request can prevent settlement without preventing any screen’s use.

   **Evidence:** [RFC:350](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:350) and [outstanding.rs:74](/Users/admin/Developer/observe-lab/exact2/runner/src/runner/outstanding.rs:74) count pending placeholder resources without checking readership.

   **Fix:** Delete the sentence. The enumerated ledger already defines settled completely; removing this second criterion improves both simplicity and first-reading clarity.

NOT READY