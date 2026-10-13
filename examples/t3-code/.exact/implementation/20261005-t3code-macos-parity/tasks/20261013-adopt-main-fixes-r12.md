---
name: 20261013-adopt-main-fixes-r12
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r12'
pr_url: https://github.com/ccheever/exact2/pull/520
verified_commit: a9ae1b645
---

# Main adoption, round 12 (the releaseHold crash fixed by #519), and the workarounds its fixes end

## Outcome

`feat(example)/t3-code` takes current `main`. Round 11 ([adopt-main-fixes-r11](closed/20261012-adopt-main-fixes-r11.md),
#497) adopted main `b0e4a6c22`; the records of `realinput-1011c` landed after it, and the T3 tip is `8c21a129d`. Since
`b0e4a6c22` main has #519 (`b49c56029`: a view pressed under a held pointer leaves with its freed parent), which fixes the
`NodeView.releaseHold` mouse-up crash the clone hit four times in `realinput-1011c` (G4-14, LSF-6, R11-X79, R11-X51); #495
and #496 (X86), which round 11 already carried; and Charlie's work since (LLP 1115.000 D15–D17, LLP 1115.002 D2–D8: the
Mac's structure, a vertical `list` as a table, a `dialog` and a modal route as sheets, a popover as an `NSPopover`; LLP
1116.007–1116.013; and the issues main closed after round 11, `EXACT2-GAPS.md` "Closed on main since round 11"). The user
decided on 2026-10-11 that every item below is built now, matching the reference T3 Code (no further decision needed unless
the reference itself is ambiguous).

| Id | Clone (the T3 tip `8c21a129d`) | What this round does (reference first) |
| --- | --- | --- |
| AD-1 | The branch lacks main's commits after `b0e4a6c22` (427 at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge or fast-forward that keeps main's ancestry); main's framework, the clone's `examples/t3-code`; the clone adapted to whatever main renamed, now refuses or now presents differently |
| RT-1 | The releaseHold crash: `EXC_BAD_ACCESS` in `NodeView.releaseHold` → `removeFromSuperview` on a mouse-up, after Archive thread from a row's menu then a click (G4-14), on a legacy thread's Delete confirm (LSF-6), and on the sidebar's Settings from a draft whose right panel held the file viewer (R11-X79, R11-X51); `STATUS.md`, `EXACT2-GAPS.md`, `plan.md` and two task records say "draft main PR #519, not merged" | The reference never crashes on these clicks. #519 adopted; the four triggers driven in agent mode where the agent reaches them, real-input steps where only real input does; the records say fixed on main by #519, adopted in round 12 |
| RT-2 | X86: round 11 retired the citation comment's 4rem–10rem box after #496 and `realinput-1011c` DCP-F passed | Nothing to retire (say so); the X86 row checked |
| RT-3 | Main's other fixes since `b0e4a6c22` (`EXACT2-GAPS.md` "Closed on main since round 11" and the open issues main moved): each clone workaround they end | Retire the trivial ones, matched to the reference; list the others with what each needs |

## Steps

1. Before: build this branch's bundle at the T3 tip `8c21a129d` before merging main and drive it once (lane
   `adopt-main-fixes-r12-before`, base port 16840: the clone's server on 16842).
2. `git merge --no-ff origin/main` at main's tip; main's side for the framework, the T3 side for `examples/t3-code/`.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
3. Check what the clone relies on in main's new commits (Contract refusals and renames, `contract build` warnings, the
   scripts the clone's tools call, rules and docs, and the Mac presentations LLP 1115.002 gives what the clone writes) and
   record each with its outcome ("Adopted main", "Each item").
4. RT-1 against #519's issue and the four crash records; RT-3 against the reference source; Bun tests for each source
   change.
5. `EXACT2-GAPS.md` (each row: fixed on main by #N, adopted in round 12, workaround retired or kept), `STATUS.md`'s crash
   rows and paragraph, `plan.md`, the legacy-sidebar-full and adopt-main-fixes-r11 records' notes.
6. After: this branch's bundle, the same drive (lane `adopt-main-fixes-r12`); before | after images.
7. Merge the latest `origin/feat(example)/t3-code` (and main again if it moved), then the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 2 is empty | text |
| AD-1: the checks pass on the merged tree | CLAUDE.md's checks once on the final head | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, the macOS bundle; ONE agent-mode drive on the before bundle and on this branch's | before / after screenshots |
| RT-1 | agent drive, before and after: the right panel's file viewer open, then the sidebar's Settings and more clicks; a press whose view leaves the window under the held button, then a click (the mechanism #519's issue gives for all four); real-input steps for the menu triggers (Archive thread, Delete) | before / after text and images |
| RT-1 records | `STATUS.md`, `EXACT2-GAPS.md`, `plan.md`, legacy-sidebar-full, adopt-main-fixes-r11 | text |
| RT-2 | reading: `citation-comment.contract`, the X86 row | text |
| RT-3 | each retirement: Bun guard, `contract build`, agent drive where the agent reaches it; each kept workaround listed with its reason | before / after text and images |
| Real input | "Real-input batch steps" for the coordinator's next session | — |

## Reference

- Method: [adopt-main-fixes-r11](closed/20261012-adopt-main-fixes-r11.md) (Steps, "Adopted main", checks, Delivery).
- RT-1: the reference's clicks never crash: `apps/web/src/components/Sidebar.tsx` and `hooks/useThreadActionMenu.ts`
  (Archive thread, Delete from the row's menu), `components/threadActionMenu.logic.ts`; the sidebar footer's Settings.
  Main: `issues/closed/20261013-macos-a-click-after-a-node.md` (#519), `host/apple/Sources/ExactKit/Mac/MouseChainMac.swift`,
  `GesturePrecedenceMacTests` (`testARowFreedUnderAHoldTakesThePressedViewWithIt`,
  `testAPanelReplacedUnderAClickTakesThePressedViewWithIt`, `testTheButtonLetsThePressedViewGoAfterTheHeldNodeLeft`).
- RT-2: `apps/web/src/components/chat/AssistantCitationCommentEditor.tsx` (`field-sizing-content min-h-16 max-h-40`).
- RT-3: `apps/web/src/components/settings/ConnectionsSettings.tsx` line 1427 (the "+N" toggle: `border-b border-dotted
  border-muted-foreground/60`); `components/sidebar/SidebarUpdatePill.tsx` line 304 (`aria-disabled`, a Tab stop with
  `focus-visible:ring-2`); the rest per row in "Each item".

## Adopted main

**`46f300937`**, then **`96ea7d177`**, **`ffe36f63a`** and, in review round 1, **`aa9705c33`** and **`169ea1efb`** (main's tip at the last merge,
445 commits past round 11's `b0e4a6c22`):

- `d7a0e0477`: this record's first version.
- Before any merge: the T3 tip `8c21a129d`'s bundle built in this worktree and driven (lane `adopt-main-fixes-r12-before`).
- `7c02ad186`: `git merge --no-ff origin/main` at `46f300937` (#519, #518, #517, #516, #515, #514, LLP 1115.000 D15–D17, LLP
  1115.002 D2–D8, LLP 1116.007–1116.013, LLP 1120, the issues main closed since round 11). No conflict (`Cargo.lock` merged
  clean). Before any clone change: `contract build` exit 0, 6,470 slots, 120,810 nodes, 33,303,853 bytes; nothing refused.
- `2746b0f4c`: the clone's adaptations and retirements (below); the bundle of these sources was the live drive's.
- `349a4c28a`: the records (`EXACT2-GAPS.md`, `STATUS.md`, `plan.md`, legacy-sidebar-full, adopt-main-fixes-r11).
- `7b0ab1cc8`: `git merge --no-ff origin/main` again at `96ea7d177`, after the live drive: 8 commits (the Mac's numeric height
  transitions `6621ff898`; #287, #292, X11's backdrop and X29's iframes closed as qualified or declared, `80a799d7e`,
  `81bde641b`; the web's socket resources `8cb663713`; the presenter's empty-batch skip `f5839e51b`). No conflict; `contract
  build` unchanged (33,309,113 bytes with this round's changes). `a9ae1b645`: the records it touches.
- `e71f9d7cb`: `git merge --no-ff origin/main` at `ffe36f63a` after the checks (two commits: the web JS target's bound
  `autofocus`, `host/web-js`; nothing the clone uses). No conflict.
- Review round 1: `de548a792` (the five dashed boxes and the source guards), then `3c00d8b3b`: `git merge --no-ff origin/main`
  at `aa9705c33` (one commit, #281: on macOS a positioned overlay's mouse contact reaches the overlay and its ancestors,
  never the siblings it covers, as in Chrome; `NodeViewMac.swift` forwards an unpressed click to `nextResponder` instead of
  NSView's default, which retried covered siblings; it closes #321's `elementsFromPoint` issue, X68). No conflict. This
  bundle was built and driven (sessions 1, 2b and 4, below), so the second to fourth merges' Mac changes (`f5839e51b`,
  `6621ff898`, `aa9705c33`) ran in the app.
- `4b09f25e7`: `git merge --no-ff origin/main` at `169ea1efb` before the checks (five commits, web only: the web dev and static
  servers take a dotted deep link as the app's document, a web modal sheet's neutral ground; `host/web`, `host/web-js`,
  `scripts/origin.mjs`; nothing on the Mac path the clone runs). No conflict.

On the final head after a fresh fetch: `git merge-base --is-ancestor origin/main HEAD`, `git rev-list --count
HEAD..origin/feat(example)/t3-code` and `git diff origin/main HEAD -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
(Checks). The `Cargo.toml` / `Cargo.lock` difference is the `examples/t3-code/macos` member and the `t3-code-macos` package
only. The PR must land as a merge (or a fast-forward of the feature branch to its head), never a squash: a squash drops
main from the branch's ancestry.

## Each item of main's new commits

| Item | Main commit | Outcome | Clone change |
| --- | --- | --- | --- |
| The releaseHold crash | `b49c56029` (#519) | RT-1 | none (no workaround existed) |
| A vertical `list` is an `NSTableView` (LLP 1115.000 D16, LLP 1115.002 D5) | `c8314ba60`, `beff41742`, `b564b6ab0` | Under real input the clone's five lists (the timeline transcript, the diff, the pull request Code list, the legacy projects, an environment's routes) would become tables with AppKit's separators, selection and keys, where the reference draws divs; main's own apps opt their transcripts and documents out with `appearance="none"` | `appearance="none"` on the five (AD-1) |
| A popover that is no menu is an `NSPopover` (LLP 1115.002 D7.3) | `34855f4b3`, `38281323f` | Under real input the clone's non-menu popovers would become AppKit popovers (arrow, material, AppKit's placement), where the reference paints Base UI popups; an authored `position-try-fallbacks` row selects the painted layer (LLP 1021 D2), and its painted placement takes `none` as no row (`MenusMac.layout`) | `position-try-fallbacks="none"` on all 62 popovers (AD-1); a menu's shape stays the `NSMenu` it was |
| A route `header` as the title bar and toolbar, a root `tablist` as a sidebar, a list and its detail as a split view, `list appearance="auto"` as the grouped form, a `dialog` and a modal route as sheets (LLP 1115.002 D2–D4, D6, D7.2, D7.4) | various | The clone writes none of these (no routes, no `dialog`, no `header`, no root `tablist`, no `appearance="auto"`): nothing changes; session 1's screens equal the before's | none |
| New `contract build` warning `lower-width-padding` (LLP 1116.009 D5: a padded `width="100%"` box is wider than its parent) | `31538c22f` | One site, uncapped: `pages-prs.contract:248`, the pull request list's error notes, 1.5 rem past its parent (content-box; the reference's Tailwind is border-box everywhere) | `box-sizing="border-box"`, with a Bun source guard (review round 1) |
| `lower-dialog-role`, `analyze-read-after-assign`, `lower-count-plural`, `type-performance-now-persisted`, `lower-colour-one-scheme` | — | Read uncapped (a compiler built with `MAX_WARNINGS` lifted into `target/r12/uncapped-target`, the constant restored at once, nothing committed): 21, 8, 1, 73 and 233, the same as round 11's review; nothing new but the padding warning | none |
| LLP 1115.000 D10 (`hr` one hairline, no margins on checkbox/radio/range, a range and a progress as wide as their parent), D15 (a button's face when no `buttonStyle`) | `2b89082b0`, `590b073cd`, `a9cc2a251`, `e42caa419` | The clone's four `hr`, its range and its progress write their sizes and margins (the author's win); session 1's screens equal the before's | none |
| An empty `href` is no link; `aria-level` without `role="heading"` is no heading | `a22051869`, `b569b7840` | The clone's headings write `role="heading"`; its tree items keep `aria-level` as items | none |
| `aria-disabled` (X70) | `9074a745b`, `c4f1279fc` | RT-3: retired | the update pill |
| `dotted` and `dashed` borders (#287) | `ff2251b0c`, closed by `80a799d7e` | RT-3: every `border-dashed` and `border-dotted` of the reference's web UI checked (11 sites): the "+N" toggle and, in review round 1, five boxes retired; the unresolved chip's and the pending review comment card's stroked dashed rects kept (follow-up); three with no clone site (`EXACT2-GAPS.md` #287 row) | the toggle; the themes empty box, the Theme file zone, "No compatible agents found", "Desktop host signals not connected", the keybinding When editor's empty box |
| The issues main closed since round 11 (X10, X19, X25, X26, X28, X33, X34, X45, X48, X53, X55, X57, X60, X63, X65, X66, X69, X77's note, X83; second merge: X11's backdrop, #292, X29's iframes, the height transition) and the partial fixes (X17, X20, X22, X23, X27, X54) | various | RT-3: each listed with what became of its workaround (`EXACT2-GAPS.md` "Closed on main since round 11") | X70, the "+N" toggle |
| An empty scroll port passes the pointer on (X65) | `3998e650e` | Adopted; `usage-ground` kept (RT-3). Its `ChainingScrollView` hold refuses AppKit's removal when the parent is freed: X87, found by the live drive (RT-1) | none (framework) |
| macOS: relaunch, display mode, scroll restore, selection rects, popover commands, `close=`, `keycapture=`, menu-bar items, notification `navigate`, `intersect=`, `scrollend`, `setRangeText`, installed font families, `querySelectorAll` | various | New framework surface the clone can use later; each related workaround listed in RT-3 | none |
| The agent driver: a tap is one line, `state` one line a section (`4387e37b5`); a tap on a disabled control fails the step (`59156f084`); `--chrome platform` on macos | various | The drive's JSON is unchanged under `--json`; no step taps a disabled control; session 3b uses `--chrome platform` (the T3 tip's agent refuses it on macos) | none |
| A positioned overlay takes its own click on macOS (#281); `elementsFromPoint` (#321, X68) | `aa9705c33` (review round 1's merge) | Adopted: no clone workaround relied on a click falling through to a covered sibling (sessions 1, 2b and 4 on this bundle; sessions 1 and 2b pixel-equal to the live drive's); Inspect app colors (U18) can be built now: follow-up | none |
| A sixth check, `cargo check --workspace --all-targets --keep-going` | (round 11) | run with the checks | none |
| iOS, Android, web, Linux, Windows, LLP docs, QUEUE | various | nothing on the Mac path the clone uses | none |

## Cause and fix

- **RT-1, the releaseHold crash (#519, adopted).** #519's issue gives the mechanism: while the button is held, the hold
  keeps the view the button went down on in its parent; its `removeFromSuperview` refused every removal, AppKit's own when
  it frees the parent included, so the view pointed at a freed superview and `releaseHold` messaged it at the next mouse-up.
  The agent reproduces it with the clone's own views: `tap <timeline> down`, ⌘1 and ⌘2 (thread.jump) while held, `tap up`
  (AppKit sends it to no view), then a click on the sidebar's Settings. The T3 tip's bundle exits with SIGSEGV at that
  click's up, with the realinput frames (`NodeView.releaseHold()` MouseChainMac.swift:383 ← `pointerReleased` ←
  `mouseUp`). With the press on a text node (the reply table's Mail cell, session 2b), round 12's bundle opens Settings and
  goes on. The literal third trigger (the file viewer open, then Settings and more clicks) does not crash under the agent on
  either build (session 1), and the menu triggers need a native menu that takes the right-click's up, which the agent never
  pops (`T3ContextMenu` answers dismissed under the agent): real-input steps 1–3.
- **RT-1, X87 (new, framework).** With the press on the timeline's empty area (session 2, `tap transcript down`: the
  scroller's port below the reply), round 12's bundle ends earlier, at ⌘1's batch, before the up: SIGABRT,
  `NSInternalInconsistencyException` "<ExactKit.ChainingScrollView> has reached dealloc but still has a super view". Bisected
  in `target/r12/diag` with the same ops (the reason read from a driver copy that kept 400 lines of the app's stderr, removed
  after): the same abort with `MouseChainMac.swift` from before #519, and on the merge commit alone (no clone change of this
  round). Main `3998e650e` (X65's fix: "an empty scroll port passes the pointer on") gave `ChainingScrollView` its own hold,
  a `removeFromSuperview` that refuses every removal while its mouse is held, AppKit's own when it frees the parent
  included: #519's pattern in another class (#519's guard, "only while it is in the window", would fit it too). A framework
  bug; not filed here (the coordinator files), recorded as X87 in `EXACT2-GAPS.md` and in `STATUS.md` "Found, not in
  scope". No clone workaround: the clone cannot keep a press from landing on an empty port.
- **AD-1, the Mac's presentations.** `appearance="none"` is the list rule's opt-out (`contract/lower/src/cells.rs`), and an
  authored `position-try-fallbacks` the popover's (`PopoversMac.wantsWindow`); `none` keeps the painted placement as it was
  (no flip; X17's flips stay a separate change). Under the agent's own chrome nothing changes either way (both paint), so
  session 3b drives `--chrome platform`: the "Add panel surface" popover (no menu's shape) is painted, its navigation state
  names no `NSPopover`, and the timeline is the painted list.
- **RT-2, X86.** Nothing to retire: round 11's second merge took #496 and retired the citation comment's 4rem–10rem box
  (`citation-comment.contract`: the textarea straight in the popover, `adopt-main-fixes-r11.test.ts`), and `realinput-1011c`
  DCP-F passed (the popover holds at about 230 pt). The X86 row already says so.
- **RT-3, X70 (retired).** The reference's `SidebarUpdatePill` is `aria-disabled`: a Tab stop that reads as disabled. The
  clone wrote `disabled=true` (Contract had no `aria-disabled`), so Tab skipped it, and the host also dimmed it over its
  authored 60%: the icon was `#e1e1e2` where the reference's is `#c8c8cc`. Now `aria-disabled=true`: Tab from Settings' Back
  reaches it (state focus 1376, `settings-check-updates`), the icon is `#c8c8cc`, and a press still does nothing (it has no
  press).
- **RT-3, the "+N" toggle (retired).** `border-bottom-style="dotted"`, as `ConnectionsSettings.tsx`'s `border-b
  border-dotted border-muted-foreground/60`. Not driven: the toggle shows only with Network access on (a server restart).
- **RT-3, listed.** Kept with why, in `EXACT2-GAPS.md` "Closed on main since round 11": X57's `text-align="left"` where the
  reference does not say it (a line that fits keeps its centre; a check per site), X60's `tsStep` (equivalent; the dialog
  needs the tailnet stub lane), X65's `usage-ground` (the Limits page's pinned popover), X66's `KeyMenuOpen`, X53's traps
  (`close=` serves a `dialog` element, a sheet on the Mac now), X25's and X26's native readers and menus, X28 (no routes),
  X83 (needs `background-attachment: local`), X33 and X45 (parity changes), X17, X22, X23, X27, X54 (reworks), the dashed
  rects, `T3MenuTurn` (#292: the module's own menus), the terminal drawer's `interpolate-size` (the height transition),
  X11's flattened glass (declared). X48 and X69 unblock their held tasks. Review round 1 put each kept one, and Inspect
  app colors (U18, unblocked by `aa9705c33`), in the follow-up [r12-kept-workarounds](20261013-r12-kept-workarounds.md).
- **`lower-width-padding` (fixed).** The pull request list's error notes are `width="100%"` with 0.75 rem padding each side in
  a content-box column, so 1.5 rem wider than their parent; `box-sizing="border-box"`, as Tailwind's. Review round 1 added its
  source guard (`adopt-main-fixes-r12.test.ts`; it fails on the T3 tip).
- **RT-3, #287's dashed boxes (retired, review round 1).** Main paints `dashed` with Chrome's geometry since `ff2251b0c`. Of
  the reference's 11 `border-dashed`/`border-dotted` sites, five more are boxes the clone drew `solid`: the themes empty box
  (`ThemeSearchSection.tsx:359`), the Theme file zone (`ThemeImportDialog.tsx:520`), "No compatible agents found"
  (`AcpRegistrySearchStep.tsx:198`), "Desktop host signals not connected" (`ResourceTelemetryDiagnostics.tsx:1151`) and the
  keybinding When editor's empty box (`KeybindingsSettings.tsx:687`); each is now `border-style="dashed"` (one attribute;
  widths and colours unchanged). The unresolved chip and the pending review comment card keep their stroked `svg` rects
  (retiring them changes the box; follow-up KW-7); the composer's workspace drop overlay, the sidebar's empty-section drop
  hint and the hero's no-project button have no clone site.

## Acceptance results

Drives: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/2827a3485383f954ed5b6164b37b9cb786bdb211/adopt-main-fixes-r12/drive.sh.txt) and [lane.sh](https://raw.githubusercontent.com/ccheever/exact2/d64b3a7a6ceb6941849219796ef6a8752e347f60/adopt-main-fixes-r12/lane.sh.txt); [readouts](https://raw.githubusercontent.com/ccheever/exact2/d3c50deffc9d03407c6d3282e4e48f4bf9275513/adopt-main-fixes-r12/readouts.txt) (exits, focus, pixels, crash frames, the bisect, the
uncapped analysis).

| Row | Result | Proof |
| --- | --- | --- |
| AD-1: the branch contains main | pass at the final merge (main `169ea1efb`) | Adopted main; Checks |
| AD-1: the framework tree is main's | pass: empty after `7c02ad186`, `7b0ab1cc8`, `e71f9d7cb`, `3c00d8b3b`, `4b09f25e7` and on the final head | Adopted main; Checks |
| AD-1: the checks pass | see Checks | Checks |
| AD-1: the clone works | pass: session 1 (shell, Settings General, Appearance, Providers, Tab from Back, Usage, the fixture thread, the file viewer, Settings from it, Back) exits 0 on both; every screen is pixel-equal but for the update pill (147 px, RT-3 X70). The lists and popovers keep the reference's presentation under `--chrome platform` (session 3b, after only) | [shell](https://raw.githubusercontent.com/ccheever/exact2/186077d9d38fbd2a946cd86e2d36383e056797d3/adopt-main-fixes-r12/ad1-shell.png), [thread and file viewer](https://raw.githubusercontent.com/ccheever/exact2/97497c53393374936d136fdb3d460a882f6832c2/adopt-main-fixes-r12/ad1-thread-files.png), [Mac presentations](https://raw.githubusercontent.com/ccheever/exact2/88a5316be91eadf0d6b1da893b54da236a0878ba/adopt-main-fixes-r12/ad1-platform-chrome.png), readouts |
| AD-1: the later merges run (review round 1) | pass: the bundle of `3c00d8b3b` (main `aa9705c33`), sessions 1 and 2b again: exit 0, all 15 screens pixel-equal to the live drive's (main `46f300937`), Tab from Back on the pill (node 1376) | [readouts-r1](https://raw.githubusercontent.com/ccheever/exact2/f7a58fe10fe5387a3cd716dbe9c4b11e1fd868d0/adopt-main-fixes-r12/readouts-r1.txt) |
| RT-1 (#519's mechanism) | pass: session 2b, a press held on the reply table's Mail cell, ⌘1 ⌘2, the up, Settings: before SIGSEGV at the Settings click's up (`NodeView.releaseHold`, MouseChainMac.swift:383, the realinput frames); after, Settings opens and Back returns (exit 0) | [image](https://raw.githubusercontent.com/ccheever/exact2/a37b824b6ebcbc181a3c01f4213de0ba702814bd/adopt-main-fixes-r12/rt1-519-held-press.png), readouts |
| RT-1 (the literal third trigger) | no crash on either build under the agent (session 1: the file viewer open, Settings, Appearance, Back); the real one is real-input step 3 | readouts |
| RT-1 (the menu triggers, G4-14 and LSF-6) | not reachable by the agent (`T3ContextMenu` pops no menu under it, so no menu takes an up): real-input steps 1 and 2 | — |
| RT-1 (X87, new) | **fail, framework**: session 2, the press on the timeline's empty area: before SIGSEGV at the Settings click (the releaseHold crash); after SIGABRT at ⌘1's batch (`ChainingScrollView` "has reached dealloc but still has a super view"), from main `3998e650e`, bisected; recorded as X87, not filed | readouts |
| RT-1 records | pass: `STATUS.md` (the paragraph, the four rows, round 12's entry, X87 in "Found, not in scope"), `EXACT2-GAPS.md` (the releaseHold section, X87), `plan.md`, legacy-sidebar-full and adopt-main-fixes-r11's notes | the diff |
| RT-2 | none to retire (round 11 retired X86's box; `realinput-1011c` DCP-F passed) | Cause and fix |
| RT-3 X70 | pass: before, Tab from Back went past the pill (node 3592; the pill `disabled`); after, Tab focuses it (node 1376, `accessibilityDisabled`), as the reference's (CDP: "Check for updates", `aria-disabled=true`, `:focus-visible`); the icon `#c8c8cc` as the reference's (before `#e1e1e2`). The agent's window is never key, so the ring AppKit draws is a real-input step | [pill](https://raw.githubusercontent.com/ccheever/exact2/53ec2ea8696459f287e950d3a099ad6e59f263f9/adopt-main-fixes-r12/rt3-x70-pill.png), [Tab](https://raw.githubusercontent.com/ccheever/exact2/3d93f50a17947232215d28fabb1364cc676e8681/adopt-main-fixes-r12/rt3-x70-tab.png) |
| RT-3 "+N" toggle | pass by `adopt-main-fixes-r12.test.ts` and `contract build`; not driven (needs Network access on): real-input step 6 | Tests |
| RT-3 #287's dashed boxes (review round 1) | pass: five guards (fail on the T3 tip and on the round's previous head), `contract build`; session 4, before (the evidence base) solid, after dashed, as the reference's, for all five | [Add theme](https://raw.githubusercontent.com/ccheever/exact2/db9f5304d6dc2c042b51f1aee04226251f887dda/adopt-main-fixes-r12/rt3-287-add-theme.png), [ACP](https://raw.githubusercontent.com/ccheever/exact2/324b49aa62d591c4fc052063bcbb1f29fd8553da/adopt-main-fixes-r12/rt3-287-acp.png), [When](https://raw.githubusercontent.com/ccheever/exact2/05662e3569d50d84e839619b63afb18d83bd7e0b/adopt-main-fixes-r12/rt3-287-when.png), [Diagnostics](https://raw.githubusercontent.com/ccheever/exact2/8317fa15ba5805ae33da2bc6c2d77385135a7873/adopt-main-fixes-r12/rt3-287-diagnostics.png) |
| RT-3 listed | records, and the follow-up [r12-kept-workarounds](20261013-r12-kept-workarounds.md) (13 rows) | `EXACT2-GAPS.md` |
| `lower-width-padding` | pass: the uncapped build no longer names it after the fix; the source guard (review round 1) fails on the T3 tip | readouts, Tests |
| Real input | "Real-input batch steps" below | — |

Drives (agent mode, 1280×840, `--epoch 2026-10-13T22:00:00+09:00 --time-zone Asia/Seoul`, base port 16840, the clone's
server on 16842; each session on a lane reset to round 11's seed). Before: this worktree at the T3 tip `8c21a129d` (main
`b0e4a6c22`), built before the merge (lane `adopt-main-fixes-r12-before`); session 2b on `t3-code-ri1011b`'s bundle of
`1f813789b` (the T3 tip after #497, the same code: four test files differ), driven only, since this worktree's before bundle
had been rebuilt by then. After: the bundle of `2746b0f4c`'s sources (main `46f300937`; lane `adopt-main-fixes-r12`).
Reference: T3 Code `1e2ecbd975` on lane `adopt-main-fixes-r12` (16840/16841) over CDP.

## Real-input batch steps

For the coordinator's next session. Launch this branch's bundle as a lane copy (its own bundle id, a lane home outside any
git checkout) seeded as `lane.sh` seeds it, window 1280×840, beside the reference. Never send a message, never change System
Settings.

1. **RT-1 G4-14.** Right-click a thread row in the sidebar, pick Archive thread from its menu, then click the sidebar
   footer's Settings: Settings opens (before: SIGSEGV on that click's mouse-up). Continue G4-14's steps 1–5 from there.
2. **RT-1 LSF-6.** In the GitHub lane with a background environment (B12's), right-click a legacy background thread, Delete,
   then Confirm: the thread is deleted and the app stays (before: SIGSEGV on Confirm's mouse-up). Then LSF-6's steps 4–5.
3. **RT-1 R11-X79, R11-X51.** In a work draft, open the right panel's Files, open an image (shot.png) beside a rendered
   Markdown file, then click the sidebar's Settings: Settings opens. Then R11-X79's import wizard step (Tab from the popup:
   Cancel; Shift+Tab: the X) and R11-X51's colour plane press (Appearance › Create theme › the accent swatch, drag on the
   plane over the editor's controls: the colour follows, nothing beneath changes).
4. **AD-1, the Mac's presentations.** Click the right panel's "+" (Add panel surface), the Diff's comparison target (with a
   branch base), a pull request list's Sort and a Usage page's period: each is a painted popup as the reference's, no AppKit
   popover arrow. Click a timeline row, a diff row and a pull request Code row: no table selection highlight, no separator
   lines between rows.
5. **RT-3 X70.** Look at the sidebar's "Check for updates" pill: the same grey as the reference's. If this Mac has Keyboard
   navigation on (do not change it), Tab from Settings' Back reaches the pill with a ring.
6. **RT-3 "+N" (optional).** In a lane app only: Settings › Connections › Network access on (a server restart), then the
   "Reachable at … +N" toggle: a dotted underline, as the reference's. Network access off again.
7. **X87 (optional, ends the lane app).** In a thread, press and hold below the last message (the timeline's empty area),
   and with the button held press ⌘1: round 12 aborts (X87). Release; relaunch the lane copy.
8. **The final bundle (main `aa9705c33`, review round 1).** Session 1's walk by real clicks: Settings › General, Appearance,
   Providers, Back; Usage and back; the fixture thread, the right panel's Files and `fixture.txt`, the sidebar's Settings,
   Appearance, Back. Session 2b by real input: press and hold on the reply table's Mail cell, ⌘1, ⌘2, release, click the
   sidebar's Settings, Back. Each click acts once; nothing crashes. Then a click on an overlay over a control (the right
   panel's "+" popover over the panel's tabs, an open popover's row over the page beneath): only the overlay acts (#281).
9. **#287's boxes (optional).** Settings › Appearance › Add theme (the Theme file zone; a search for `zzqxjvwk`: the empty
   box), Settings › Keybindings › + › When (the empty box): dashed as the reference's.

## Tests

- `adopt-main-fixes-r12.test.ts` (new, 4): every vertical `list` says `appearance="none"` (the five, by file); every
  popover says `position-try-fallbacks="none"` (62 or more); the update pill is `aria-disabled`, no HTML `disabled`; the
  "+N" toggle's underline is dotted.
- `adopt-main-fixes-r11.test.ts`: X51's colour popover guard reads the popover line with its fallback row.
- `adopt-main-fixes-r12.test.ts` (review round 1, +6, 10 in all): the five boxes the reference draws `border-dashed` are
  `dashed` (each by its anchor, with its reference line); the pull request list's error notes are `width="100%"`, padded
  0.75 rem each side and `border-box` (`lower-width-padding`). All ten fail on the T3 tip `8c21a129d`; the five dashed ones
  fail on the round's previous head `bed687f8b`.
- No Swift or Rust of the clone changed, so no AppKit binary was rebuilt (`host/apple/modules/` is unchanged since round
  11); `cargo test -p t3-code-macos --lib` ran with the checks.

## Checks

Once, on `a9ae1b645` (the code head after the second merge; `target/r12/checks.sh`). The commits after it change three test
files' source guards (`1218c664f`), the records, and merge main's two web-only commits (`e71f9d7cb`: the web JS target's bound
autofocus, `host/web-js`); on those, `bun test`, `caps` and `boot` ran again.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 1, then 0 | the first run: 3 fail, three source guards that read the old pill and popover lines (`settings-a-about`, `sidebar-palette-keys`, `usage-pr-pages`); after `1218c664f`: 4,850 pass, 1 skip, 0 fail (328 files), and again on `e71f9d7cb` |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,470 slots, 120,810 nodes, 48 resources, 33,309,113 bytes; 233 `lower-colour-one-scheme`, 20 `lower-dialog-role` shown (the cap). Uncapped: 21 `analyze-read-after-assign`, 8 `lower-count-plural`, 1 `type-performance-now-persisted`, 73 `lower-dialog-role`, 233 colours, no `lower-width-padding` after the fix. `app.contract` 1,453 lines |
| `cargo test -p t3-code-macos --lib` | 0 | 25 passed |
| AppKit binaries | — | none touched (no clone Swift changed; `host/apple/modules/` unchanged since round 11) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap (again 0 on `e71f9d7cb`) |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,330 passed, 0 failed, 46 ignored (103 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | again 0 on `e71f9d7cb` |
| `cargo check --workspace --all-targets --keep-going` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | three times: the before (`8c21a129d`), the live drive's (`2746b0f4c`'s sources) and the checks' (`a9ae1b645`) |

After a fresh fetch on the final head: `git merge-base --is-ancestor origin/main HEAD` yes (main `ffe36f63a`), `git rev-list
--count HEAD..origin/feat(example)/t3-code` 0, `git diff origin/main HEAD -- ':!examples/t3-code' ':!Cargo.lock'
':!Cargo.toml'` empty.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive, session 1, runs 1–4 | this worktree at `8c21a129d` | stopped on the drive's own errors: `open-pull-requests` is not in this lane's footer (run 1, and run 2 found an embedded server a crashed session had left on the lane port), the right panel was not open (run 3), the Nightly notice lay over the file row (run 4) | not used |
| Before drive, run 1 sessions 2 and 3; run 5 session 1 | the same | session 2: SIGSEGV at the Settings click (the releaseHold crash); session 3: the T3 tip's agent refuses `--chrome platform` on macos; session 1 (run 5): exit 0 | the before column |
| Before session 2b | `t3-code-ri1011b`'s bundle (`1f813789b`) | run 1: "Contact" names six views; run 2 (the Mail cell by its point): SIGSEGV at the Settings click | the before column of RT-1 |
| After drive (the live drive) | the bundle of `2746b0f4c`'s sources | session 1 exit 0; session 2 SIGABRT (X87); session 3 met the server session 2's abort had left on the lane port | the after column |
| Diagnosis | `target/r12/diag` | session 2's ops four times: the reason from a driver copy keeping 400 lines (removed after), then with `MouseChainMac.swift` from before #519, then on the merge commit's clone sources: the same SIGABRT each time; each build restored | readouts |
| Review round 1 drive | the bundle of `3c00d8b3b` (main `aa9705c33`) | sessions 1 and 2b exit 0 (15 screens pixel-equal to the live drive's); session 4 stopped at op 46 (`tap View diagnostics`: one word per target) after its screens 40-43, and its retry (`setting-diagnostics-link`) exit 0; before session 4 on the evidence base's bundle exit 0 | [drive-r1.sh](https://raw.githubusercontent.com/ccheever/exact2/ea8b39b73f9acdc943caa5c7d1d7277973481c98/adopt-main-fixes-r12/drive-r1.sh.txt), [readouts-r1](https://raw.githubusercontent.com/ccheever/exact2/f7a58fe10fe5387a3cd716dbe9c4b11e1fd868d0/adopt-main-fixes-r12/readouts-r1.txt) |
| Reference, review round 1 | Electron `1e2ecbd975` on lane `adopt-main-fixes-r12-ref` (16840/16841) over CDP | the When editor, Add theme with no result, the ACP search with no result, Diagnostics | the reference column |
| After retry (the one retry) | the bundle of `2746b0f4c`'s sources, rebuilt | session 2b exit 0; session 3 targeted a comparison-target button this thread does not show; 3b (the same session with the "Add panel surface" popover) exit 0 | the after column |
| Reference | Electron `1e2ecbd975` on its lane, CDP | the shell's pill, Tab from Settings' Back, the diff scope popup | the reference column |

## Seen while building, not a finding row

- A session that crashes leaves its embedded T3 server running on the lane port (the server outlives the app), and the
  next session then connects to it with its home deleted, so its sidebar is empty; `drive.sh` now stops that server after
  each session (its own PID only).
- The pull request footer button (`open-pull-requests`) is absent in this lane's seed for the first minute; the drive left
  the pull request list out.
- Settings › Diagnostics shows "Desktop host signals not connected" on both builds in this lane (the T3 tip's and round 12's),
  where the reference shows its Host state rows (power source, idle, lock: its Electron host supplies them). Pre-existing,
  not this round's; not investigated (the telemetry task's desktop feed should supply them; a known-differences candidate).

## Not done / not verified

- Real input: steps 1–7 (the menu triggers G4-14 and LSF-6 and the literal R11-X79/X51 trigger need a real pointer; the
  pill's ring needs a key window; the "+N" toggle needs Network access on).
- X87: a framework crash found here; the clone cannot work around it; not filed (the coordinator files framework issues).
- RT-3's listed workarounds stay (each needs a parity change across many sites, a rework, or a drive the round's lanes lack):
  X10's `text-wrap: balance` and `::placeholder` colours, X57's `text-align="left"` sites, X60's `tsStep`, X65's
  `usage-ground`, X66's `KeyMenuOpen`, X17's flips, #287's two stroked dashed rects, the terminal drawer's
  `interpolate-size`, X33's Cite anchor, X45's relaunch, X25/X26's native readers and menus, X22/X23/X27/X54's hooks; and
  Inspect app colors (U18), unblocked by `aa9705c33`. Each is a row of the follow-up
  [r12-kept-workarounds](20261013-r12-kept-workarounds.md) (planned; the coordinator decides which to run).
- The merges' Mac runtime changes under real input (the presenter's empty-batch skip `f5839e51b`, the numeric height
  transitions `6621ff898`, the overlay contact `aa9705c33`): driven in agent mode on the final bundle (review round 1),
  real-input step 8.

## Decision needed

None: every item follows the reference or main's platform rules. X87 is a framework bug for the coordinator to file.

## Review round 1 (2026-10-13)

| Problem | What was done |
| --- | --- |
| RT-3's #287 inventory incomplete: four `solid` boxes the reference draws `border-dashed` | All 11 `border-dashed`/`border-dotted` sites of the reference checked: those four and a fifth (the keybinding When editor's empty box) retired to `dashed`, with guards and before/after/reference images; the two stroked rects kept (follow-up KW-7); three have no clone site. `EXACT2-GAPS.md`'s #287 row lists all 11 |
| `lower-width-padding` had no source guard | A guard in `adopt-main-fixes-r12.test.ts` (fails on the T3 tip) |
| The second merge's Mac runtime changes never launched | Main had moved again (`aa9705c33`, Mac pointer change): merged, the bundle built, sessions 1 and 2b driven on it (exit 0, pixel-equal to the live drive's) plus session 4; real-input step 8 for the coordinator |
| Stale records (no `ffe36f63a`) | `STATUS.md` item 5, `plan.md`, `EXACT2-GAPS.md`'s date line and "Closed on main since round 11" heading name every merge through `aa9705c33` |
| The PR's Not done lacked RT-3's kept workarounds | Listed in the PR and here, each a row of the follow-up [r12-kept-workarounds](20261013-r12-kept-workarounds.md) |

## Delivery

Draft PR [#520](https://github.com/ccheever/exact2/pull/520) against `feat(example)/t3-code`. It must land as a merge or a fast-forward of the feature branch to its
head, never a squash (a squash drops main from the branch's ancestry).

## Next action

The coordinator: review, file X87 on main, run the real-input steps, then merge (merge or fast-forward).
