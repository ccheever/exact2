---
name: 20261012-adopt-main-fixes-r11
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r11'
pr_url: https://github.com/ccheever/exact2/pull/497
verified_commit: 1f813789b
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
  (#282), `issues/20261009-macos-menu-stalls-native-work.md` (#292), `issues/20261012-macos-inline-link-run-tab-stop.md`;
  X86's `issues/closed/20261012-column-height-ignores-leaf-max-height.md` (filed by #488, fixed by #496 during this task).

## Adopted main

**`b610c88ba`**, then **`c4cd2bcba`**, then **`b0e4a6c22`** (main's tip at the last merge, 281 commits past round 10's
`f084cc1c9`):

- `cae99e9d7`: this record's first version.
- `6b6446e97`: `git merge --no-ff origin/main` at `b610c88ba` (#327, #477, #478, #486, #487, #488, #490, #491, #493, #494,
  LLP 1115.000/1115.002 and 1116.004–1116.006). The one conflict was `Cargo.toml`'s `members` line (main added
  `apps/shelf/android` and `apps/unsaid/android`, the T3 branch carries `examples/t3-code/macos`): main's line plus the
  clone's member. `Cargo.lock` merged clean. Before any clone change: `contract build` exit 0.
- `4cabf32fc`: the retirements (RT-1 to RT-5); `6297a1728`: the records (`EXACT2-GAPS.md`, `STATUS.md`, `issues/README.md`)
  and a test guard that named a retired keyed label. The bundle of these sources was the live drive's.
- `a793030ba`: `git merge --no-ff origin/main` again at `c4cd2bcba`, after the live drive: the two main fix tasks that ran
  beside this one landed (#496, X86: a flex column takes its leaves clamped by max-height; #495, the web's `contextmenu`
  from a form control), with LLP 1115.000's Mac text follow-ups (`6a7f16c6f`: a right-click on selected page text shows
  AppKit's own menu; `99a0ff7e3`, `a63a217eb`, `ccbedd1c5`: selection ink and text composition; `883c7300d`: the focus
  ring). No conflict. Then: X86's box retired and the shell's menu over selected text adapted to `6a7f16c6f` (Cause and
  fix), with `adopt-main-fixes-r11.test.ts`.
- `86dcbfd88` (review round 1): `git merge --no-ff origin/main` at `b0e4a6c22` (the terminal's animation payloads and colour
  escapes: `host/terminal`, `apps/harness`; nothing the clone uses). No conflict.

On the final head after a fresh fetch: `git merge-base --is-ancestor origin/main HEAD`, `git rev-list --count
HEAD..origin/feat(example)/t3-code` and `git diff origin/main HEAD -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
(Checks). The `Cargo.toml` / `Cargo.lock` difference is the `examples/t3-code/macos` member and the `t3-code-macos`
package only. The PR must land as a merge (or a fast-forward of the feature branch to its head), never a squash: a squash
drops main from the branch's ancestry.

## Each item of main's new commits

| Item | Main commit | Outcome | Clone change |
| --- | --- | --- | --- |
| Contract refusals and renames (`efe10e0c1` number-only style rows, `ad3d5695d`, `b91aceb51` `lower-level-without-heading`, `e5061587c` `lower-back-twice`) | various | `contract build` exit 0 on the merged tree before any clone change: 6,471 slots, 120,814 nodes (110,196 at round 10: main's lowering), 48 resources, 33,200,547 bytes; nothing refused | none forced |
| New `contract build` warning `lower-dialog-role` (LLP 1116.004 D4: `role="dialog"` on a box names a dialog without making one) | `8aef9a509` | 73 (20 shown: the printed list's cap, `contract_lower::MAX_WARNINGS`), on the clone's popovers and hover cards that take Base UI's `role="dialog"` (the reference's PopoverPopup and Dialog popups); a warning, nothing refused. Not changed here: the reference's popups are role-dialog boxes, and turning them into `dialog` elements or routes is a design change beyond an adoption. They fill the cap, so `analyze-read-after-assign`, `lower-count-plural` and `type-performance-now-persisted` no longer reach the printed list. **Review round 1, uncapped:** a compiler built with the cap lifted (the constant changed in the worktree for one build into a private target dir and restored at once; nothing committed) on this branch's `app.contract` and on the T3 tip's: the same lists, 21 `analyze-read-after-assign` at 13 sites, 8 `lower-count-plural`, 1 `type-performance-now-persisted`, 73 `lower-dialog-role`, 233 colours. This round adds no finding. Of the 21, 20 are the sites round 10 read (renamed since; the old value read on purpose, exclusive `if` arms, or a slot already 0 at a drag's start); the one since round 10, `jumpRequest` (`app.contract:718`/`723`, legacy-sidebar-full #469), is two exclusive arms (a message equal to `sidebar:new-thread` or `pr-handoff-next` never starts with `sidebar:opened`), each a counter bump: no bug. The 8 plural counts are the reference's own words (DiffStatLabel's `${additions} additions`, OpenSourceLicenses' `notices`, terminalCloseConfirm's `terminals`). `STATUS.md` round 11 asks each round to read the analysis uncapped ([record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §3) | none |
| `lower-colour-one-scheme` | — | 233 (230 at round 10; the count of literal colours the lint sees) | none |
| X63, X64, X62, X52, X51, X50; #282 and #292 partial | `ad7ebfc2f` (#327) | RT-1 | RT-1 |
| X82 | `c43abdc3a` (#494) | RT-2 | RT-2 |
| X84, X85, X77 | `700943aa5` (#486), `88d02ff2a` (#487), `53b444af0` (#491) | RT-3 | RT-3 |
| X58; `pan=` captures | `877f9149d` (#477), `c45d6e4af` (#478) | RT-4 | RT-4 |
| X79, X81 | `db8a9ddb9` (#490), `957250523` (#493) | RT-5 | RT-5 |
| X80 | `2a3d3fb82` (LLP 1115.000 D7.9) | RT-6 | records |
| X86 | `db7a98a02` (#488, an issue file), `97803b277` (#496, the fix, second merge) | RT-7; fixed by #496 | the citation comment's 4rem–10rem box retired |
| A right-click on selected page text shows AppKit's own menu (second merge) | `6a7f16c6f` | ExactKit's menu there is now AppKit's for a read-only text view holding the selection (`SelectionMenuText`, TextMenuMac.swift); the shell's menu module knew ExactKit's by its Look Up item (`lookUpSelection:`), which it no longer has, so a real right-click on selected page text would have shown AppKit's menu (Translate, Search With, Share, Writing Tools, Services) where the reference shows the shell's | `T3TextContextMenu.isHostTextMenu`: an item aimed at that text view; `text-menu.swift`'s stand-in follows |
| Text composition, selection ink, the focus ring's border (second merge) | `a63a217eb`, `ccbedd1c5`, `99a0ff7e3`, `883c7300d` | platform rules (LLP 1115.000 D7.9, LLP 1115.001 D8); the composer is the host's textarea, so a Korean composition now hears `input` at each change and a value the app writes waits for the composition's end | none (real input would show it) |
| macOS platform rules (LLP 1115.000 D7, D8): Tab honours Keyboard navigation (`f25326e4d`), a click on a button or empty ground keeps the first responder (`11ac0f368`), Space and Return (`ff77345d0`), NSAlert for an alert-shaped confirmation (`46a02a57c`), unsaid `autocorrect`/`spellcheck` (`6c5610c46`), the caret (`7c226e8ac`), AppKit scrolling and scroll keys (`81a41f114`, `fc90b0b82`), selection colour, I-beam and Shift-click (`ca5344b41`, `49490ae20`, `21b615090`), a pressable's pressed effect (`e2b8f3ccb`), a background window's click and hover (`2a3d3fb82`) | various | Taken as the platform's (LLP 1115). Under the agent Keyboard navigation is off from launch, so a button is no Tab stop unless `prefer keyboard-navigation true` (the drive's Tab cycles say it, after only: the T3 tip's agent refuses the fact); the clone has no `dialog` element, so NSAlert takes none of its confirms; the agent's window is never key and its clicks all count (`NodeView.everyClickCounts`), so the drives are unchanged by `2a3d3fb82` | none |
| A sixth check: `cargo check --workspace --all-targets --keep-going` | `a59700afe` | run with the checks (it compiles the clone's `t3-code-macos`) | none |
| iOS, Android, web, Linux, LLP docs | various | nothing on the Mac path the clone uses | none |
| The terminal's animation payloads and colour escapes (review round 1's merge) | `b0e4a6c22` | `host/terminal`, `apps/harness`; the clone does not use them | none |

## Cause and fix

- **RT-1 X63 (#327, retired).** The reference's Edit › Undo and Redo are Electron roles that act on the focused editor's
  history, and its renderer binds ⌘Z to thread.undo only `when: "!terminalFocus && !editableFocus"`. Exact's textarea keeps
  its own undo manager; before #327 `undo:` and `redo:` sent up the responder chain reached `NSWindow` (the window's
  manager), so the clone's `R8KeysMenus` took Edit › Undo and Redo and undid the focused text view's own history. #327 makes
  the textarea answer `undo:`, `redo:`, their validation and ⌘Z/⇧⌘Z itself. Now Edit › Redo is the host's own item again,
  and Edit › Undo (still the clone's, for thread.undo with no editable focus) sends `undo:` up the focused text's responder
  chain and validates it there. `macos/tests/r8-keys`: the T3 tip's module and test pass (6/6); this branch's pass (6/6);
  this branch's module with a pre-#327 textarea stand-in (no `undo:`/`redo:` of its own) fails 4 of 6 (⌘Z and Edit › Undo
  do nothing), which is why the routing stood and why it can go now. Main keeps its issue open for iOS.
- **RT-1 X64 (#327, retired).** A wrapped paragraph that shrank below the text-raster size kept painting its old raster;
  the clone keyed the provider row's status, the editor's status line, the model trigger's label (by words and size),
  the scheduled task editor's Runs on and Base branch labels and every `SettingsSelect` label, so each new value mounted a
  new node (the last three for X82's pump too). #327 retires the old ink and #494 paints an unselectable label; each is a
  plain text again, as the reference's are.
- **RT-1 X62 (#327, rationale only).** Hover is now the hit's ancestor path with overflowing descendants. The window's hover
  layer stays: it also fixes the clipping (#251, #248) and is the reference's portal. The structures X62 shaped (a painted
  menu's rows reporting the pointer, the Usage popovers drawn in the page's scroll content, the floating player's pill, the
  sidebar section with no hover of its own) work and stay; their comments say X62 as history.
- **RT-1 X52 (#327).** Date, time and select are default Tab stops; with LLP 1115.000 D7.2 every control is a stop only with
  Keyboard navigation on. Custom snooze's toggle, close and trap handlers (`toggleKey`, `closeKey`) need no change; re-driven
  (below). `dialog-focus.test.ts`'s X52 note follows.
- **RT-1 X51 (#327, retired).** A plain contact inside a popover stops at its top layer, so the colour popover's own
  `press=popPress retainFocus=true` (and `popPress`) is gone. `popPress` also set `mouse` (the plane's and hue's focus ring
  hides after a pointer); the plane and hue presses set it themselves, and a press on the popover's plain parts moves no
  focus to either.
- **RT-1 X50 (#327, retired).** A module's context now holds the session's named roots under `--storage`, so the clone's
  `T3Storage.dataRoot(agent:)` (which re-rooted the module's files under `TMPDIR/<the scratch tree's name>/data` in agent
  mode, apart from the named store and never emptied by a fresh one) is gone; the module uses `context.data` everywhere.
  `macos/tests/transport` drops the re-rooting's test.
- **RT-1 #282 (kept).** Main's `issues/20261009-gui-dialog-action-commands.md` is open: main has `showModal(id)`/`close(id)`
  since `b968df0d8` (which #327 took in place of its own), not the invoker modal semantics (inertness, focus, Tab, Escape,
  close events) the clone's state-mounted dialogs would need. fix-keyboard-focus's plumbing for bugs 16, 6 and 5 stays.
- **RT-1 #292 (kept).** #327 opens the host's context and button menus on a common-mode run-loop turn; `T3MenuTurn` does the
  same for the module's own NSMenus (`T3ContextMenu`), which the host does not open, and main keeps #292 open for its
  full-app cadence check.
- **RT-2 X82 (#494, retired).** The reference's `CommitEvent` is one `<button>` holding the marker, the title, the sha and age
  and the counts. TC-1 had laid a transparent button over texts beside it (the host's raster pump asked only selectable texts
  again, and no text under a button is one), and FU-2 kept the title selectable. #494 builds the pump's index from every shown
  paragraph and asks again when both raster workers are busy. The row is the reference's one button again; nothing is
  `aria-hidden` or `user-select` by hand, and its texts are unselectable as a button's content is on the Mac (LLP 1115 D8).
- **RT-3 X85 (#487), X84 (#486), X77 (#491).** The shell's menu module (`T3TextContextMenu`) found the clicks Electron's
  `context-menu` answers by watching right mouse-downs only. Now `isSecondaryClick` is a right-click or a Control-click: the
  monitor takes one in a text input or over selected text (AppKit's own editing or text menu would open) and records one on
  the page as the page click `shellMenu` answers (#487 runs a node's `contextmenu` for it), and the tail answers a
  Control-click a node that is not pressable passed up the chain (`mouseDown`), as it answers an unanswered right-click.
  VoiceOver's show menu (#486) runs a node's `contextmenu` with no event, so a `shellMenu` request with neither a fresh click
  nor a ContextMenu key behind it is that request while VoiceOver or Switch Control is on (`assistive`): the shell's menu
  for the link it names, at the pointer, as DesktopWindow pops its template with no position (`assistiveMenu`). X77 needed no
  clone change: a table cell's link runs already wrote `contextmenu=linkMenu(…)`, and #491 runs it. `FlowRuns` stays (it
  also boxes inline code). The inline link's Tab stop (main `issues/20261012-macos-inline-link-run-tab-stop.md`) stays open.
- **RT-4 X58 (#477), X13 / `pan=` (#478).** X58 had no clone code: a drive made one `prefer` first. #477 applies the agent's
  light at launch, so the first `prefer` names only what it names (before/after below). #478 types a `pan=` action's hidden
  captures, so ThreadSection's sweep calls the window's `sidebarHover` (`hoverCard`), the same leave a pointer leaving the
  row sends (it counts the hover and calls the root's half), instead of the root's half handed down as `unhover`, which is
  no prop now. `contract build` accepts it.
- **RT-5 X79 (#490, retired).** The reference's wizard (Base UI Dialog, `restoreFocus: "popup"`) puts the focus on the popup
  after a step change, and the FloatingFocusManager's guards lie before and after the popup, so Tab from it reaches its first
  stop and Shift+Tab the guard that wraps to its last. The clone's guards lay inside the popup, and the popup's own `key`
  handler sent Tab and Shift+Tab itself because the Mac had no Tab from a box that is no stop. With #490 the guards move
  beside the popup, as Base UI's, and `popupKeys` (and its `held`) go; the popup still tells the root its focus (`track`).
  **X81 (#493):** no clone workaround; a copy across the diff rows leaves the line numbers out.
- **RT-6 X80 / RT-7.** Records only.
- **X86 (#496, second merge), retired.** The citation comment's textarea (the reference's `field-sizing-content min-h-16
  max-h-40`) sat in a column held at 4rem to 10rem with `overflow: hidden`, because the column's automatic height took the
  textarea's whole content height. #496 clamps a leaf's flex base size by its min and max (four places in the vendored
  Taffy), naming this popover. The textarea is the popover's own child again. Not driven after the merge.
- **The selected-text menu (`6a7f16c6f`, second merge), adapted.** `replacement(for:)` asks the hit view's `menu(for:)` and
  replaces it with the shell's menu only when it is ExactKit's text menu. That menu is now AppKit's own, aimed at a hidden
  read-only text view (`SelectionMenuText`); the module recognises it by an item (or a submenu's) aimed at that view instead
  of the old `lookUpSelection:` item. Under the agent ExactKit gives no such menu, so the click goes on to the tail's page
  menu (Copy enabled by the selection), as before; under real keys it is a real-input step.

## Acceptance results

Drives: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/6c0502ed3bcc250a480dedec573ca24f71203731/adopt-main-fixes-r11/drive.sh.txt) and [lane.sh](https://raw.githubusercontent.com/ccheever/exact2/d2a41a6c23eb7d48c73f438e12b0a1dc205a240a/adopt-main-fixes-r11/lane.sh.txt); readouts [before](https://raw.githubusercontent.com/ccheever/exact2/a8b0044ec5201b79122085694840ac3f6950245c/adopt-main-fixes-r11/readouts-before.txt) and [after](https://raw.githubusercontent.com/ccheever/exact2/a6615e1d55d3cb9bbd20d3540f2fbfbe74d5cc81/adopt-main-fixes-r11/readouts-after.txt).

| Row | Result | Proof |
| --- | --- | --- |
| AD-1: the branch contains main | pass at the final merge (main `b0e4a6c22`, review round 1) | Adopted main; Checks |
| AD-1: the framework tree is main's | pass: empty after `6b6446e97`, `a793030ba`, `86dcbfd88` and on the final head | Adopted main; Checks |
| AD-1: the checks pass | see Checks | Checks |
| AD-1: the clone works | pass: the same three sessions before (the T3 tip `20ce9dfdb`'s bundle) and after (the bundle of `6297a1728`'s sources on main `b610c88ba`) exit 0, no op failed; screens equal but for the rows' own changes and main's platform rules (the launch scheme, X58; the launch focus in the first key view, LLP 1115.000 D7.5) | [shell](https://raw.githubusercontent.com/ccheever/exact2/8c79a05e63f94d098d95cc6074884e3145ea0c88/adopt-main-fixes-r11/ad1-shell.png), [Settings](https://raw.githubusercontent.com/ccheever/exact2/71739d0042f659834626ce4a822e3bc886fbfeb7/adopt-main-fixes-r11/ad1-settings.png), [pixels](https://raw.githubusercontent.com/ccheever/exact2/901987fc6fcb5d8fd05a1a975b105bdf4048d912/adopt-main-fixes-r11/ad1-pixels.txt) |
| RT-1 X63 | pass: `macos/tests/r8-keys` before 6/6, after 6/6; the retired routing over a pre-#327 textarea fails 4 of 6 (⌘Z and Edit › Undo do nothing), so it stood only for want of #327. Real keys: step 4 | [record](https://raw.githubusercontent.com/ccheever/exact2/f466379af15c628b096ad3b48b6d223193f8015a/adopt-main-fixes-r11/x63-and-appkit-record.txt) |
| RT-1 X64 | pass: the keyed texts are plain; the scheduled task editor's and Settings' selects, the provider row and editor, the model trigger keep their words (Bun guards). Review round 1's drive, with a second environment "Studio" (no provider) paired: a new scheduled task, Runs on › Studio rewrites Runs on (the Mac's name → "Studio") and Project ("Verification fixture" → "studio-app", both shorter) and Base branch ("From main" → "From origin/main"), then Runs on › the Mac back again; each label is drawn whole at its new width, before (keyed) and after (plain) alike, the typed name kept; the tree reads the same labels on both. The provider row's status and the model trigger need a provider (Bun guards only) | [image](https://raw.githubusercontent.com/ccheever/exact2/d1a62098a7e0542ba486da64a0b5d92cbe0ff420/adopt-main-fixes-r11/rt1-x64-runs-on.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §1 |
| RT-1 X62 | kept, rationale only: the hover layer stays (clipping, the reference's portal) | Cause and fix |
| RT-1 X52 | pass (Keyboard navigation on, after only): date screen Tab: toggle → Date → **Time** → Cancel → Snooze → X → toggle; duration: toggle → Decrease → amount → Increase → **Unit** → Cancel (the reference's order; before: Time and Unit skipped) | [image](https://raw.githubusercontent.com/ccheever/exact2/54fdf23e450be2bf9eaca699374d5f538a74a2c2/adopt-main-fixes-r11/rt1-x52-snooze.png), readouts |
| RT-1 X51 | pass: a press on the accent plane over the editor stays in the popover (it stays open, the accent changes, nothing beneath pressed); the focus then on the popover as the reference's (Chromium focuses the PopoverPopup), where before `retainFocus` kept it on the saturation slider | [image](https://raw.githubusercontent.com/ccheever/exact2/c40c2bf5fe6ea50e79664e3b5b1c512531090e16/adopt-main-fixes-r11/rt1-x51-plane.png), [record](https://raw.githubusercontent.com/ccheever/exact2/4779f9d88280eb2c15241d64175c0680881dfcd3/adopt-main-fixes-r11/rt1-x51-record.txt) |
| RT-1 X50 | pass: before, each session's `t3-code.json` was in a scratch tree per process under `TMPDIR` and its named store held nothing; after, in each session's named store (`~/Library/Caches/exact/com.exact.t3code.macos/agent/<store>/data`). The kept pull request detail and U6 relaunch rows were not re-run (a relaunch is a second launch) | [record](https://raw.githubusercontent.com/ccheever/exact2/deacec1ac0bdb3def3c1441e1792126ce09377df/adopt-main-fixes-r11/x50-files.txt) |
| RT-1 #282, #292 | kept, with why (main's issues open) | Cause and fix |
| RT-2 | pass: each commit row is one button holding its marker and texts (the tree: before a column with an overlay `Pressable`, after the `Pressable` holding them); every title drawn at rest, shown again, after a 900-pt scroll and in dark; those four screens equal the before's but for 212 px of a sidebar glyph. ⌘A: not a match. The reference highlights exactly the shown texts. Both builds also paint boxes that match no Timeline text: they are the hidden Summary tab's texts (Reviewers, the description, the comments' headers and bodies; review round 1's drive shows the Summary again with the same selection on them). Both tabs stay mounted under `visibility: hidden`, as the reference's, and the macOS host paints the selection of hidden text. This is a host difference, not this round's change, listed in `STATUS.md` "Known differences" | [rest](https://raw.githubusercontent.com/ccheever/exact2/0e2e9414b53d8c41dbadd81ffa6f22967a46a951/adopt-main-fixes-r11/rt2-timeline-rest.png), [shown again](https://raw.githubusercontent.com/ccheever/exact2/b79f173c5f741194d44962b0be1a1482b52a37bc/adopt-main-fixes-r11/rt2-shown-again.png), [scrolled](https://raw.githubusercontent.com/ccheever/exact2/2e31101af67a35c495c845ef6caf853a704e0c7b/adopt-main-fixes-r11/rt2-scrolled.png), [dark](https://raw.githubusercontent.com/ccheever/exact2/150106009f16d33da09bc3e35b805b0fca6a3684/adopt-main-fixes-r11/rt2-dark.png), [dark top](https://raw.githubusercontent.com/ccheever/exact2/5013cf45242c0ae1e026e536d35d6f79dc28da12/adopt-main-fixes-r11/rt2-dark-top.png), [select all](https://raw.githubusercontent.com/ccheever/exact2/5f8ee34d6d1ec3d8f6013ded65cd4bf9a3ded076/adopt-main-fixes-r11/rt2-select-all.png) (the titles take no highlight, as a button's content; the other boxes are the hidden Summary tab's texts, before and after alike: [image](https://raw.githubusercontent.com/ccheever/exact2/e71ba5824a00cbba6b413ba8f912a70fbfa75f9d/adopt-main-fixes-r11/rt2-select-all-owner.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §2, a known difference), [tree](https://raw.githubusercontent.com/ccheever/exact2/17fdba0313c8ba94d6ee9b4fe9c0ca54a0ddee67/adopt-main-fixes-r11/rt2-commit-row-tree.txt) |
| RT-3 X85 | pass: a Control-click on the reply's mailto link ("the team"): before nothing (a modified press), after the shell's menu `t3.textmenu: Cut (disabled) \| Copy (disabled) \| Paste (disabled) \| Select All`, the reference's for a mailto link; AppKit: the T3 tip's module fails 3 of the 4 Control-click tests, this branch's passes them | [thread](https://raw.githubusercontent.com/ccheever/exact2/1a0922868fb39ad0d2de026cacfa603062e387c9/adopt-main-fixes-r11/rt3-thread.png), [log](https://raw.githubusercontent.com/ccheever/exact2/a6615e1d55d3cb9bbd20d3540f2fbfbe74d5cc81/adopt-main-fixes-r11/readouts-after.txt), [AppKit](https://raw.githubusercontent.com/ccheever/exact2/5779fe6b9ff8bd7752e54e2cc70369eb7fa34dd4/adopt-main-fixes-r11/rt3-appkit-record.txt) |
| RT-3 X77 | pass (after only: the T3 tip's agent activates a run on `tap <run> contextmenu`): a right-click on the table cell's web link ("the site") opens the app's link menu `t3.contextmenu: Open in integrated browser \| Open in system browser \| Copy Link`, as the reference's `externalLinkContextMenu`; X77's record has the before (the shell's Copy Link menu) | [log](https://raw.githubusercontent.com/ccheever/exact2/a6615e1d55d3cb9bbd20d3540f2fbfbe74d5cc81/adopt-main-fixes-r11/readouts-after.txt) |
| RT-3 X84 | pass (AppKit): with VoiceOver on (the switch injected), a `shellMenu` request no click or key raised opens the shell's menu for the link it names, at the pointer; off, nothing; a fresh click still wins. VoiceOver itself: step 5 | [AppKit](https://raw.githubusercontent.com/ccheever/exact2/5779fe6b9ff8bd7752e54e2cc70369eb7fa34dd4/adopt-main-fixes-r11/rt3-appkit-record.txt) |
| RT-4 X58 | pass: before, launch `prefersColorScheme` dark in an Aqua window, and the first `prefer has-focus` remounted the Usage page at its top (scroll node 4738 → 6046, scheme arm 0 → 1); after, light from launch and the same node where it was | [image](https://raw.githubusercontent.com/ccheever/exact2/145b7e907cd7c91702bddfdf90e41f33ab166edd/adopt-main-fixes-r11/rt4-x58-usage.png), readouts |
| RT-4 X13 | pass: `contract build` accepts `hoverCard(hoverId, false)` from the `pan=` action; the drive's sweep closes the card and arms Sweep four and three, the release settles 2, as before | [image](https://raw.githubusercontent.com/ccheever/exact2/8fbb2857a651cfeabfad295952feec8071954df7/adopt-main-fixes-r11/rt4-x13-sweep.png) |
| RT-5 X79 | pass: from the focused popup (after "I've quit it"), Tab → Cancel and Shift+Tab → the X, before (`popupKeys`) and after (the host and the guards beside the popup), with Keyboard navigation off (the guards are stops by their `tabindex=0`) | [image](https://raw.githubusercontent.com/ccheever/exact2/e8be26ed553cfeed9fd38ac23796f5f5b5961357/adopt-main-fixes-r11/rt5-x79-wizard.png), readouts |
| RT-5 X81, RT-6, RT-7 | records | `EXACT2-GAPS.md`, `STATUS.md`, `issues/README.md` |
| Second merge: X86 retired, the selected-text menu adapted | Bun guard and AppKit `contextmenu` (70/0); not driven (the live drive came before the merge; review round 1's drive ran the merged bundle, through Settings, Scheduled Tasks and a pull request, but not these two: steps 3 and 8) | Tests |

Drives (agent mode, 1280×840, `--epoch 2026-10-12T17:00:00+09:00 --time-zone Asia/Seoul`, base port 16820, the clone's server
on 16822; each of the three sessions on a lane reset to the same seed: the audit seed, reply-links-reference-behavior's
fixture reply, five settleable threads, the pages lane's usage data, the GitHub lane's playground clone, the import wizard's
fixture browser home, the provider CLIs pinned to an absent path): before on this worktree at `20ce9dfdb`'s bundle (lane
`adopt-main-fixes-r11-before`), after on the bundle of `6297a1728`'s sources (lane `adopt-main-fixes-r11`). Two ops are
after only (the T3 tip refuses or mishandles them): `prefer keyboard-navigation true` (D7.2 is main's) and the cell link's
`tap … contextmenu`; session 1 reads `logs` once more after the Control-click (read-only). Reference: T3 Code 1e2ecbd975
Electron on lane `adopt-main-fixes-r11-ref` (16820/16821) over CDP.

Review round 1 drive (agent mode, 1280×840, the same facts and lanes; [script](https://raw.githubusercontent.com/ccheever/exact2/0497056558f2cb81307dc7cf781804c57cb6ab77/adopt-main-fixes-r11/drive-x64.sh.txt), [Studio](https://raw.githubusercontent.com/ccheever/exact2/6e52be9491849c0826141a3f9876e03e5368cb3c/adopt-main-fixes-r11/studio.sh.txt)): a second
environment "Studio" (a T3 0.0.46 server on 16823 with its own home, no provider CLI, a `scutil` shim for its label, the project
studio-app with main, develop and release/2026-10) paired through Settings › Connections with a fresh link each drive, as
scheduled-tasks-followups ST-1 had it. Before: the T3 tip `20ce9dfdb`'s bundle in the integration worktree `t3-int` (at
`dba2285f6`, the same tree; driven only, not built or edited), since this worktree's before bundle was rebuilt for the live
drive. After: this branch's bundle of `86dcbfd88`'s sources (main `b0e4a6c22`). The Nightly notice lies over Scheduled Tasks'
New task button, so both dismiss it first. Both sessions exit 0.

## Real-input batch steps

For `realinput-1011c` (the coordinator's session). Launch this branch's bundle as a lane copy (its own bundle id, a lane home
outside any git checkout) seeded as `lane.sh` seeds it (the fixture reply, the playground clone with the GitHub lane's
login), window 1280×840. Never send a message, never change System Settings.

1. **RT-3 X85.** Verification fixture › the reply: Control-click "the team" (mailto): the shell's menu Cut, Copy, Paste, Select
   All (no Copy Link). Control-click "the site" in the table: the app's link menu (Open in integrated browser, Open in system
   browser, Copy Link). Control-click an empty part of the timeline: the shell's menu. Control-click a word in the composer:
   the shell's text menu (no Services, AutoFill or Writing Tools).
2. **RT-3 X77.** Right-click "the site" in the table: the app's link menu (before: the shell's Copy Link menu).
3. **Selected text (`6a7f16c6f`).** Select a sentence of the reply by dragging, right-click on it: the shell's menu (Cut
   disabled, Copy, Paste disabled, Select All), not AppKit's (Look Up, Translate, Search With, Share, Writing Tools).
4. **RT-1 X63.** Type in the composer, ⌘Z (undone), ⇧⌘Z (redone); Edit › Undo and Edit › Redo from the menu bar do the same.
   With nothing focused after archiving a thread, ⌘Z restores it (thread.undo).
5. **RT-3 X84 (optional).** VoiceOver on (⌘F5), the VoiceOver cursor on "the team", VO-⇧-M: the shell's menu; on a thread row:
   its app menu. VoiceOver off.
6. **RT-2.** Pull Requests › #168 › Timeline, the pointer away: every commit title drawn; scroll to the end and back; Summary
   then Timeline; the lane app's Appearance › Dark and back: drawn each time; a title turns blue under the pointer; a click
   opens the Code tab on that commit; a drag across two rows selects nothing.
7. **RT-1 X51.** Settings › Appearance › Create theme › the accent swatch; click and drag on the plane where the editor's
   controls lie beneath: the colour follows, the popover stays, nothing beneath changes.
8. **X86.** Select text in a reply, Cite, type or paste about 8,000 characters into the comment: the popover stays about
   230 pt tall and the comment scrolls inside it.
9. **RT-5 X79 (optional).** Settings › Integrations › Add profile › a browser that is open: after "I've quit it" the popup
   holds the focus; Tab: Cancel; again after "I've quit it", Shift+Tab: the X.

## Tests

- `adopt-main-fixes-r11.test.ts` (new, 6): X51 (no press or retainFocus on the colour popover), X50 (no `T3Storage`, the
  context's data root), X63 (no text history or Redo of the clone's own), RT-3 (the secondary click in the monitor, the
  replacement and the tail; the assistive switch; the selected-text menu known by its text view), X86 (the textarea straight
  in the popover).
- `search-key-and-timeline-commit-title.test.ts`: TC-1's guard is the reference's one button holding its texts (RT-2).
- `providers.test.ts`, `realinput-1010c-fixes.test.ts`, `scheduled-tasks-followups.test.ts`, `settings-labels.test.ts`: the
  plain texts (X64).
- `sidebar-sweep.test.ts`, `pr-list-scroll-and-sidebar-sweep.test.ts`, `contract-action-fixture.ts`: the sweep calls
  `hoverCard` (X13).
- `dialog-trap.test.ts`, `import-wizard-initial-focus.test.ts`: the wizard's guards beside its popup, no `popupKeys` (X79).
- `dialog-focus.test.ts`: the X52 note. Comment-only: `hover-layer.test.ts`, `menu-keys.test.ts`.
- AppKit: `macos/tests/r8-keys` (the Undo test over #327's textarea), `macos/tests/contextmenu` (`secondary-clicks.swift`,
  new, 6; the shell tests pin `assistive` off; `text-menu.swift`'s stand-in is main's menu), `macos/tests/transport` (the
  re-rooting's test removed).

## Checks

Review round 1, once, on `3eb37c322` (the review's head: main `b0e4a6c22` merged, records only besides; the next commit
changes this record only), `target/r11/checks-review.sh`. The change touches no clone Rust or Swift, so the
`t3-code-macos` lib test and the AppKit binaries were not re-run (their round 11 results stand, below).

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,846 pass, 1 skip, 0 fail (327 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,470 slots, 120,810 nodes, 48 resources, 33,303,262 bytes; 233 `lower-colour-one-scheme`, 20 `lower-dialog-role` shown (the cap; uncapped in "Each item"). `app.contract` 1,453 lines |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,946 passed, 0 failed, 36 ignored (99 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `cargo check --workspace --all-targets --keep-going` (main's sixth check) | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | the review drive's after bundle, `86dcbfd88`'s sources |

After a fresh fetch on the review's head: `git merge-base --is-ancestor origin/main HEAD` yes (main `b0e4a6c22`),
`git rev-list --count HEAD..origin/feat(example)/t3-code` 0, `git diff origin/main HEAD -- ':!examples/t3-code'
':!Cargo.lock' ':!Cargo.toml'` empty.

Round 11, once, on `476dae166` (the code head then), `target/r11/checks.sh`:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,846 pass, 1 skip, 0 fail (327 files; `adopt-main-fixes-r11.test.ts` added) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,470 slots, 120,810 nodes, 48 resources, 33,303,262 bytes; 233 `lower-colour-one-scheme`, 20 `lower-dialog-role` shown (the list's cap); run on these sources before the record commit. `app.contract` 1,453 lines |
| `cargo test -p t3-code-macos --lib` | 0 | 25 passed |
| AppKit binaries the retirements touch (README recipe) | 0 | `r8-keys` 6/0, `contextmenu` 70/0, `transport` 64/0, `menus` 46/0, `real-key-chords` 4/0 (screen locked) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,945 passed, 0 failed, 36 ignored (99 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `cargo check --workspace --all-targets --keep-going` (main's sixth check, `a59700afe`) | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | twice: the before (`20ce9dfdb`) and the live drive's (`6297a1728`'s sources) |

After a fresh fetch on the final head: `git merge-base --is-ancestor c4cd2bcba HEAD` yes, `git rev-list --count
HEAD..origin/feat(example)/t3-code` 0, `git diff c4cd2bcba HEAD -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
empty. Main moved one commit after these checks (`b0e4a6c22`, the terminal's animation payloads), merged in review round 1.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive 1 | this worktree at `20ce9dfdb` (bundle built before the merge) | session 1 stopped at `tap connection-settings` (the Usage page hides the sidebar's footer); sessions 2 and 3 completed | overwritten |
| Before drive 2 | the same | session 1 reordered (Settings first, Usage last), session 2 reads the dialog's tree, session 3 clicks the selection away after ⌘A; all three completed | the before column |
| After drive (the live drive) | bundle of `6297a1728`'s sources (main `b610c88ba`) | all three sessions completed; no retry | the after column |
| Reference | Electron 1e2ecbd975 on its lane, CDP | the Timeline at rest, ⌘A, shown again, scrolled, dark; the accent plane's press and focus | the reference column; [X51 record](https://raw.githubusercontent.com/ccheever/exact2/4779f9d88280eb2c15241d64175c0680881dfcd3/adopt-main-fixes-r11/rt1-x51-record.txt) |
| Review round 1, before | `t3-int`'s bundle (the T3 tip's tree), `drive-x64.sh before` | run 1 stopped at `type task-title` (the Nightly notice took the New task press: dismissed first after); run 2 stopped at the Mac's Runs on item (its label also names the settings scope's choice: targeted by its id after); run 3 completed | the before column of [X64](https://raw.githubusercontent.com/ccheever/exact2/d1a62098a7e0542ba486da64a0b5d92cbe0ff420/adopt-main-fixes-r11/rt1-x64-runs-on.png) and [⌘A](https://raw.githubusercontent.com/ccheever/exact2/e71ba5824a00cbba6b413ba8f912a70fbfa75f9d/adopt-main-fixes-r11/rt2-select-all-owner.png) |
| Review round 1, after | this branch's bundle of `86dcbfd88`'s sources, `drive-x64.sh after` | completed, first run | the after columns |

## Seen while building, not a finding row

- `contract build` now prints `lower-dialog-role` (LLP 1116.004 D4) on the clone's role-dialog popovers, 73 of them and 20
  shown under the printed list's cap; `analyze-read-after-assign` no longer reaches the list. Review round 1 read it uncapped:
  the T3 tip's 21, none new, none a bug (Each item); `STATUS.md` round 11 asks each round to read it so.
- ⌘A over a pull request's Timeline also paints boxes where the hidden Summary tab's texts lie, before and after (review
  round 1: RT-2's row, `STATUS.md` "Known differences").
- Review round 1's after drive: the scheduled task editor's Name field holds the focus with no ring under the agent (the
  before draws Exact's ring). Since main's `883c7300d` a bare field with a written border takes AppKit's own ring, which AppKit
  draws only in the key window, and the agent's window is never key. Real keys would show it; not examined further.
- After the merge the agent's launch puts the focus in the window's first key view (the sidebar's Search field, main's LLP
  1115.000 D7.5), and a button is no Tab stop unless Keyboard navigation is on (D7.2): the platform's, not changed here.
- The after drive's sweep left the logical focus on a sidebar node (332) where the before's left none: main's click focus rule
  (D7.1); not examined further.

## Not done / not verified

- Real input (steps 1–9): ran in `realinput-1011c` ("Real-input results" below); steps 7 and 9 crashed before their checks.
- The live drive ran on main `b610c88ba`; the final head adds main `c4cd2bcba` (#495, #496, the Mac text follow-ups) and
  `b0e4a6c22`, X86's retirement and the selected-text menu's adaptation. Those two are covered by `contract build`, the Bun
  guard and the AppKit `contextmenu` binary, not driven (the agent gets no selected-text menu from the host, and a long
  citation comment needs a selection and Cite: steps 3 and 8); review round 1's drive ran the merged bundle elsewhere.
- RT-1 X50: the kept pull request detail and U6 relaunch rows with `--storage` (a relaunch is a second launch; the drive's
  named-store files are the evidence).
- RT-1 X64: the provider row's status and the model trigger's label (a Reconnect or a model Shift+click needs a provider);
  the select labels were driven in review round 1.

## Review round 1

An independent review found four should-fix problems; no code changed for them.

| Problem | Done |
| --- | --- |
| RT-1 X64 had no drive of a select's label shrunk by a change made elsewhere, and the Not done reason covered only the provider rows | Driven before and after with a second environment "Studio": Runs on › Studio shortens Runs on and Project and lengthens Base branch, then back; every label drawn at its new width on both ([image](https://raw.githubusercontent.com/ccheever/exact2/d1a62098a7e0542ba486da64a0b5d92cbe0ff420/adopt-main-fixes-r11/rt1-x64-runs-on.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §1). Not done now names only the provider rows |
| RT-2's ⌘A image paints boxes that match no visible text, and the caption called the after a pass | The boxes are the hidden Summary tab's texts, painted by the host's selection of `visibility: hidden` text, before and after alike ([image](https://raw.githubusercontent.com/ccheever/exact2/e71ba5824a00cbba6b413ba8f912a70fbfa75f9d/adopt-main-fixes-r11/rt2-select-all-owner.png), [record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §2); RT-2's row says so, and `STATUS.md` "Known differences" has the row |
| 20 `lower-dialog-role` warnings fill the printed cap, so `analyze-read-after-assign` is never shown | Read uncapped on this branch and on the T3 tip: the same 21 at 13 sites, none new, none a bug ("Each item", [record](https://raw.githubusercontent.com/ccheever/exact2/8245c7d69171c2d6a243178ded37e7b732d5bfcb/adopt-main-fixes-r11/review-record.txt) §3); `STATUS.md` round 11 asks each round to read it uncapped |
| `STATUS.md`'s "Real input, not verified" row still said #261, #307 and #311 wait for main #327 | It says they waited for #327, which round 11 adopted, and are not yet run by real input |

Also: main's one new commit since the last merge (`b0e4a6c22`) merged (`86dcbfd88`), so the branch contains main again.

## Decision needed

None: every item follows the reference or main's platform rule (LLP 1115).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, RT-3 X85 | RLR-4 (g)–(j) (B13) | Pass | [RLR-4-ctrl-site-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/RLR-4-ctrl-site-pair.png) |
| 2, RT-3 X77 | R11-X77 (B13) | Pass | [R11-X77-menu-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/R11-X77-menu-pair.png) |
| 3, selected text (`6a7f16c6f`) | R11-SEL (B14) | Pass: the shell's menu over a selection (the reference's adds Services) | [R11-SEL-menu-pair.png](https://raw.githubusercontent.com/ccheever/exact2/dae1a9619a430b2ecdea34ac21af9c34cdd37a1d/realinput-1011c/b14-r11/pairs/R11-SEL-menu-pair.png) |
| 4, RT-1 X63 | R11-X63 (B14) | Fail (step 3): ⌘Z, ⇧⌘Z and Edit › Undo and Redo in the composer pass; after an archive from the row menu the clone puts the focus back in the composer, so ⌘Z undoes the composer instead of the archive (reference: the thread comes back) | [R11-X63-thread-undo-pair.png](https://raw.githubusercontent.com/ccheever/exact2/dae1a9619a430b2ecdea34ac21af9c34cdd37a1d/realinput-1011c/b14-r11/pairs/R11-X63-thread-undo-pair.png) |
| 5, RT-3 X84 (optional) | RLR-6v (B6x) | Fail: VO-⇧-M opens nothing on a link or a thread row (reference: the shell's menu, the row's menu) | [RLR-6v-pair.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/pairs/RLR-6v-pair.png) |
| 6, RT-2 | F12-2 (B12) | Pass; finding: the clone's ⌘A selects and copies hidden views' text and paints stray selection boxes | [F12-2-select-all-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/F12-2-select-all-pair.png) |
| 7, RT-1 X51 | R11-X51 (B14) | Fail (crash before the step): the clone crashed on the sidebar's Settings click from a draft whose right panel held the file viewer (`NodeView.releaseHold`, the releaseHold crash, which main PR #519 files as `issues/closed/20261013-macos-a-click-after-a-node.md` and fixes: merged as `b49c56029` and adopted in round 12, [adopt-main-fixes-r12](../20261013-adopt-main-fixes-r12.md)); the reference half not run | [R11-X51-crash-summary.txt](https://raw.githubusercontent.com/ccheever/exact2/dae1a9619a430b2ecdea34ac21af9c34cdd37a1d/realinput-1011c/b14-r11/R11-X51-crash-summary.txt) |
| 8, X86 | DCP-F (B13) | Pass: the popover holds at about 230 pt | [DCP-F-limit-record-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/DCP-F-limit-record-pair.png) |
| 9, RT-5 X79 (optional) | R11-X79 (B13) | Fail (crash at step 1, the same trigger): the reference half and step 5 not run | [R11-X79-crash-summary.txt](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/R11-X79-crash-summary.txt) |
| X81 (no step of its own) | R11-X81 (B12) | Pass (clone): the copy takes the code without the line numbers; the reference's ⌘C copied nothing (its diff is in a shadow root); recorded: the clone's copy has an extra blank line between the − and + rows | [R11-X81-pbpaste-clone.txt](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/R11-X81-pbpaste-clone.txt) |

Not passed: R11-X63's step 3, RLR-6v, and R11-X51 and R11-X79, which crashed before their steps.

## Delivery

Merged on 2026-10-12 by fast-forward (#497; the branch tip `1f813789b`, the bundle `realinput-1011c` ran), never a squash.

## Next action

None. Closed after `realinput-1011c`; R11-X63's step 3, RLR-6v and the releaseHold crash (R11-X51, R11-X79; a framework bug, filed and fixed by main PR [#519](https://github.com/ccheever/exact2/pull/519), `issues/closed/20261013-macos-a-click-after-a-node.md`, merged as `b49c56029` and adopted in round 12, [adopt-main-fixes-r12](../20261013-adopt-main-fixes-r12.md); steps 7 and 9 wait for real input to run again) are in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
