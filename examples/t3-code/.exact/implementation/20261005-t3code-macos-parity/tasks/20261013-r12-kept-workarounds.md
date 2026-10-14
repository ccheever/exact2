---
name: 20261013-r12-kept-workarounds
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Follow-up: the workarounds round 12 kept after main's fixes, and Inspect app colors

## Outcome

Round 12 ([adopt-main-fixes-r12](20261013-adopt-main-fixes-r12.md), draft PR #520) adopted main through `aa9705c33`. Main's
fixes since round 11 end the framework reason for each workaround below, but round 12 kept them: each needs a parity
change across many sites, a rework, or a drive the round's lanes lacked. This record lists them so they are not lost (the
round 12 review asked for follow-up tasks). The coordinator decides whether to run it, split it, or move rows to
`STATUS.md` "Known differences"; nothing here is started. Each row names the reference, the clone site, what main now
offers, and what retiring needs. `EXACT2-GAPS.md` "Closed on main since round 11" and "Partly fixed" hold the same rows.

## Rows

| Id | Reference (`1e2ecbd975`, `apps/web/src`) | Clone now | Main now offers | What retiring needs |
| --- | --- | --- | --- | --- |
| KW-1 (X10) | `text-balance` on the tooltip, popover tip, empty state and toast (`ui/tooltip.tsx:112`, `ui/popover.tsx:90`, `ui/empty.tsx:22, 115`, `ui/toast.tsx:731`); `placeholder:` colours on inputs (`ui/input.tsx:25`, `ui/select.tsx:96`, `ui/command.tsx:106`, `ui/sidebar.tsx:541`, `GitActionsControl.tsx:830`, `settings/ThemeImportDialog.tsx:127`, `chat/AssistantCitationCommentEditor.tsx:54`) | Neither is written: tips wrap greedily, placeholders take the host's colour | `text-wrap: balance` and `::placeholder { color }` on every host (`b686fb6df`) | A pass over the clone's tooltip, popover, empty-state, toast and input components (each a shared class or component), then one drive of a long tooltip and a placeholder against the reference. Status: implemented by tasks/20261014-css-workaround-retirements.md (2026-10-14); see its KW-1 row for what was driven. |
| KW-2 (X57) | No `text-left` at the base freshness mark, the filter submenu's value, the right panel's tab title and the palette's option rows | `text-align="left"` at those sites (the overflow line started left of a centred box on macOS) | A line wider than its box is start-aligned on the Apple hosts (`24083b9b9`) | Per site: does the box ever exceed the text (then the reference's centre shows)? Remove where the reference says nothing; drive each once. Status: implemented by tasks/20261014-css-workaround-retirements.md (2026-10-14); see its KW-2 row for what was driven. |
| KW-3 (X60) | `type="number"` Tailscale port field (`settings/ConnectionsSettings.tsx`) | `tsStep` steps the field itself (1 to 65,535) | Arrow-key stepping, the adjustable role, Chrome's edit rules on macOS (`3998e650e`) | Remove `tsStep`; drive the Tailscale dialog on the tailnet stub lane (ArrowUp/ArrowDown at both bounds, a typed letter) |
| KW-4 (X65) | A press on the Limits page's ground closes the pinned segment popover | `usage-ground`, a box that hears the press | An empty scroll port passes the press on to the scroll node (`3998e650e`) | Remove the box; drive the pinned popover and a press on the ground with pooled limits data (the pages lane). Status: implemented by tasks/20261014-css-workaround-retirements.md (2026-10-14); see its KW-4 row for what was driven. |
| KW-5 (X66) | Base UI popovers and menus opened from the keyboard | `KeyMenuOpen` (an invisible `popovertarget` button the key handler presses), the snooze row's pin | `showPopover`, `hidePopover`, `togglePopover`, `toggle=` on every host (`62469206a`) | A rework of `menu-keys.contract`'s door and its 38 uses; the menu-keys tests and one drive per menu kind |
| KW-6 (X17) | Base UI flips a popup to the other side near an edge | Every popover says `position-try-fallbacks="none"` (no `NSPopover`, no flip, round 12); hover cards flip in the window hover layer (`hoverFlip`); end-aligned layers use fixed placement | `position-try-fallbacks` with `flip-block` and `flip-inline` on invoker popovers (`38281323f`, first slice); an authored row keeps the painted layer | Per popover: `flip-block` where the reference flips, checked against the reference near the window's edges; `hoverFlip` stays until hover-opened popovers are covered. Status: implemented by tasks/20261014-css-workaround-retirements.md (2026-10-14); see its KW-6 row for what was driven. |
| KW-7 (#287) | `border-dashed` on the unresolved chip (`ContextChip.tsx:51`) and the pending review comment card (`pullRequest/PullRequestReviewAnnotation.tsx:66`) | A stroked dashed `svg` rect over each box (`markdown.contract`, `pages-pr-threads.contract`) | `dashed` borders with Chrome's geometry (`ff2251b0c`) | Replace each rect with a 1 px dashed border (mind the box's size: the rect added none); drive the chip (a message with an unresolved reference) and the card (the real-GitHub lane). Status: implemented by tasks/20261014-css-workaround-retirements.md (2026-10-14); see its KW-7 row for what was driven. |
| KW-8 (README pitfall #99) | The terminal drawer's height animates | `border-box` and `interpolate-size` on the drawer | A numeric `height` transition with either box sizing, no keyword opt-in (`6621ff898`) | Remove both; film the drawer opening and closing (`screenshot … over 600 every 50`) |
| KW-9 (X33) | Cite's popover anchors at the selection's end rectangle; a code block's selection clears after Cite | Cite anchors at the action's frame | `Selection.rects`, `removeAllRanges`, `setSelectionRange` (`48755525c`) | A parity change in the Cite path (G4-03, diff-cite-popover); drive Cite from a multi-line selection |
| KW-10 (X45) | "T3 Code will restart." after a Local environment or Network access change | The embedded server restarts in place (U4's stopgap) | `relaunch()` on macOS (`df28d4ced`) | Decide with the user whether to relaunch as the reference (U4 said relaunch); then call `relaunch()` after the persisted change |
| KW-11 (X25, X26) | DOM capture-phase keys; the Electron menu bar | Native key readers (`T3ComposerIntent`, `T3KeyRecorder`, `R8KeysLauncher`, `T3Sidebar`); `T3Menus.swift`, `R8KeysMenus.swift` | `keycapture=` (`3f283d582`); a button names its menu-bar menu (`6ef2c3f9a`) | Reworks, each its own task if taken: move a reader to `keycapture=`, move the bar's items into Contract |
| KW-12 (X22, X23, X27, X54) | `IntersectionObserver`, scroll restore, the title bar's display mode, bubbling focus | `t3-turn` hooks; `R9Input.swift`, `T3TimelineTurns.swift`; `T3FullScreen.swift`; "Show full comment" as a stop, `KmDoor` | `intersect=`, `firstVisibleKey`/`scrollIntoView offset=`, `displayMode`, `focusin`/`focusout` (partial each) | Reworks; each issue stays partly open on main |
| KW-13 (X68, U18) | The theme editor's Inspect app colors (`settings/ThemeEditorPanel.tsx:612-724`, `themeInspector.ts`) | Not built | `elementsFromPoint(x, y)` as an action read, and an overlay that takes its own click on macOS (`aa9705c33`, #321 and #281 closed) | Build Inspect: an overlay that takes the click, `elementsFromPoint` for what it covers, the role from the clone's themed paint; [blocked-theme-usage-highlight](20261009-blocked-theme-usage-highlight.md) (X69) is the sibling |

Kept and not listed here, since main's change does not end their reason: X28 (a notification's `navigate` needs routes the
clone does not have), X83 (`background-attachment: local` stays deferred), X11's flattened glass (the parent-subtree
backdrop limit is declared, ruling 12), X53's traps (`close=` serves a `dialog` element, a sheet on the Mac), #292's
`T3MenuTurn` (the module's own `NSMenu`s), X19 (data-module timers stay deferred).

## Acceptance

Per row taken: the Bun source guard, `contract build`, one agent drive (or a real-input step where only real input
reaches it) with before | after | reference images; the row's `EXACT2-GAPS.md` cell says retired.

## Next action

The coordinator: decide which rows to run (and how to split them), or move them to `STATUS.md` "Known differences".
