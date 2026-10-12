---
name: 20261012-adopt-main-fixes-r11
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r11'
pr_url:
verified_commit:
---

# Main adoption, round 11, and the workarounds its fixes end

## Outcome

`feat(example)/t3-code` takes current `main`. Round 10 ([adopt-main-fixes-r10](20261012-adopt-main-fixes-r10.md), #472)
adopted main `f084cc1c9`; the 2026-10-11 wave landed on the T3 branch after it (#470, #464, #465, #466, #468, #469, #479,
#474, #482), and the T3 tip is `20ce9dfdb`. Since `f084cc1c9` main has Charlie's #327 (`ad7ebfc2f`: native text, focus,
pointer routing, storage and popovers), LLP 1115.000's waves (among them `2a3d3fb82`, D7.9: the hover stays during a held
drag), LLP 1116.x, and the clone's framework fixes #477 (X58), #478 (`pan=` types its captures), #486 (X84), #487 (X85),
#490 (X79), #491 (X77), #493 (X81) and #494 (X82). The user decided on 2026-10-11 that every item below is built now,
matching the reference T3 Code (no further decision needed unless the reference itself is ambiguous).

| Id | Clone (the T3 tip `20ce9dfdb`) | What this round does (reference first) |
| --- | --- | --- |
| AD-1 | The branch lacks main's commits after `f084cc1c9` (259 at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge or fast-forward that keeps main's ancestry); main's framework, the clone's `examples/t3-code`; the clone adapted to whatever main renamed or now refuses |
| RT-1 | `STATUS.md` "Next up" 4 lists what #327 retires: X63 (#306's own Edit › Undo and Redo routing, `R8KeysMenus.swift`), X64 (the status-keyed redraws), X62 (the hover layer's rationale), X52 (the Custom snooze Tab cycle, `dialog-focus.test.ts`'s note), X51 (`press=popPress retainFocus=true` on the colour popover), X50 (`T3Storage.dataRoot(agent:)`), #282 (fix-keyboard-focus's focus plumbing for bugs 16, 6, 5), #292 (`T3MenuTurn`) | Each retired, or kept with why, against the reference's behaviour |
| RT-2 | X82: the pull request Timeline's commit row (`pages-pr-timeline.contract`) lays a transparent button over texts that sit beside it, only so the host's raster pump paints the title; the title stays selectable (followups-1012 FU-2) | The row is one `button` holding its marker and texts, as the reference's `CommitEvent` `<button>`; every title drawn at rest, after a scroll and when the tab is shown again; `search-key-and-timeline-commit-title.test.ts`'s TC-1 guard follows |
| RT-3 | X84, X85, X77: the shell's menu (`T3TextContextMenu`) answers `shellMenu` only for a fresh right-click or ContextMenu key, so a Control-click (#487) or VoiceOver's show menu (#486) on a reply's mailto, irc, xmpp or fragment link would open nothing; a Markdown table cell's link runs show the shell's Copy Link menu where the reference shows the app's link menu (X77) | Retire what main now does; the shell's menu answers a Control-click and VoiceOver's show menu as Electron's `context-menu` answers them; `EXACT2-GAPS.md` X84, X85, X77 say fixed on main (#486, #487, #491), adopted in round 11; the inline link's Tab-stop note stays (main issue open, Charlie's decision) |
| RT-4 | X58: a drive that measures a scroll across `prefer` makes one `prefer` first (pr-list-scroll-and-sidebar-sweep PS-1). X13: ThreadSection's sweep calls the root's half of the hover (`unhover`), because a child's action on `pan=` could not call the window's `sidebarHover` (its hidden captures were untyped; #468's record) | Retire both: the first `prefer` measured with nothing before it; the sweep calls the window's `sidebarHover` (`hoverCard`), as the reference's list's `pointer-events: none` sends the hovered row its leave |
| RT-5 | X79: the import wizard's popup sends Tab and Shift+Tab itself (`popupKeys`); X81: a copy across the diff rows on macOS includes the line numbers | X79: the popup's keys go; Tab and Shift+Tab from the popup reach its first stop and, through the trap's guard, its last, as Base UI's do. X81: the row says fixed on main by #493, adopted; no clone workaround to retire |
| RT-6 | X80's row says "none needed" and waits on main's issue | The row says decided by the platform rule: main closed the issue (`2a3d3fb82`, LLP 1115.000 D7.9; the clone's PR #492 closed): the hover stays where it was during a held drag (AppKit's rule) and follows the pointer at the release |
| RT-7 | `EXACT2-GAPS.md` X86 cites main PR #488 as open; `STATUS.md` still lists FU-1 in "Found, not in scope" | X86 cites main `issues/20261012-column-height-ignores-leaf-max-height.md` filed by #488 (merged; the issue open, a fix task running); the `STATUS.md` rows these touch; FU-1's "Found, not in scope" row removed (followups-1012 moved it) |

## Steps

1. Before: build this branch's bundle at the T3 tip `20ce9dfdb` before merging main and drive it once (lane
   `adopt-main-fixes-r11-before`, base port 16820: the clone's server on 16822).
2. `git merge --no-ff origin/main` at main's tip; main's side for the framework, the T3 side for `examples/t3-code/`.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
3. Check what the clone relies on in main's new commits (Contract refusals and renames, `contract build` warnings, the
   scripts the clone's tools call, rules and docs) and record each with its outcome ("Adopted main").
4. RT-1 to RT-6 against the reference source; Bun tests and AppKit tests for each source change.
5. `EXACT2-GAPS.md` (each row: fixed on main by #N, adopted in round 11, workaround retired or kept), `STATUS.md`'s rows
   these touch, `issues/README.md`'s mapping (RT-7).
6. After: this branch's bundle, the same drive (lane `adopt-main-fixes-r11`); before | after | reference images.
7. Merge the latest `origin/feat(example)/t3-code` (and main again if the two main fix tasks running now,
   x86-column-height-leaf-max-height and web-input-contextmenu, have landed), then the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 2 is empty | text |
| AD-1: the checks pass on the merged tree | CLAUDE.md's checks once on the final head | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, the macOS bundle; ONE agent-mode drive on the before bundle and on this branch's | before / after screenshots |
| RT-1 X63 | `macos/tests/r8-keys` (AppKit) against the before and after modules | before / after text |
| RT-1 X64 | the Bun guards; agent drive: a select's label shrunk by a change made elsewhere, `screenshot … window` | before / after images |
| RT-1 X62 | reading: the hover layer's comments and the X62 row | text |
| RT-1 X52 | agent drive: Custom snooze, Keyboard navigation on, Tab through the date and the duration screens (`state` focus) | before / after text |
| RT-1 X51 | agent drive: the theme editor's colour popover, a press on its plane over the editor's controls | before / after images and state |
| RT-1 X50 | `macos/tests/transport` (AppKit); after the drive, where the module's `t3-code.json` lies (the named store or the scratch tree) | before / after text |
| RT-1 #282, #292 | reading: main's issue files; kept with why | text |
| RT-2 | agent drive: PR #168 › Timeline at rest, shown again from Summary, after a 900-pt scroll, a live switch to dark; ⌘A from the lifecycle text; `search-key-and-timeline-commit-title.test.ts` | before / after / reference images |
| RT-3 | `macos/tests/contextmenu` (AppKit): a Control-click is the page click `shellMenu` answers; VoiceOver's show menu opens the shell's menu for the link it names; agent drive: a Control-click on a reply's mailto link, a right-click on a table cell's link (`t3.textmenu:` / `t3.contextmenu:` lines) | before / after text |
| RT-4 X58 | agent drive: a scroll node's id and offset across the session's first `prefer has-focus` | before / after text |
| RT-4 X13 | `contract build`; `sidebar-sweep.test.ts`; agent drive: a row-action sweep closes the row's card | before / after images |
| RT-5 X79 | agent drive: the import wizard's popup focused, Tab and Shift+Tab (`state` focus) | before / after text |
| RT-5 X81, RT-6, RT-7 | records | text |
| Real input | "Real-input batch steps" for `realinput-1011c` | — |

## Reference

- Method: [adopt-main-fixes-r10](20261012-adopt-main-fixes-r10.md) (Steps, "Adopted main", checks, Delivery).
- RT-1: `apps/web/src/components/ComposerPromptEditorTiptap.tsx` (the editor's own history: Mod-z / Mod-Shift-z),
  `apps/desktop/src/window/DesktopApplicationMenu.ts` (Edit's `undo` / `redo` roles act on the focused editor);
  `apps/web/src/components/settings/ProviderInstanceCard.tsx` (the status line), `components/ui/select.tsx`
  (`SelectValue`), `components/chat/ProviderModelPicker.tsx` (the trigger's label); `components/CustomSnoozeDialog.tsx`
  (Date button, Time input, Unit select: Tab stops in tree order); `components/ui/color-picker.tsx` (a press inside the
  popover stays in it); `components/ui/dialog.tsx` (Base UI Dialog); `apps/desktop/src/electron/ElectronMenu.ts`
  `showContextMenu`.
- RT-2: `apps/web/src/components/pullRequest/PullRequestTimelineTab.tsx` `CommitEvent` (lines 369-407): one `<button
  type="button" className="group relative mb-5 block w-full … pl-12 text-left …" aria-label="View commit <id>">` holding
  `ActorTimelineMarker`, the title (`truncate text-xs font-semibold text-foreground … group-hover:text-primary`), the sha
  and age, and `PullRequestDiffStat`.
- RT-3: `apps/desktop/src/window/DesktopWindow.ts` `installContextMenu` (lines 535-596: `contents.on("context-menu")`,
  Copy Link from `params.linkURL`, `electronMenu.popupTemplate({ window, template })` with no position, so at the
  pointer); `apps/web/src/components/ChatMarkdown.tsx` (the anchor's `onContextMenu` returns before `preventDefault`
  for a link with no web host); `apps/web/src/components/chat/externalLinkContextMenu.ts` (the app's link menu).
  Chromium sends Control-click as `contextmenu` and fires `contextmenu` at the element for VoiceOver's show menu (Blink
  `ShowNonLocatedContextMenu`).
- RT-4: `apps/web/src/routes/_chat.pull-requests.tsx` (`useLiveRefresh`: the list re-reads on the window's `focus`
  and keeps its rows); `apps/web/src/components/Sidebar.tsx` `startActionSweep` and `Sidebar.pointer.ts`
  `SidebarPointerSensor` (past 6 px the list takes `**:pointer-events-none`, so the hovered row hears its leave).
- RT-5: `apps/web/src/components/settings/BrowserImportWizard.tsx` (Base UI Dialog, `restoreFocus: "popup"`; the
  FloatingFocusManager's guards lie before and after the popup, so Tab from the popup reaches its first stop and
  Shift+Tab the guard that wraps to its last); `@pierre/diffs` (`[data-column-number]` is `user-select: none`).
- Main: `issues/closed/` files of X50, X51, X52, X62, X64 (#327), X58 (#477), the `pan=` captures (#478), X77 (#491),
  X79 (#490), X80 (`2a3d3fb82`), X81 (#493), X82 (#494), X84 (#486), X85 (#487); open:
  `issues/20261009-macos-textarea-native-undo.md` (X63, its iOS part), `issues/20261009-gui-dialog-action-commands.md`
  (#282), `issues/20261009-macos-menu-stalls-native-work.md` (#292), `issues/20261012-macos-inline-link-run-tab-stop.md`,
  `issues/20261012-column-height-ignores-leaf-max-height.md` (X86).

## Next action

Build the before bundle at `20ce9dfdb`, drive it, merge main.
