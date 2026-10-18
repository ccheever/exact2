# LLP 1132 r11 — review by Sol (gpt-6.1-sol, xhigh), blind, 2026-10-17

Read before the text was revised for the writing guidelines; finding 6 was fixed by that revision.

No critical findings. R11 coherently applies explicit declarations, independent sections, optional labels, and the agreed name. Split panes are included, but the revised visit lifetime still has problems.

1. **Major — D1/D2: “host shows” conflicts with the no-pause ruling under stacked sheets.**

   D1 ends a visit when the host stops showing its route, while also saying that a sheet never ends the underlying visit. Those conditions disagree on existing hosts.

   For A → modal B → modal C, the web shows C and its immediate predecessor B; A becomes hidden. A fullscreen presentation also hides its predecessor. See [navigation.js:308](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:308). Linux likewise preserves only the immediate predecessor when `modal` is true: [navigation.rs:53](/Users/admin/Developer/observe-lab/exact2/host/linux/src/navigation.rs:53). The cited `NavigationRules.segments` establishes retained presentation owners, not visibility on every host.

   Consequently, following “stops being shown” cancels A’s pending TTI when C opens, contradicting the owner’s ruling.

   **Fix:** distinguish a visit’s first presentation from its continuing lifetime. Once shown, retain it through covering presentations; end it on replacement within its presentation, tab deselection, removal, or the specified terminal outcomes. Correct D1’s cross-host visibility claim. Preserve the separate requirement that a route never presented cannot become interactive.

2. **Major — D1/D7: the measurement start and the start of showing a route are conflated.**

   D1 says a screen visit “starts when its route becomes shown.” D7 journals a `start`, but never clearly distinguishes that timestamp from the input anchor described in the Summary and D1’s cause rules.

   These yield different durations when an input handler performs work before committing navigation. If the tap occurs at 0 ms, selection at 100 ms, and presentation at 116 ms, the possible TTRs are 116 ms, 16 ms, or zero. D1’s section rule compounds this by taking the later of the visit’s start and the mounting input.

   The existing implementation explicitly uses the input timestamp, otherwise the commit: [NavigationMarks.swift:243](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:243), [navigation_marks.rs:246](/Users/admin/Developer/observe-lab/exact2/host/linux/src/navigation_marks.rs:246).

   **Fix:** define `start` explicitly as the causal input timestamp, otherwise the selection commit; define lifetime eligibility and `shown` separately. State the foreground and layout-only anchors explicitly. This needs clearer definitions, not another mechanism.

3. **Major — D2: optional input loading can hold unrelated, already working key UI.**

   Runtime readiness is app-wide. An unselected tab containing a custom pressable can start the deferred input load, thereby delaying—or permanently cancelling—the declared TTI of a selected screen containing working native buttons.

   Native buttons receive their click listeners immediately; only custom pressables trigger `input()`: [rt.js:914](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:914). The deferred load is global and retains its failed promise: [rt.js:1140](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1140).

   That recreates an implicit dependency on UI outside the application’s declared key UI. D2’s citation also overstates the dependency: the input module and motion executor load independently; custom keyboard activation does not generally require motion.

   **Fix:** gate on the dispatch facilities needed by the measured subject. Do not make an unrelated optional input or motion capability a universal terminal failure. Specify and test the native-control screen/unselected-custom-control case.

4. **Major — D7: Observe’s supersession policy disappears when general visits survive sheets.**

   D7 takes navigation TTI from each router-started visit’s `interactive`. Under the new no-pause rule, B’s visit survives navigation to sheet C, so B can emit interactive afterward. The mapping consequently sends B’s navigation TTI.

   The consumer’s documented policy says a newer navigation cancels a measurement still waiting: [README.md:41](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:41). That policy currently lives in the framework’s replacement of `pending`: [NavigationMarks.swift:225](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:225).

   **Fix:** keep B’s general visit alive, but explicitly transfer Observe’s navigation-supersession rule into the module. Its pending navigation can end independently of the framework visit. If changing that consumer policy is intended, say so rather than presenting D7 as moving the existing policies intact.

5. **Minor — Status/D5: proposed removals are presented as built behavior.**

   The status says D5 describes what the branch builds, and D5 is labelled “built,” yet it says settlement is unjournaled and exposed through `timing`.

   Today Apple journals the startup report containing `timeToInteractive`: [Launch.swift:379](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:379), [Launch.swift:571](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:571). The agent section remains `observe`: [Agent.swift:293](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Agent.swift:293). The web likewise journals the metric: [marks.js:79](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:79).

   **Fix:** label the loading ledger as built, and the unjournaled diagnostic, renaming, and removal of navigation settlement metrics as proposed changes.

6. **Minor — D6: the eager-adoption rule retains the discarded attribute name.**

   It requires eager adoption for a template declaring **`interactive`**, although the registration rule correctly uses `markinteractive`. Implementing that predicate literally misses the declarations whose late adoption this mechanism exists to prevent.

   **Fix:** replace this remaining token with `markinteractive`, including transitive component detection and the adoption test.

7. **Minor — D3/D6/Tests: remove optional machinery from the required design.**

   The repeated-section compiler warning adds component-use cardinality analysis merely to explain the owner’s chosen per-copy behavior. Documentation can explain that behavior and its journal-volume cost.

   D6 also mandates a new benchmark while acknowledging that it needs approval; the conclusion nevertheless says nothing is open. [RULES.md:64](/Users/admin/Developer/observe-lab/exact2/rules/RULES.md:64) requires explicit approval for new apparatus.

   **Fix:** drop the cardinality warning and dedicated benchmark requirement. Verify costs through existing measurement tooling, or record approval for specifically scoped new apparatus. Neither addition is necessary to provide independent section metrics.

The receipt design otherwise credibly covers changed values, clears, invalid initial values, renewed descendants, and viewless Apple nodes. The web’s separate registration mechanism is necessary because it has no runner.

**NOT READY**