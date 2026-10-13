---
name: 20261013-adopt-main-fixes-r12
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r12'
pr_url:
verified_commit:
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
