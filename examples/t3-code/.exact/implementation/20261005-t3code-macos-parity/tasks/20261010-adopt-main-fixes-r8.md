---
name: 20261010-adopt-main-fixes-r8
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

# Main adoption, round 8: merge current main into the T3 branch

## Outcome

`feat(example)/t3-code` takes current `main`. Round 7 ([adopt-main-fixes-r7](closed/20261010-adopt-main-fixes-r7.md),
#384) adopted main `bc357d03c`; outside `examples/t3-code/` the branch's tree equals that commit except the workspace
registration in `Cargo.toml` and `Cargo.lock`. Main has 22 commits after it (`bc357d03c..1f127a788` when this record was
written; take main's tip at merge time). `git rev-list` reports hundreds more because #384 was squash-merged, which
drops main from the branch's ancestry; the content is already here.

#327 (bucket 2) is still open. Its retirements wait for round 9.

## Steps

1. Record the last adopted main without its content: `git merge -s ours bc357d03c`, then `git merge origin/main`.
   Keep the T3 side for `examples/t3-code/` and main's side for framework files. After the merge,
   `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
2. Check what the clone relied on in main's new commits, and record each with its outcome:
   - "A tap that names a node presses that node" (LLP 1012 §1, `a09f48a5e`, `e5d9b46f4`, `abc1eadff`): the agent's
     `tap <target>` now presses the target or refuses when its middle holds another control. Run the clone's agent-driven
     tests and one drive. A `tap` that is now refused moves to the named control or to `tap <target> at <x> <y>`.
   - A heading with no `aria-level` is level 2 (`f9dcdb1af`): check the clone's headings in the macOS accessibility tree
     and any test that reads a level.
   - LLP 1041 six workers for independent HTTP (`94dfb2332`, `1b31848f5`): note whether the clone's HTTP reads change.
     Nothing to change unless a test or the PR panel's one-at-a-time reads depend on the old bound.
   - Canvas host passes (`0685da1f0` … `5ee9ce1a0`, runner `collection_shown`): check whether the clone uses a
     collection that the runner change reaches. If so, the drive covers scrolling it.
   - Docs (`docs/start-here.md`, LLP 1115 D7) and `rules/`: re-read `rules/RULES.md` and `rules/DEFERRED.md` if they
     changed. Record any rule that changes the clone.
3. Main's `issues/` now holds the clone's framework gaps (#386, #394). Nothing to copy; `P/issues/README.md` already
   points there.

## Acceptance

| Row | How to verify |
| --- | --- |
| The branch contains main | `git merge-base --is-ancestor origin/main HEAD` after the merge (at the merge time) |
| The framework tree is main's | the `git diff` of step 1 is empty |
| The five checks pass on the merged tree | CLAUDE.md's five checks once on the final head |
| The clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries; one live agent drive of the main surfaces (shell, thread, composer, right panel, Settings) with before/after screenshots |
| Each item of step 2 has an outcome | the record's table lists each with its commit, or "nothing to change" and why |

## Next action

Start now. It does not wait for #327.
