---
name: 20261014-adopt-main-fixes-r13
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r13'
pr_url: null
verified_commit: null
---

# Main adoption, round 13 (X87 fixed by #523)

## Outcome

`feat(example)/t3-code` takes current `main`. Round 12 ([adopt-main-fixes-r12](20261013-adopt-main-fixes-r12.md), #520,
merged 2026-10-13 by fast-forward as `9149492d4`) adopted main `0b956d63d`. Since then main has #523 (`898ca3a09`: a
`ChainingScrollView` keeps a held view only while it is in the window), which fixes X87, the SIGABRT round 12's agent
drive found (a press held on the timeline scroller's empty area, then ⌘1 and ⌘2); and Charlie's work since (Ibex's
consumer adoption, `f31c6e2ba` to `6dd91519c`; the intersection fix for sticky targets, `c016978a6`). The user decided
on 2026-10-11 that every item below is built now, matching the reference T3 Code (no further decision needed unless the
reference itself is ambiguous).

| Id | Clone (the T3 tip `9149492d4`) | What this round does (reference first) |
| --- | --- | --- |
| AD-1 | The branch lacks main's commits after `0b956d63d` (11 at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge or fast-forward that keeps main's ancestry); main's framework, the clone's `examples/t3-code`; the clone adapted to whatever main renamed, now refuses or now presents differently |
| RT-1 | X87: a press held on the timeline scroller's empty area (`tap transcript down`), then ⌘1 and ⌘2 (`thread.jump`), aborts the app at ⌘1's batch (SIGABRT, `NSInternalInconsistencyException`: "<ExactKit.ChainingScrollView> has reached dealloc but still has a super view"); `EXACT2-GAPS.md`, `STATUS.md` and `plan.md` say "not filed" | The reference never aborts on a press and a thread switch. #523 adopted; round 12's agent trigger driven on the before bundle (aborts) and on this round's (runs on), with readouts; the records say fixed on main by #523, adopted in round 13 |
| RT-2 | Main's log since `0b956d63d` against `EXACT2-GAPS.md`'s rows and the follow-up [r12-kept-workarounds](20261013-r12-kept-workarounds.md) | Retire only the trivial workarounds a merged main fix ends (before/after each); list the rest with why |

## Steps

1. Before: build this branch's bundle at the T3 tip `9149492d4` in this worktree before merging main and drive it once
   (lane `adopt-main-fixes-r13-before`, base port 16850: the clone's server on 16852).
2. `git merge --no-ff origin/main` at main's tip; main's side for the framework, the T3 side for `examples/t3-code/`.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
3. Check what the clone relies on in main's new commits (Contract refusals and renames, `contract build` warnings, the
   scripts the clone's tools call, the Mac host changes) and record each with its outcome ("Adopted main", "Each item").
4. RT-1 against #523's issue (`issues/closed/20261013-macos-a-press-held-on-a.md`) and round 12's X87 readouts; RT-2
   against main's log and the reference source; Bun tests for each source change.
5. `EXACT2-GAPS.md` (X87: fixed on main by #523, adopted in round 13), `STATUS.md` (X87's "Found, not in scope" row,
   the adoption entry), `plan.md`.
6. After: this branch's bundle, the same drive (lane `adopt-main-fixes-r13`); before | after images.
7. Merge the latest `origin/feat(example)/t3-code` (and main again if it moved), then the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 2 is empty | text |
| AD-1: the checks pass on the merged tree | CLAUDE.md's checks once on the final head, the workspace check | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, the macOS bundle; ONE agent-mode smoke drive on the before bundle and on this branch's | before / after screenshots |
| RT-1 (X87) | agent drive, before and after: the fixture thread, `tap transcript down` (the timeline scroller's empty area), ⌘1 and ⌘2 (`thread.jump`), the up, the sidebar's Settings, Back; exits and crash frames read from the drive | before / after text and images |
| RT-1 records | `EXACT2-GAPS.md` X87, `STATUS.md`, `plan.md` | text |
| RT-2 | main's log since `0b956d63d`, commit by commit, against `EXACT2-GAPS.md` and the 13 rows of r12-kept-workarounds: each trivial retirement with a Bun guard, `contract build` and before/after; each kept workaround listed with its reason | text (images where a retirement is visible) |
| Real input | "Real-input batch steps" for the coordinator's next session, if any row needs real input | — |

## Reference

- Method: [adopt-main-fixes-r12](20261013-adopt-main-fixes-r12.md) (Steps, "Adopted main", "Each item", checks, Delivery).
- RT-1: the reference never aborts on a press held over its timeline while a thread switch replaces it
  (`apps/web/src/components/chat/MessagesTimeline.tsx`, the thread routes `routes/_chat.$environmentId.$threadId.tsx`;
  `thread.jump.1` to `.9` in `packages/contracts/src/keybindings.ts`, run by `components/Sidebar.tsx`
  (`threadJumpIndexFromCommand`)). Main:
  `issues/closed/20261013-macos-a-press-held-on-a.md` (#523), `host/apple/Sources/ExactKit/Mac/ChainingScrollView.swift`,
  `ScrollRoutingTests.testAnEmptyScrollPortFreedUnderAPressGoesWithItsNode`; round 12's
  [readouts](https://raw.githubusercontent.com/ccheever/exact2/d3c50deffc9d03407c6d3282e4e48f4bf9275513/adopt-main-fixes-r12/readouts.txt)
  (session 2, the bisect).
- RT-2: main `git log 0b956d63d..origin/main`; `EXACT2-GAPS.md` "Closed on main since round 11" and "Partly fixed";
  [r12-kept-workarounds](20261013-r12-kept-workarounds.md) KW-1 to KW-13 with each row's reference lines.
