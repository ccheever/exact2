# LLP 1132 r2 — review by a Claude reviewer (Opus 5.5), one of two, blind, 2026-10-17

Codex (Sol, Astra) was at its workspace spend cap; the owner chose two Claude reviewers for r2.

All r2 citations were checked against HEAD 279354b41; three have small offsets (m6).

## Critical

None.

## Major

**M1. D1/D7 — the Observe module cannot apply Observe's rules from the specified events.** D1: "Labels never correlate occurrences", and the labels have no router visit id; D7's cause is only `input` or `program`. Observe's once-per-screen keys on visit + URL (`NavigationMarks.swift:397-415`, `marks.js:164-178`). D7 maps `cold_ttr`/`warm_ttr` from "a screen's shown whose cause changed the router", but no field says the router changed; under r2 a dialog close or a return from background starts a new screen occurrence the module cannot tell from a navigation. The module also emits the launch route's `cold_ttr` and `tti` (`modules/observe/linux/launch.rs:789-808`), which D7 does not map. Fix: put the visit id and a "began with a router change" flag on screen events and allow correlation by visit id; map the launch screen explicitly. Astra 8 is only partly resolved.

**M2. D4 — the launch fallback and its lifecycle are wrong.** "The first interactive time of any screen occurrence in the process", called "how the branch behaves today", is false: the branch uses only the launch session (`NavigationMarks.swift:212-213`, `Launch.swift:362`) and ends startup as `interrupted` on background (`Launch.swift:426-431`). Under r2 a second window's screen, or a screen after a return from background, sets `launch.interactive`. The root contains every layer: D1 does not say whether a launch sheet or a modal dialog covers the root, nor what a cancelled root occurrence followed by a new one means. "This is what Observe records" puts Observe policy in the framework. Fix: `launch.interactive` is the root's first occurrence's interactive time, else the launch destination's occurrence in the launch session; background or poison before it ends the launch without one; the root is exempt from route and cover rules; "first per launch" goes to the module.

**M3. D1/D2/D4 — the occurrence lifecycle contradicts itself and has gaps.** D1 starts an occurrence at eligibility; D2 journals any that ends first as `cancelled`; D4 and the Tests say a route replaced before its first frame "has no occurrence" and leaves "no trace". A return from background makes elements eligible with no input and often no commit, so "the cause is the commit" gives no start (Sol 1 asked for "their restart on return"). D1 names "the launch" as a cause, but D7's cause has only `input` and `program`. Fix: drop, unjournaled, an occurrence that ends before it is shown; start at the foreground notification; add `launch` to the causes.

**M4. D4/D5/D7 — startup's end is undefined, and services load at that end.** Services load when startup finishes (`Launch.swift:366-375, 391`; `marks.js:66-75`); on the web `finish` is the only trigger and there is no timeout, where Apple has one (`Launch.swift:122-130`). r2 does not say which event ends startup or sets the report's `tti` outcome; a root declaration that never becomes true never loads the web's services. Fix: name the event — the later of settled and interactive, bounded by the existing 30 s timeout on every host, the web included.

**M5. D3 — the recommended image and resource declarations stay false forever on some hosts and paths.** `load=` fires on "the web, macOS and iOS" (`docs/contract-for-agents.md:1351-1352`); Linux has none (its only `"load"` is a refused media command, `presenter/commands.rs:113-115`), so `interactive=(heroLoaded)` never becomes true there, and a symbol "fires neither". `feed.ready` is app-defined; the documented idiom is `loaded: bool` (`contract/cli/src/vocab/concepts.md:51-53`). A failed resource keeps the expression false, and no cancel reason covers it. Fix: make Linux fire `load`/`error` or document the gap; show `not showHero or heroLoaded or heroFailed` and `feed.loaded or failed(feed)`; add a Linux test.

**M6. D6 — the per-read cost on Apple does not hold.** Swift reads the ledger as JSON through the agent op (`Launch.swift:304-312`, `Bridge.swift:689-694`); carrying the set "with the ledger" serializes every declaration each read, so the cost grows with declarations, not waiting occurrences. Declarations grow with the node count when a list row's component declares, which a benchmark with fixed declarations cannot find. A declaration that never becomes true keeps reads going all session, where today they stop at TTI or cancel. Fix: send changed declarations only (a delta in the batch); benchmark one declaration per row at 401 and 20,001 nodes; state the cost of an occurrence that waits forever.

**M7. D7 — timing activation contradicts the code; Sol 8 is not resolved.** "The agent in a development build": Apple turns startup timing off under `EXACT_AGENT=1` (`Launch.swift:121, 285`; `Session.swift:61`). The web includes `marks.js` only when a launch module has a web service (`build.mjs:340-341`), and D7 keeps that gate, so an app that declares `interactive` without a launch module gets no timing. Fix: delete the agent claim and document that declarations do nothing without a launch module, or turn timing on when `interactive` is declared or in development builds, and add that to the table.

## Minor

**m1. D1 — eligibility restates each host's inertness, and its citation is host-dependent.** `head.rs:109-144` is the runner's layout inference and returns false when the list has its own host block (`head.rs:115-117, 138`). The rule misses authored `inert` (`navigation.js:302, 319`, `authoredInert`) and `display: none`: a declared part inside an authored-inert region would become interactive. Fix: one condition — the host presents it, it is not inert by the host's rule (`navigation.js:313-319`, `navigation.rs:53-66`, `ModalIOS.swift:453-454` plus `RouteController.appeared`), and the app is in the foreground.

**m2. D1 — macOS has no `routeAppeared`** (`#if os(iOS) || os(tvOS)`, `NavigationMarks.swift:272`); Mac sheets use `beginSheet` (`SheetsMac.swift:228`). Name the AppKit fact.

**m3. D2/D5 — two host differences are missing.** Apple navigations do not count activation; only startup adds it (`Launch.swift:299`). The web has no activation-failure signal ("a failed load never ends", `marks.js:47-48`), so D2.2 cannot be built there as written.

**m4. D7 — the inventory of Observe names is incomplete.** Also: `timeToFirstRender`, `timeToInteractive` and `<type>LaunchTime` in the Apple and web reports (`Launch.swift:567-571`, `marks.js:89-90`); the `tti` outcome field (`launch_marks.rs:442`, `marks.js:92`); the `observe:` smoke lines (`ExactIOS/main.swift:162`, `ExactMac/main.swift:738`); the `dev.exact.observe` defaults suite (`Launch.swift:462`); the route at TTI, "as Observe names the screen whose marker fired" (`Launch.swift:540`).

**m5. D5 — per-screen `settled` has no consumer.** Keep `launch.settled` (M4 needs it); drop per-occurrence settled until someone names a consumer (agents add no apparatus).

**m6. Citation offsets.** The next-frame callback is at `NavigationMarks.swift:218-222`, not 205-214; the web gate is `build.mjs:341`; `host/android` is also at HEAD; `host/windows` is not mentioned.

**m7. Gaps.** What `interactive` on a component use means; what an invalid (optional) value means (the runner sends `ClearProp`, `instance.rs:1048-1053`); the benchmark is new apparatus and needs Charlie's approval.

## §R against r1

| §R item | r1 | Result |
|---|---|---|
| One mechanism | Astra 8 | Partly (M1, M2) |
| Eligibility | Sol 1, 2; Astra 1 | Mostly; foreground restart undefined (M3), m1, m2 |
| No interactive time without a declaration | Sol 4; Astra 7 | Yes; one host difference missing (m3) |
| Viability | Sol 5; Astra 4 | Yes in text; not buildable on the web (m3) |
| The runner keeps declarations | Sol 6, 7; Astra 5, 6 | Coverage and renewal yes; new cost (M6) |
| Launch rule | Sol 3; Astra 2, 9 | Apple gate yes; redirects, root, background no (M2, M3) |
| Observe policies move | Sol 8; Astra 8 | No (M7, m4) |
| Corrected | Sol 9–12; Astra 3, 10–12 | Yes |

**Verdict: NOT READY.**
