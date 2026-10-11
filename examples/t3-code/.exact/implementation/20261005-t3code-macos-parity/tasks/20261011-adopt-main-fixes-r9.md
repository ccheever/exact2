---
name: 20261011-adopt-main-fixes-r9
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r9'
pr_url:
verified_commit:
---

# Main adoption, round 9: merge current main into the T3 branch

## Outcome

`feat(example)/t3-code` takes current `main`. Round 8 ([adopt-main-fixes-r8](closed/20261010-adopt-main-fixes-r8.md),
#402) adopted main `d413487a8` and landed by fast-forward, so main `d413487a8` is an ancestor of the T3 branch and this
round is a plain `git merge origin/main` (no `-s ours` step). On 2026-10-11 main had 128 commits the T3 branch
(`6db5cadea`) lacks (`d413487a8..dd589856e`), among them LLP 1116's bake-off rounds (D1–D8: the web's designed default
look, iOS layout and navigation, `state … persist`, the warning channel in `contract build`, headings in the emphasized
weight, `aria-value*` on any node (`43f6d298c`) and a determinate `progress value=… max=…` (`ab9110c3a`), exact:content,
patterns), resource answers kept as bytes (#441) or forgotten when too big (#432), `fetch`'s `redirect` (#415), CSS
`text-box` (#442), a percentage height bound on columns (#435), a press's screen laid out ahead of its release, idle
workers that cost nothing, and the issue files of the clone's X74–X83 (#405 … #430). The user decided on 2026-10-11 that
this round is built now (no further decision needed).

| Id | Clone | What this round does |
| --- | --- | --- |
| AD-1 | The branch lacks main's last 128 commits (`d413487a8..dd589856e` at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge that keeps main's ancestry); conflicts resolved keeping main's framework and the clone's `examples/t3-code`; the clone adapted to whatever main renamed or now refuses (Contract, main's examples sweep, the compiler stack depth X67); a clone workaround retired only when a merged main fix makes it unnecessary and the change is trivial, every other retirement listed for the coordinator. The clone's progress bars are not switched to main's determinate `progress` (a stacked follow-up task does that) |

## Steps

1. `git merge origin/main` at main's tip. Keep the T3 side for `examples/t3-code/` and main's side for framework files.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
2. Check what the clone relies on in main's new commits and record each with its outcome: Contract refusals and
   renames (`contract build`, main's examples sweep `cargo test -p contract --test it`), the new `contract build`
   warnings (LLP 1116 D6), headings' emphasized weight (D7), `aria-value*` and determinate `progress` (D8, #279: list,
   do not switch), the agent and build scripts the clone's tools call (`--help` and unknown-flag refusals, D4's
   stale-build rebuilds), resource answers (#441, #432), `fetch` `redirect` (#415), a press laid out ahead of its
   release, pictures and idle workers, `rules/` and the docs.
3. #327 (bucket 2): check whether it merged; if it did, its retirements (STATUS "Next up" 4) are listed or done.
4. Main's `issues/` holds the clone's framework gaps; check which of the clone's X records main closed.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 1 is empty | text |
| AD-1: the five checks pass on the merged tree | CLAUDE.md's five checks once on the final head | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries (README recipe), the macOS bundle; ONE agent-mode drive (launch, open a thread, Settings › Providers, Usage) on the before bundle (evidence-base) and on this branch's | before / after screenshots side by side |
| AD-1: each item of step 2 has an outcome | the record's table lists each with its commit, or "nothing to change" and why | this record |

## Reference

- Method: [adopt-main-fixes-r8](closed/20261010-adopt-main-fixes-r8.md) (Steps, "Adopted main", checks, Delivery) and
  [adopt-main-fixes-r7](closed/20261010-adopt-main-fixes-r7.md) (history; X67 and the flatten,
  [view-depth-under-test-stack](closed/20261009-view-depth-under-test-stack.md)).
- STATUS.md "Next up" item 4 (the #327 retirements list) and `issues/README.md`'s mapping table.
- The reference T3 Code source is not matched in this round (no clone feature changes); the drive compares the clone
  before and after the merge.
