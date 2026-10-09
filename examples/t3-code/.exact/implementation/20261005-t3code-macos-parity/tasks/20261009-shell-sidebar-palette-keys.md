---
name: 20261009-shell-sidebar-palette-keys
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

# Shell: ⌘N on a focused row, the ⇧⌘S undo notice, the palette's project picks, the no-projects header and the snooze date

## Outcome

The window shell behaves as the reference in six places:
- ⌘N and ⇧⌘N work while a sidebar thread row has the focus.
- ⇧⌘S shows the sidebar's "Settled 1 thread, ⌘Z to undo" notice, and ⌘Z brings the thread back.
- The palette's "New thread in…" rows show ⌘1–⌘9, and those keys pick the project.
- With no projects, the sidebar header shows only Search and a disabled New thread.
- A project path in a palette row starts at its start and ends in an ellipsis.
- Custom snooze picks its date from a date button with a calendar popover.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| SH-1 | A click on a thread row gives the row the focus. ⌘N then opens the palette on "New thread in…" (2 projects). ⇧⌘N opens a new draft in the current project. `chat.new` and `chat.newLocal` are window handlers that ignore a focused non-editable element. | After a tap on the row (the focus is the row's NodeView), ⌘N opens nothing and ⇧⌘N does nothing; the header stays on the thread. `commandPending` is false and the `shortcut-chat.new` buttons exist. With the focus in the composer, ⌘N opens the picker. ⌘B and ⇧⌘P work with the row focused. | Main lane, 2 projects. Dismiss the toast. Click the "Timeline verification" row. Press ⌘N, then ⇧⌘N. Clone ops: `tap thread-verify-timeline`, `type t3-code key Meta+n`, `tree`, `type t3-code key Meta+Shift+n`, `tree chat-header`. | `target/t3-audit/evidence/shell/SH-1-ref.png`, `SH-1-clone.png`, `SH-1b-clone.png`, `SH-1-notes.txt` |
| SH-2 | ⇧⌘S on a thread moves it to Settled and shows "Settled 1 thread, ⌘Z to undo". ⌘Z (focus not in a field) brings the thread back. | ⇧⌘S moves the thread to Settled (1). No `sidebar-undo-notice` appears, and ⌘Z does not bring it back. The chord runs `chat:settle`, which dispatches `thread.settle` without the sidebar's `remember()` undo record that the row's Settle button uses. | Open "Timeline verification". Move the focus off text fields (clone: tap `toggle-thread-details` twice). Press ⇧⌘S. Expect the notice above the sidebar footer. Press ⌘Z. | `target/t3-audit/evidence/shell/SH-2-ref.png`, `SH-2-clone.png`, `SH-2-notes.txt` |
| SH-3 | Each "New thread in…" project row shows ⌘1, ⌘2, … (`thread.jump.N`, desktop build). ⌘1 in the page creates a draft in the first project. | The rows show no shortcut. ⌘2 in `palette-input` leaves the palette open. `palette.ts:236` assumes the served web build (`isDesktop` false); the reference desktop build is `isDesktop`. | ⌘K › "New thread in…" (or ⌘N with 2 projects). Look at the right edge of each row; press ⌘1. Clone ops: `type t3-code key Meta+n`, `type palette-input key Meta+2`, `tree`. | `target/t3-audit/evidence/shell/SH-3-ref.png`, `SH-3-clone.png`, `SH-3-notes.txt`, `pal-newin.aria.txt`, `SH-3-clone.tree.txt` |
| SH-4 | With no projects the header shows only Search and a disabled New thread. Filter and Add project are hidden (`hasProjects` false). | The header also shows "Filter threads by project" and "Add project". | Fresh T3 home. Finish the welcome with "Do not import projects". Compare the sidebar header row. | `target/t3-audit/evidence/shell/SH-4-ref.png`, `SH-4-clone.png`, `SH-4-ref.aria.txt`, `SH-4-clone.tree.txt` |
| SH-5 | "Lo… · /Users/daehyeonmun/orca/…/lanes/s…": the path starts at "/Users" and ends in an ellipsis. | "Lo… · s/daehyeonmun/orca/…/lanes/shell/…": the start of the path is cut (the first glyph half clipped) and an ellipsis still shows at the end. | ⌘K › "New thread in…" with long project paths. Compare the second line of each row. | `target/t3-audit/evidence/shell/SH-5-ref.png`, `SH-5-clone.png` |
| TH-8 | Custom snooze's date is an outline button ("Oct 9, 2026", calendar icon) that opens a calendar popover: past days disabled, the locale's week start. Duration mode's Unit select spans its column. | `input type="date"`: a stepper field that shows "10/ 9/202…" (the year clipped). The Unit select is narrower than its column. | Thread title menu › Snooze › Custom… (reference: sidebar row › Snooze › Custom…). | `target/t3-audit/evidence/thread/TH-8-ref.png`, `TH-8-clone.png` |

## Scope and exclusions

Included: the six findings above.

Excluded:
- SH-6 (the sidebar's "Check for updates"): blocked, [blocked-desktop-update-controls](20261009-blocked-desktop-update-controls.md).
- SH-7 (the palette field's focus ring): X61, waits for main fix of #302.
- SH-8 (View › Toggle Developer Tools): X2, a permanent declared difference.
- ⌘Z, ⌘W and ⌘O rows that the audit did not compare (they need real keys).
- Pixel differences (the user's rule: no pixel-perfect loops).
- Framework code. If SH-1's cause is in the host, record it for the coordinator and keep the row open.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`):
- SH-1: `routes/_chat.tsx:96-131` (window handlers for `chat.new` and `chat.newLocal`).
- SH-2: `components/ChatView.tsx:7627-7647` (settle with the undo notice).
- SH-3: `components/CommandPalette.logic.ts:234-244`, `CommandPalette.tsx:1867, 3051`, `CommandPaletteResults.tsx:197-241`.
- SH-4: `components/sidebar/SidebarThreadHeader.tsx:126-134`, `Sidebar.tsx:4839`.
- SH-5: `components/CommandPalette.tsx` `ProjectSearchDescription` (`truncate`).
- TH-8: `components/CustomSnoozeDialog.tsx` (the date button, the Calendar popover, the Unit select).

Clone (`examples/t3-code`):
- SH-1: `keyboard-dispatch.ts:151` (`paletteRows`), `app.contract:992-1015`, `settings-shortcuts.contract:51-55, 104-109`
  (`ShortcutButton`, hatch `t3-terminal-key`). The cause is not isolated. First find what takes ⌘N while the row has the
  focus (the row's own key handling, a native key monitor in `T3Sidebar.swift`, or the hatch). Other ShortcutButtons
  work with the same focus.
- SH-2: `keyboard-dispatch.ts:198` (`threadRows`: `chat:settle`), `chat-commands.ts:21-26`, `sidebar-commands.ts:127-129`
  (`remember()`). The chord must record the same undo step as the row's Settle button.
- SH-3: `palette.ts:233-245`. The clone is the desktop build, so it follows `isDesktop`.
- SH-4: `sidebar.contract` (the header buttons render always), `sidebar-view.ts:262` (`hasProjects`).
- SH-5: root cause X57 ([#291](https://github.com/ccheever/exact2/issues/291), main file
  `issues/20261009-apple-overflow-line-alignment.md`). The clone's pattern (task `pr-list-title-clip`): `text-align="left"`
  on the row button (`palette.contract:284`, rows at `209-216`).
- TH-8: `sidebar-overlays.contract:190`. `Intl.Locale` and its week info are in the data runtime (X36, main #204, adopted).
  Use the clone's menu-key pattern (`menu-keys.contract`) for the calendar's keys. Add no per-site flip arithmetic (X17,
  #112). The X52 note in `dialog-focus.test.ts` (date input Tab order) changes when the date becomes a button.

Shared files: `keyboard-dispatch.ts` is also changed by [model-picker-parity](20261009-model-picker-parity.md) (CO-4).
Merge one before the other starts, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| SH-1 | Focus a thread row, press ⌘N: the palette opens on "New thread in…". Press ⇧⌘N on a focused row: a new draft opens. Agent drive first, then real keys. | `sh1-row-cmd-n.png`, `sh1-row-shift-cmd-n.png` | agent, then needs_real_input (a real click on the row and real ⌘N/⇧⌘N) |
| SH-2 | ⇧⌘S shows `sidebar-undo-notice` with "Settled 1 thread, ⌘Z to undo". ⌘Z returns the thread to active. Unit test: the chord records the same undo step as the row's Settle. | `sh2-undo-notice.png` | agent |
| SH-3 | The "New thread in…" rows show ⌘1… ⌘9. ⌘2 in the palette opens a draft in the second project. Unit test on the row list. | `sh3-project-hints.png` | agent |
| SH-4 | With no projects, the header has Search and a disabled New thread only (tree). | `sh4-no-projects-header.png` | agent |
| SH-5 | A long path reads from its start and ends in "…". | `sh5-palette-path.png` | agent |
| TH-8 | Custom snooze shows a date button. It opens a calendar with past days disabled and the locale's week start. A pick sets the date. The Unit select spans its column. | `th8-snooze-date.png`, `th8-calendar-open.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build and unit-test. Then do one batched live drive at the end for every
row's before/after pair. Close every row in this PR, or record the blocker of a row that cannot pass.
