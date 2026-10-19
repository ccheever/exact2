# LLP 1132 r3 — review by a Claude reviewer (Opus 5.5), focused on semantics and host edge cases, blind, 2026-10-17

Checked against the worktree at 279354b41; every r3 file:line was opened; external documents were not fetched.

## Critical

None.

## Major

**M1. D1/D2 — a part has no defined lifecycle outside a current visit.** Hosts keep covered stack routes and unselected tabs mounted (`runner/src/head.rs:76-79`); a resource landing there mounts a part, which D1 starts at "the input or commit that mounted it" while its shown time waits for the visit to be current again, so its TTR is time spent elsewhere. D2 cancels "when a visit ends, or a part unmounts" but does not say whether a visit's end cancels its parts, and has no reason for it. A screen gets new times each visit, but a part is "measured once" per mount, so a part pending when the user leaves its tab never gets a time though it stays mounted. After a background return, which "starts no visit", the parts of the screen on top have no visit. Launch parts are "always current", so a part outside the routes becomes interactive under a modal dialog that makes it inert. D1 says a launch part "is not measured after the launch ends" and D4 that "Times reached after the launch ends are still journaled". Fix: one rule — a part is measured within one visit of its visit element, starting at the later of its mount and the visit's start; the visit's end cancels it with the visit's reason; a new visit measures again the parts still mounted; launch parts follow the modal rule and the launch end cancels them.

**M2. D1/D4 — the modal pause disagrees with sheet routes; claude-b M1 stays open.** On iOS a dialog and a sheet route are both `.pageSheet` (`DialogSheetIOS.swift:34`, `ModalIOS.swift:29`); on the Mac a modal route "is the sheet a `dialog` is … kept by the same host" (`RouteSheetMac.swift:5-6`). In r3 a sheet route ends the screen beneath and a dialog only pauses it, so the screen's time includes the user's reading of the dialog. With no root declaration `launch.interactive` is the launch screen's, so a consent dialog at launch makes the launch's TTI its close. "The launch is always current, so a launch sheet or a modal dialog does not pause it" holds only with a root declaration. Fix: a modal layer opening over a pending visit cancels it (`covered`), as a sheet route does; or keep the pause and say in Q3 that it measures the time in the layer, the launch fallback included.

**M3. D1 — popover visits cannot be built as specified, and "topmost" pauses layers nothing covers.** On iOS a popover of buttons is a `UIMenu` and "The popover subtree itself never paints here" (`MenusIOS.swift:1-8`); on macOS menus are `NSMenu`, alert-shaped popovers `NSAlert`, others `NSPopover` (`MenusMac.swift:1-8`); D1 names no start event for these. "While it is the topmost open layer" makes a menu popover inside a dialog pause the dialog, and two `popover="manual"` popovers pause each other, though neither is modal and only a modal layer pauses a screen. The owner asked about bottom sheets and modals, not popovers. Fix: drop popovers from layer visits and do not measure parts in them; a layer is current while no modal layer is open above it, as for screens.

**M4. D6/D7 — a part per list row floods the journal; "Work per change is constant" is false.** The benchmark puts a declaration on each row; a declaring component makes every instance a part (D3). Each journals `shown` then `interactive` or `cancelled`; each renewal (LLP 1078) is a new mount, so a recycled list journals again while scrolling. The journal is a 4,096-event ring (`LaunchParts.swift:39`, `host/linux/src/journal.rs:16`, `marks.js:42`); services attach at the launch end, up to 30 s, so about 2,049 declared rows at launch fill it and drop `launch.marks`, `launch.shown` and `router` before Observe reads them. A visit becoming current or ending must visit every pending part, O(rows), so the benchmark's acceptance fails by design. Fix: measure only parts with an `id`, once per visit per `id`; state that a visit transition costs O(pending parts) and benchmark it.

**M5. D4 — the launch end omits Linux, and one name means two things.** Linux starts its services at the launch-end mark, before boot, on their own thread (`display.rs:352-353`; `journal.rs:70`; `modules/observe/linux/launch.rs:160`), with no 30 s bound; D4 names only Apple and the web, so §R's "bounded by 30 s on every host" has no Linux plan. "Launch end" is a raw mark and also "The launch ends at the later of…", so "30 s after launch end" and `launch.end` are ambiguous. Fix: name the composite (for example `launch.complete`), keep "launch end" for the mark, and say whether Linux keeps its early services.

**M6. D7 — the Observe inventory is still incomplete.** Missing: `launchScreen`, Observe's `isAppLaunch` rule computed in the framework (`Launch.swift:542-543`, `marks.js:93-94`, `launch_marks.rs:233-234`), read by the module (`modules/observe/linux/launch.rs:810`); Linux's route "as Observe names the screen whose marker fired" (`launch_marks.rs:449`); Linux's `launchType: "cold"` and the Apple mark names `didFinishLaunching` and `boot` (`launch_marks.rs:422-424, 439`), the module reading `marks["boot"]` (`launch.rs:791`); Linux's `observe:` stderr lines (`display.rs:675`, `navigation_marks.rs:368`); the web's "exact: observe marks:" prefix (`marks.js:45, 101`). "Mapped as the module does today" reads these fields and today's rule that the launch screen's `cold_ttr` is the app's first frame, also for a sheet UIKit presents later (`README.md:39`), which D4 changes. Fix: list them; specify the launch route's mapping from `router` and screen events.

## Minor

- **m1. D1/D7 — a visit has no unique identity.** The router's visit id is a stack entry (`router.rs:21`); tab A → B → A, or a back, puts the same entry and URL on top again, which r3 counts as a new visit, colliding with "at most one of each time" and "A consumer correlates by visit". Fix: a per-session sequence number on each visit's events.
- **m2. D3 — the two examples do not compile together.** `load=heroLoaded error=heroFailed` names actions; the prose reads them as state; states and actions "share one set of names" (`contract/types/src/component.rs:60-84`, `type-duplicate-name`). Fix: states `heroReady`, `heroBroken` set by those actions.
- **m3. D4 — the cancel rule conflicts with the root declaration**: navigating away from the launch screen still cancels the launch, against "always current". Fix: apply it to the fallback only.
- **m4. D1 — a redirect after a tap gives an early time**: a visit replaced before its first frame leaves nothing, so the destination is timed from the redirect's commit. Fix: a replacement before the first frame takes the cause and start of the visit it replaced.
- **m5. D1 — router journaling is not built**: no `router` event exists today (`NavigationMarks.swift:213, 338`). Mark it new.
- **m6. D7 — host statements.** Apple stops startup timing whenever `EXACT_AGENT=1`, also with `EXACT_AGENT_TIMING=platform` (`Session.swift:61-70`, `Launch.swift:121`); Linux times only the display path ("a headless run reports none", `launch_marks.rs:1`); "a declaration costs only the runner's set" is false on the web JS target, which has no runner; Linux has no background signal, so D1's background end does not apply there.
- **m7. Citations.** `renewed` is `txn.rs:86`; the launch-time formula is `Launch.swift:568`; the quote is "(Observe records one per screen)" at `NavigationMarks.swift:1-6`.
- **m8. D2 — two rules for an early end**: a declared part that unmounts before it is shown journals `cancelled`, while a visit that ends before its first frame leaves nothing. Use one rule.

## §R against r2

| §R item | Result |
|---|---|
| Visits | Mostly; parts open (M1), popovers new problems (M3) |
| First frame | Done for visits; parts disagree (m8) |
| Launch | claude-a M2 done; claude-b M1 not for the fallback (M2) |
| Defined end | Apple and the web only (M5) |
| Observe separation | Events done (m1); inventory incomplete (M6) |
| Settled for the launch only | Done |
| Examples compile | Partly (m2) |
| Linux image events, Apple batch, timing gate | Done (m6) |
| Citations | Mostly (m7) |

**Verdict: NOT READY.**
