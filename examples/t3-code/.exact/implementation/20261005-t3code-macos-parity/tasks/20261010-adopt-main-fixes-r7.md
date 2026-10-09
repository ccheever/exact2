---
name: 20261010-adopt-main-fixes-r7
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

# Main adoption, round 7: merge current main into the T3 branch and adopt its fixes

## Outcome

`feat(example)/t3-code` takes current `main` (224 commits past the last adopted main `e200397ec` on 2026-10-10) and adopts
the fixes listed in `examples/t3-code/STATUS.md` "Next up" item 4 and `plan.md` "Status, 2026-10-08" (round 7). The round
was blocked by X67 (main's examples sweep aborts on the clone at a 2 MiB test stack); the user decided on 2026-10-10 to
flatten the clone instead of waiting ([view-depth-under-test-stack](20261009-view-depth-under-test-stack.md)), so this
round starts after that task merges.

## Steps

1. `#297` was squash-merged, so first record the last adopted main without its content:
   `git merge -s ours e200397ec` (its framework content is already on the branch), then `git merge origin/main`.
   Resolve conflicts keeping the T3 side for `examples/t3-code/` and main's side for framework files; the branch carries
   no framework changes of its own (check with `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'`
   after the merge: only the workspace registration of the example crates may remain).
2. Adopt (STATUS "Next up" 4): main #305 (closes #300, X59: drop the clone's line-clamp workaround), main #304 (closes
   #285: drop the branch's `QUEUE.md` `clock +N real` entry), the `now()` → `performanceNow()` rename (main `9731c8056`,
   the clone's call sites), remove the comment-only `panels.contract` and `settings-panels.contract` (main's examples
   test refuses them), main #309 (#101: the clone's `isInspectable` follows main's development-only rule), #313 (SIGTERM
   through the orderly quit), #314 (the ContextMenu key: retire the clone's own handling where main's covers it) and #325
   (closes #286). #327 is still open: its retirements wait for a later round.
3. Re-read main's binding rules that changed since round 6 (`rules/RULES.md`, `rules/DEFERRED.md`, `docs/issues.md` — open
   framework issues now live in main's `issues/` folder) and record any that change the clone.
4. Update the issue records whose main fix landed (each X file's status line, `issues/README.md` buckets) and
   `EXACT2-GAPS.md` rows that a landed fix retires.

## Acceptance

| Row | How to verify |
| --- | --- |
| The branch contains main | `git merge-base --is-ancestor origin/main HEAD` after the merge (at the merge time) |
| main's examples sweep passes with the clone | `cargo test -p contract --test it button_migration` exit 0 (needs the flatten) |
| The five checks pass on the merged tree | CLAUDE.md's five checks once on the final head |
| The clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries; one live agent drive of the main surfaces (shell, thread, composer, right panel, Settings) with before/after screenshots |
| Each adopted fix is removed or kept with its reason | the task record lists each item with its commit |

## Next action

Start after view-depth-under-test-stack merges. The PR is squash-merged like the others, so record the main SHA this
round adopts in the record: the next round starts with `git merge -s ours <that SHA>`, as this round does for `e200397ec`.
