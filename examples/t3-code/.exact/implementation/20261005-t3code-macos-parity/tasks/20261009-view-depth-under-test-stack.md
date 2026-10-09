---
name: 20261009-view-depth-under-test-stack
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

# The clone's views nest fewer than 100 sites deep after inlining, so main's examples sweep compiles it on a test thread

## Outcome

main's source sweep (`contract/cli/tests/it/button_migration.rs`,
`every_app_corpus_and_example_compiles_and_the_button_migration_is_complete`) compiles every `.contract` under
`examples/` in-process on a 2 MiB test thread. An unoptimized compiler spends about 19 KiB of stack per nested view site
(`contract_lower::Lowerer::node`), so 103 nested sites abort the whole `it` test binary (SIGABRT). The clone's root
(75,582 nodes after inlining) nests deeper than that: it aborts at 2 MiB and passes at 4 MiB. So a main adoption round
(round 7 is blocked by this, plan.md "Status") and #99's clean app-only candidate on current main both fail the
`cargo test` check.

The framework fix is main's filesystem issue `issues/20261009-compiler-small-stack-depth.md` (X67, GitHub #320, P1,
open, no fix in progress). Its "Workarounds" name the clone-side one: "flatten an app's views to fewer than about 100
nested sites after inlining". This task does that, without changing what the app shows or does.

## Scope and exclusions

Included: measure the clone's deepest nesting after inlining (per root and per component chain), then remove nesting
that adds no layout: wrapper views with no style, handlers or semantics, single-child columns, components that wrap
their whole body in an extra view. Keep every testId, role, label and handler; keep the layout identical. Excluded:
compiler or test changes (no bigger test stacks: Charlie's decision on #320, "Do not merely enlarge test stacks").

## Context and guidance

- Measure: `cargo build -p contract --bin contract` (debug), then `(ulimit -s 2048; target/debug/contract build
  examples/t3-code/app.contract -o /tmp/p.plan)`; lower the depth until it passes with margin (target ≤ 90 sites).
  Find the deep chains with the plan's site depths or a small script over the inlined tree.
- Proof that nothing changed: the same agent `tree` (testIds, roles, labels) and the same screenshots before and after
  on the main surfaces (shell, thread, composer, right panel, Settings, a dialog).
- Order: run after the first audit fix wave merges (it touches many `.contract` files; parallel UI fixes would
  conflict). Then main adoption round 7 can merge main (plan.md "Status": record `e200397ec` with `git merge -s ours`,
  then merge main, adopt #305, #304, the `performanceNow()` rename, #309, #313, #314, #325).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| `contract build` of `app.contract` passes with `ulimit -s 2048` in a debug build | the command above, exit 0 (base: exit 134) | text |
| main's sweep passes with the clone | in a scratch worktree of main with `examples/t3-code` copied in: `cargo test -p contract --test it button_migration` | text |
| No visible or behavioral change | agent `tree` diff (testIds, roles, labels: empty) and screenshot pairs of six surfaces | before/after images |

## Decision needed

None for building it: it is a clone-side change named by the issue itself. Whether #99 waits for the framework fix
instead is the user's call at #99 time.

## Next action

Start after the first audit fix wave merges.
