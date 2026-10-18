# LLP 1132 — D2 questions (input readiness scope, app visit entrance) and Observe navigation `tti`, by Sol (gpt-6.1-sol, max), with the Expo Observe docs fetched 2026-10-18

**1. Count mounted descendants, including nested sections, for input readiness.**

Recommended RFC wording:

> A mark waits for input handling required by its element and its currently mounted descendants, including descendants inside separately marked sections; elements outside that subtree do not count. An enclosing mark never waits for a section’s expression or interactive time.

Thus a screen waits for the input module when either a nested section or an unmarked descendant uses a custom pressable. A section does not wait for unrelated controls outside it, but does count controls inside a nested section. A root mark consequently has app-wide structural scope.

This preserves the settled distinction: a comments section’s pending data does not delay the screen unless the screen’s expression includes it. Installed input machinery is a prerequisite for operating the measured container, independently of whether comments have finished loading. Adding a section mark should not change the ancestor’s input requirements.

**Evidence.** Observe requires “Content is rendered on the screen,” “Touch handlers are attached and responsive,” and “Navigation is functional.” It leaves the app responsible for deciding when its content is ready. Those requirements support checking actual input prerequisites, while keeping application readiness explicit. [Metrics reference:142–150](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/reference_metrics.md:142)

The emitter knows template structure, attributes and handlers: [emit.rs:35](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/emit.rs:35), [emit.rs:986](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/emit.rs:986), [emit.rs:1368](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/emit.rs:1368). Custom pressables require deferred keyboard activation; swipes and drags attach through the motion executor. [rt.js:914](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:914), [rt.js:1022](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1022)

**Implementation cost.** Moderate bookkeeping, not a new dependency graph. Emit two requirement bits, for input and motion, and aggregate mounted contributions for enclosing marks. Conditional arms and rows need mount/unmount accounting: compile-time knowledge of possible templates alone is insufficient. Otherwise an absent branch could require a module that nobody requests. Detached fragments require attachment-time ownership resolution; existing scope cleanup supports removal. [regions.rs:27](/Users/admin/Developer/observe-lab/exact2/host/web-js/src/regions.rs:27), [rt.js:1172](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1172), [rt.js:43](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:43)

Maintain counters on structural changes; readiness reads remain O(1), and loader completion evaluates affected marks. The loaders need explicit ready/failed states and notification **after handlers attach**, rather than relying on page-wide `inflight`. Cosmetic press feedback must not contribute a requirement. [rt.js:999](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:999), [rt.js:1135](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1135)

**Rejected alternative.** Cutting ownership at nested marks would let adding measurement markup make an ancestor interactive earlier without changing its UI. Waiting for every input dependency anywhere in the app would instead delay narrow marks for unrelated screens. Exact should enforce these local prerequisites automatically; Observe’s imperative API delegates their verification to the caller.

**2. An app has no presentation entrance; foreground input eligibility belongs in runtime readiness.**

Recommended RFC wording:

> An app visit has no presentation entrance, so condition 3 holds from its start; its interactive time still cannot precede shown. For all measurements during that app visit, condition 2 additionally waits until any host foreground-input activation phase has completed—on UIKit, the scene’s first foreground-active state—without claiming that the system’s animation has ended.

This answers the timing question directly: **condition 3 holds from the start**, not from the first frame or a guessed animation endpoint. However, a first frame alone is insufficient while UIKit still considers the scene inactive. Apply the foreground requirement to screen and section measurements too, so Observe’s startup fallback to a screen cannot bypass it.

**Evidence.** Observe’s intent is usability, not merely drawing; its setup instructions explicitly include the app’s “splash screen animation” among startup work. That supports waiting for an input-blocking entrance the host actually knows about, but does not establish a requirement to measure SpringBoard’s final animation frame. [Get started:59](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/get-started.md:59)

The available host observations differ:

- **iOS/tvOS:** `sceneDidBecomeActive` already reaches the session. Foreground timing currently begins earlier, at `willEnterForegroundNotification`; touch driving separately checks foreground-active state. These distinguish return/start from input eligibility. [main.swift:295](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactIOS/main.swift:295), [Launch.swift:439](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:439), [TouchIOS.swift:159](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/TouchIOS.swift:159)
- **macOS:** timing observes hide/unhide. Sheet entrances have a separate observable completion, but that is the sheet’s entrance, not an app launch animation. [Launch.swift:420](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Launch.swift:420), [SheetsMac.swift:219](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:219)
- **Web:** visibility/prerender activation and rendering opportunities are available; the inspected timing path observes no browser-window entrance completion. [marks.js:258](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:258)
- **Linux:** DRM presentation is observable; the RFC identifies no background signal. Android and Windows timing remain explicitly outside this RFC. [launch_marks.rs:1](/Users/admin/Developer/observe-lab/exact2/host/linux/src/launch_marks.rs:1), [RFC:131](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:131), [RFC:515](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:515)

**Implementation cost.** One foreground-readiness latch per app visit, initialized from current scene state and released through the existing activation callback. Reset it only for a new app visit; sheets and later transient inactivity do not pause it. Other current timing hosts need no additional animation gate.

**Rejected alternatives.** First-frame-only readiness can precede UIKit input eligibility. Waiting for the system animation’s exact end would require an observation these timing paths do not possess; a fixed delay would merely invent precision. Exact should use the host’s meaningful lifecycle boundary, consistent with [CLAUDE.md:18](/Users/admin/Developer/observe-lab/exact2/CLAUDE.md:18), while documenting its limits rather than reproducing Observe’s effect-call timing.

**3. Change D7 to one navigation TTI per navigation.**

Recommended RFC wording:

> For each eligible navigation, Observe maps that visit’s first `interactive` event to one navigation `tti`, even if the same screen was interactive on an earlier visit. Screen history determines cold versus warm TTR; it does not suppress TTI on later navigations.

**Evidence.** Both integration pages say: “Only the first call per navigation is recorded.” The separate statement “Emitted at most once per screen instance within a session” belongs to **cold TTR**. Startup TTI has yet another rule: first call per launch. [Expo Router:120](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/integrations_expo-router.md:120), [Expo Router:147](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/integrations_expo-router.md:147), [React Navigation:198](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/integrations_react-navigation.md:198), [Metrics reference:132](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/reference_metrics.md:132)

For A → B → A, the return can therefore have warm TTR **and another TTI**. Under the stipulated retained-screen `useEffect` that does not rerun, there is no fresh application call. That can leave the return without a freshly marked TTI; it does not establish a once-per-screen policy. The supplied docs do not settle whether an already-consumed mark is reused on refocus. React Navigation documents deferred emission for calls made while unfocused, which is a narrower promise. [React Navigation:231](/private/tmp/claude-501/-Users-admin/b6abd01f-6f86-486f-b886-7de5cfcf63ff/scratchpad/observe-docs/integrations_react-navigation.md:231)

Exact need not reproduce mount-effect omissions: its persistent expression is evaluated for each new visit, including when already true.

**Implementation cost.** Small. Remove lifetime TTI suppression from the policy being moved into Observe, retain shown history for cold/warm classification, and deduplicate by navigation identity. Today the suppression is real code, not just wording: `measuresTTI: !screen.interactive` and the web’s corresponding early finish. [NavigationMarks.swift:262](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:262), [marks.js:212](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:212)

**Rejected alternative.** Keeping once-per-screen because a sample effect may run once confuses instrumentation behavior with the documented metric rule. Change D7’s inventory and mapping, plus [modules/observe/README.md:39](/Users/admin/Developer/observe-lab/exact2/modules/observe/README.md:39); the local Swift comment is evidence of Exact’s current assumption, not authority for Observe’s contract.

## Author's note on question 3

Sol had the documentation only. expo-observe 57.0.9's source (in the lab app's `node_modules`) disagrees between its two integrations. The Expo Router integration records one `tti` per screen ID: "TTI is recorded once per screen ID for the lifetime of the storage — re-focusing the same screen (A → B → A) must not produce a second metric" (`src/integrations/expo-router/useObserveForRouter.ts`, `wasAlreadyInteractive`). The React Navigation integration records one per navigation: "Record interactive once per navigation" (`src/integrations/react-navigation/useObserveForReactNavigation.ts`, `previousWasAfterDispatch`). The documentation states the second rule for both.
