# LLP 1132 r2 — review by a Claude reviewer (Opus 5.5), the second of two, blind, 2026-10-17

Codex (Sol, Astra) was at its workspace spend cap; the owner chose two Claude reviewers for r2. Every r2 citation was opened against 279354b41; external documents were not fetched.

## Critical

None.

## Major

**M1. D4 — the launch fallback gives late, user-dependent times.** "Otherwise interactive is the first interactive time of any screen occurrence in the process." With D1's end-on-cover and new-occurrence-on-return, the launch's time can be a launch modal's close (including reading time), a time after a return from background, or minutes later when the user first reaches a declared screen. With a root declaration, D4 does not say which root occurrence counts after a background; "in the process" includes other sessions. As built, a background before TTI ends startup with none (`modules/observe/README.md:32`); startup is the launch session's (`Launch.swift:198-211`, `NavigationMarks.swift:213`); today's time is app-wide settlement labelled with the screen on top (`Launch.swift:360-363`), so "how the branch behaves today" is wrong. Astra 2 ("derive launch readiness from the actually eligible destination") is not done. Fix: the root's first occurrence, else the screen occurrence the launch caused (eligible at the first frame, or reached only through program-caused changes); journal `launch.cancelled` if it ends first; launch session only; say whether a modal layer covers the root.

**M2. D7/D1 — Observe cannot compute the rules that move to it, and `firstVisit` keeps Observe's history in the framework.** Once per screen needs a screen identity (visit + URL) and removed visits (`ScreenHistory`, `NavigationMarks.swift`); D1's labels are route pattern, URL, parameters and `id`, and "Labels never correlate occurrences". Cold/warm TTR maps from a screen's `shown` "whose cause changed the router", but D7's causes are only `input`/`program`, and a dialog close, a tab reselected and a foreground return all produce a screen `shown`. `firstVisit` is Observe's cold/warm rule and requires the framework to keep `ScreenHistory`. Fix: the router's visit id and a `navigated` flag on screen events; journal router changes with `removed`; let `modules/observe` compute first visit and once per screen; delete `firstVisit` from Exact.

**M3. D7 — the Observe inventory and the timing gate are incomplete; Sol 8 partly resolved.** Missing: Apple's `<type>LaunchTime`, `timeToFirstRender`, `timeToInteractive` "by Observe's definitions" (`Launch.swift:562-570`); the report as "Observe's metric names" (`Launch.swift:486`); the screen label "as Observe names the screen whose marker fired" (`Launch.swift:540`); `UserDefaults(suiteName: "dev.exact.observe")` (`Launch.swift:462`); the web's `timeToFirstRender`, `timeToInteractive` (`marks.js:89-90`). "Timing runs when something reads it: any launch module, or the agent in a development build" contradicts the gate that "stays" (`build.mjs:339-340`) and Apple's agent-mode off switch (`Launch.swift:121, 285`). Fix: list every name; open the gates to development builds or delete "or the agent".

**M4. D3/D1 — the main example does not compile.** `dialog open=(composing)` is refused: "`open` alone shows a dialog non-modally, which Exact does not support" (`lint.rs:145-151`); a dialog opens only with `showModal(id)` or `commandfor=id` (`concepts.md:228-230`). So D1's "not when `open` is set" names a mechanism that does not exist, and "a dialog may have none" (the basis of the Sol 9 fix) is false: a dialog needs an `id` to open. Fix: `dialog id="compose" interactive=(drafts.ready)`, opened with `showModal("compose")`; base "labels never correlate" on repeatable ids.

**M5. D1/D6 — eligibility is incomplete, and D6's source of changes does not cover it.** A split that collapses or expands changes eligibility with no navigation (`beside_a_pane` reads `host_block`, `head.rs:138-143`), but D6 says changes come from navigation events. An authored `inert`, `hidden` or `display: none` ancestor still counts as eligible; the web honours `authoredInert` (`navigation.js:302, 319`). macOS has no presentation or cover rule (`routeAppeared` is iOS/tvOS, `NavigationMarks.swift:272`; `ModalIOS.swift:453` is iOS). D6 finds "the nearest route or layer element" where D1 requires "every route around it". The web sets `inert` only on panels, the tab list and routes (`navigation.js:284-319`), so whether a sheet covers an element outside the navigation root is undefined. Fix: split and authored inert/display changes as inputs; macOS presentation and covering; the whole chain of enclosing routes and layers; define covering outside the navigation root.

**M6. D1/D2/D4 — two rules for when an occurrence exists conflict.** D1 starts it at eligibility (on iOS `viewIsAppearing`, before the first frame); D4 says "a screen replaced before its first frame … has no occurrence". D2 journals any occurrence that "ends first" as `cancelled`, so every navigation away from an undeclared screen journals a `cancelled`. Fix: start an occurrence at its first presented frame; journal `cancelled` only when a time is still pending.

**M7. D5 — per-screen `settled` has no consumer under Q2's recommendation.** Every Back, tab selection, dialog close and foreground return computes one; "a stream that never sends holds it forever", so the ledger is read after every apply while that screen is on top, each read a JSON agent op on Apple (`Launch.swift:304-305`). Fix: keep `settled` for the launch only; delete the per-screen ledger logic in `NavigationMarks.swift`, `navigation_marks.rs` and `marks.js`; bring it back only if Q2 chooses the fallback.

## Minor

- **m1. Citations.** The launch path is `NavigationMarks.swift:207-222`, its frame callback at 218; `:340,389` are 338 and 386; `navigation_marks.rs:363` is 334 and 354; `head.rs:109-144` covers only `beside_a_pane`, and head.rs selects the route the root's `navigationKey` names, not "on top of its stack". D6's 14/61/400 µs are for the removed walk; `outstanding.rs:66-101` walks no tree now — say "before the removal".
- **m2. D6:** the benchmark cannot pass: registration walks parent links, O(depth), and a deep 20,001-node tree has depth near 20,001. Carry the enclosing route and layer top-down in the runner (O(1)), or accept O(depth).
- **m3. D6:** "which it already reads each turn" is wrong: Swift reads the ledger only while a startup or a navigation waits, and never in agent mode at startup (`Launch.swift:121, 285, 304`).
- **m4. D3:** "any element's expression can read the data its key UI needs" is false inside child components: "a child takes their results as props" (`concepts.md:655`); a screen cannot read a child's local `heroLoaded`.
- **m5. D3:** Linux does not fire `load=`; only `host/apple/src/abi.rs:1010` and `host/web/src/abi.rs:536` map `Event::Load`. Such a declaration never becomes true on Linux, with no deadline.
- **m6. D2(2):** the web has no failed-activation signal ("a failed load never ends", `marks.js:47-48`).
- **m7. D2:** an occurrence that starts after poisoning (UIKit's own Back can still pop a route) waits forever; cancel it when it starts.
- **m8. Tests:** Linux has no split view (`navigation.rs:53-66`), so it cannot run "the first route beside a pane".
- **m9. D4:** "None when nothing declares" is inaccurate: a declaration on a part alone also gives none.

## §R against r1

| §R item | r1 | Resolved? |
|---|---|---|
| One mechanism | Astra 8 | Structure yes; Observe separation no (M2, M3) |
| Eligibility, not history | Sol 1, 2; Astra 1 | In principle; gaps (M5, M6) |
| No interactive time without a declaration | Sol 4; Astra 7 | Yes; M7 is leftover apparatus |
| Viable runtime | Sol 5; Astra 4 | Yes, except m6, m7 |
| The runner keeps declarations | Sol 6, 7; Astra 5, 6 | Yes; m2, m3 |
| The launch's own rule | Sol 3; Astra 2, 9 | Sol 3 and Astra 9 yes; Astra 2 no (M1) |
| Observe's policies move | Sol 8; Astra 8 | Partly (M2, M3) |
| Corrected | Sol 9–12; Astra 3, 10–12 | Yes, except Sol 9's reasoning (M4) and the Sol 11 benchmark (m2) |

**Verdict: NOT READY.**
