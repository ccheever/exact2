# LLP 1132 r3 — review by a Claude reviewer (Opus 5.5), focused on simplicity, blind, 2026-10-17

Checked against the worktree at 279354b41; every cited file:line was opened (m8 lists the wrong ones).

## Critical

None.

## Major

**M1. D1/D2 — parts get late starts, and a pending part can outlive its visit.** A part's cause is "the input or commit that mounted it, or its visit's cause if it mounted with the visit" (r3:78), and D2 cancels a part only "when a visit ends, or a part unmounts" (r3:96). Content mounted before its visit begins keeps its mount time: a closed dialog's subtree stays in the tree, hidden (`runner/src/geometry.rs:909`); the dialog at `concepts.md:298` is always in the view; likewise a retained tab's routes and a screen beneath a push. So a `section interactive=…` inside `dialog id="compose"` measures from the screen's boot commit until the dialog opens. A part pending when its visit ends (a push, a dialog close, poison) stays mounted, is never cancelled, and can complete in the element's next visit with the user's absence in its time. Fix: a part belongs to the visit current when it mounts, else to its element's next visit; it starts at the later of its mount and that visit's start; the visit's end cancels it with the visit's reason.

**M2. D1/D6 — parts flood the journal; "work per change is constant" is false.** Each mount and renewal is a new part with its own events. The journal keeps 4,096 events on every host (`LaunchParts.swift:39`, `host/linux/src/journal.rs:16`, `marks.js:42`); services attach only at the launch's end, which r3 moves later. D6's benchmark case (a declaration per row, 20,001 rows) journals over 40,000 events before services attach and drops the `launch.*` and `router` events (`modules/observe/README.md:31` already warns of this loss). A visit becoming current or ending must evaluate or cancel every pending part, O(parts), so D6's acceptance cannot pass (r3:176). Fix: measure a part once per visit per literal `id`; refuse `interactive` on a non-visit element without a literal `id`; state the cost as proportional to the declarations whose state changed; benchmark a dialog closing over the list.

**M3. D4 — the launch's time is still missing or late in common cases; claude-b M1 partly resolved.** "Cancelled … when an input navigates away from the launch screen first" (r3:136) is unconditional, so it cancels a root declaration that "is always current" (r3:133): a tab app whose root declares its shell loses its launch time when the user taps a tab early, dropping the slowest launches. No §0 precedent cancels this way. Without a root declaration, the launch screen is a screen visit that a modal dialog pauses (r3:72, 76), so a consent alert adds reading time. "A later one that the launch reached with no input" has no limit (r3:132), and "times reached after the launch ends are still journaled" (r3:139), so a timer redirect minutes later sets `launch.interactive`. Fix: an input navigation cancels only the launch-screen fallback; the fallback is always current, as the root is; fix the launch screen at the launch's end.

**M4. D1 — popover visits were not requested and are underspecified.** The owner asked about "a bottom sheet or modal". D1 gives no begin point for a popover on iOS or macOS (r3:57-58); on the Mac a popover is AppKit's and a menu native (`docs/contract-for-agents.md:1143-1147`). A non-modal popover pauses a dialog beneath it ("topmost open layer", r3:73) but not a screen. Fix: remove popover visits; refuse `interactive` inside a `popover` until a consumer asks.

**M5. D4/D7 — "launch end" names two things, and Linux is misdescribed.** "Launch end" is a raw mark (didFinishLaunching; the display opening on Linux, `README.md:22`, `launch_marks.rs:293`); r3 also uses "the launch ends" for a new moment, so "30 s after launch end" (r3:139) reads circular and `launch.end` is ambiguous. The new moment does not stop measurement but does stop launch parts (r3:78). Linux already runs services on their own thread from the launch-end mark (`README.md:31`, `display.rs:351-353`, `journal.rs:140`), so §R's "on every host" is wrong for Linux, and the table does not say Linux changes. Fix: name the new moment by what it does (for example `launch.services`); keep "launch end" for the mark; state that Linux keeps its threaded load.

## Minor

- **m1. D3 — the hero example and expression cannot compile together.** `load=heroLoaded error=heroFailed` (r3:102) names actions (`error=failed` runs `action failed(message: string)`, `docs/contract-for-agents.md:1351-1354`); the prose reads them as bools (r3:115); states and actions share one namespace (`type-duplicate-name`, `contract/types/src/component.rs:60-84`). Fix: `state heroReady = false`, set by both actions; `interactive=(not showHero or heroReady)`.
- **m2. D3** — "an absent optional counts as false" (r3:109): the compiler refuses a non-bool expression on a bool prop (`lower-attr-type`, `contract/lower/src/values.rs:1223-1250`); only an expression of unknown type reaches `ClearProp`. Name the compile error.
- **m3. D7** — "under the agent's virtual clock" (r3:187) is wrong: the guard is `ExactEnv.agentMode` (`EXACT_AGENT=1`, `Session.swift:61`), also with platform timing; the virtual clock is `agentFreezes` (`Session.swift:70`). An author cannot see a launch declaration's time under the agent on Apple, nor on the web without a launch module. Say how an author checks a declaration.
- **m4. D7 — inventory**: Linux's `observe: startup` and `observe: navigation` lines (`display.rs:675`, `navigation_marks.rs:368`) and its route "as Observe names the screen whose marker fired" (`launch_marks.rs:449`). The frame-rate thresholds cannot move unless the module has a fact to read: name it (frame intervals journaled with `launch.interactive`, or a display link in `ObserveLaunch.swift`).
- **m5. D1 — wording.** "Router changes are journaled as they are made" is under "as built" (r3:52), but no host journals router changes today. "The router's stack entry and URL identify the visit" is false across visits (a back begins a new visit with the same entry and URL). A visit "begins" both at the router change and at its first frame (r3:52, 56). Fix: mark the journal new; "entry, URL and start identify a visit"; "starts" for the cause.
- **m6. D1 — background.** "A return from the background starts no visit" (r3:69): is the ended visit still "current" for parts that mount after the return, and does a router change made in the background begin a visit at its first frame after the return? Today that screen is marked shown on return (`NavigationMarks.swift:132-141`, `README.md:41`), which Observe's cold/warm uses and r3's events cannot reproduce. One sentence each.
- **m7. Simplicity — these can go.** The `launch.*` family (the launch is a visit: use the generic events with kind `launch`, drop kind `root`; `launch.shown` repeats `present` in `launch.marks`). Failure labels on the web (r3:160): new work for a diagnostic with no consumer. Part `shown`: unasked, and false for a part in hidden content (r3:119 does not check `inert`, `hidden` or `display: none`), which an expression cannot correct. Linux image events: parity work independent of this RFC; land it separately.
- **m8. Wrong citations.** `routes.rs:186-198` is the `NavPlace` enum; the rule is its doc comment at 180-184. `host/web/src/abi.rs:536` is the wasm target; the shipping JS target fires image `load` and `error` in `host/web-js/rt.js:893-906`. D5's "SVG pictures" are counted on iOS only (`Session.swift:431-433`).
- **m9. Q1** — HTML defines "interactive content"; `interactive=false` reads as "disable input", beside `inert`, `disabled` and `interactive-widget` (`tags.rs:605`). At minimum, lint a literal `false`, which can never become interactive.

## §R against r2

| §R item | Resolved? |
|---|---|
| Visits replace eligibility | Partly: part lifetime (M1), part `shown` (m7) |
| Starts at first frame | Visits yes; parts no (M1); background (m6) |
| Launch session | Partly (M3) |
| Defined end | Apple and web; Linux and naming no (M5) |
| Observe gets what it needs | Mostly (m4) |
| Settled for the launch only | Yes |
| Examples compile | Partly (m1, m2) |
| Linux image events | Yes, as a question |
| Apple batch delta | Delta yes; cost claim no (M2) |
| Timing gate | Mostly (m3) |
| Corrected | Yes, except m8 |

**Verdict: NOT READY.**
