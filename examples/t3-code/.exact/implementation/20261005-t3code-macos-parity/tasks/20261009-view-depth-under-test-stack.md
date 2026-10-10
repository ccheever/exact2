---
name: 20261009-view-depth-under-test-stack
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-view-depth-under-test-stack
pr_url: https://github.com/ccheever/exact2/pull/382
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

## Cause and fix

Measured first (a lldb backtrace at the base overflow, then a scratch copy of `contract-lower` that records the stack
pointer per lowered node; neither is committed). Unoptimized, `Lowerer::nodes` + `Lowerer::node` cost 19,328 bytes per
nested site and `expr::compile` 12,720 bytes per nested expression level, and the two add up at each node: a child's
props and derives are substituted into its view, so an icon's `name == "a" ? … : name == "b" ? …` lookup (40 to 62
links) nested the caller's whole `name` expression under every comparison. The base overflowed at 68 sites plus 56
expression levels; its peak was 2,303 KiB at site depth 80 (the PR code-review thread › comment Markdown › table cell ›
context chip › icon route). The plan's own site depth was only 80, so the expression half had to go too.

All three changes are mechanical (made by scripts, kept on one line per lookup) and change no element:

1. Lookup chains of 8 or more links on one subject (`S == "lit"`, or `S == "a" or S == "b"`) are grouped:
   `includes([...], S) ? (S == "a" ? x : … : z) : includes([...], S) ? (…) : default`, about √N groups of √N links, so
   the same first match nests about 2√N deep (24 chains in `icons`, `timeline-icons`, `shell-icons`, `sidebar-icons`,
   `pages-icons`, `palette`, `r4-surfaces-icons`, `provider-icons`, `settings-core`, `settings-appearance-look`,
   `settings-b-kit`, `browser-surface` (line 36, outside #349's hunks), `timeline-files` and `markdown`'s chip/syntax
   fns). Every literal, an unknown value and "" give the same result before and after (1,404 values, 0 mismatches).
2. The icon components' lookups are `fn`s (`iconGeometry`, `timelineIconGeometry`, `shellIconGeometry`,
   `sidebarIconGeometry`, `paletteIconGeometry`, `fileTypeIconTint/D0/O0/R0/D1`): a call reads its argument once into a
   local, so a deep caller expression is no longer nested under each comparison.
3. View `when … else when … else` chains on the deep routes are sibling `when`s with exclusive guards (`markdown`:
   BlockBody, FlowRuns, ResolvedChip, ChipFace, TableCell, CellRuns, the item marker; `media-markdown`'s two;
   TimelineIcon's). A region draws nothing, so the children and their order are unchanged; each link is one region
   level instead of one per link. Same-subject literal tests read as `block.kind == "item"` and, for the last branch,
   `not includes([...], block.kind)`.

Result: peak lowering stack 2,303 → 1,535 KiB (75% of 2 MiB), deepest site 80 → 67, the same 109,849 plan nodes
(regions 41,379 → 43,947), plan 27,407,807 → 27,043,895 bytes. No wrapper element was removed: none was needed, and
removing styled columns would risk the layout. `timeline.contract` and the attachment dialog were flattened in a trial
and reverted: their routes peak under 900 KiB.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| `contract build` of `app.contract` passes with `ulimit -s 2048` in a debug build | pass: base exit 134; after exit 0 at 2048, and at 1792 and 1664 (margin) | [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/5425cc53bb9d66f054084a76b913236f599c3b47/view-depth-under-test-stack/evidence.txt) §1 |
| main's sweep passes with the clone | pass, with main's own language moves applied in the scratch copy only: main `67a1892b6`, `cargo test -p contract --test it button_migration` exit 0, "90 roots, 192 imported source files, 0 failures". As copied: exit 101 with no abort, 2 failures that are main's language moves, not depth (`r6-device.contract:323` `now()`, round 7's rename; `.exact/…/rem-probe.contract:24` a `calc()` height). main's debug CLI: base exit 134 at 2048, after exit 0 at 2048 and 1664. The scratch worktree is removed. | [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/5425cc53bb9d66f054084a76b913236f599c3b47/view-depth-under-test-stack/evidence.txt) §3 |
| No visible or behavioral change | pass: same lane, the base and this build: agent `tree` diff of testIds/roles/labels empty on all six surfaces (175/172/171/161/152/531 entries), and the full node diff (type, depth, handlers, every prop) empty too; screenshots pixel-identical | [Shell](https://raw.githubusercontent.com/ccheever/exact2/4f8a7b84a02fb9e7e4f8896d77c4ac874d902e95/view-depth-under-test-stack/1-shell.png), [Thread](https://raw.githubusercontent.com/ccheever/exact2/a4fac68335f08ef0d509bb7f07b0dae9afb5b552/view-depth-under-test-stack/2-thread.png), [Composer](https://raw.githubusercontent.com/ccheever/exact2/1a47f5e48b8b074964005c3d470ac1e6b78100fe/view-depth-under-test-stack/3-composer.png), [Right panel](https://raw.githubusercontent.com/ccheever/exact2/12d759635f85cc8ea2729374b634359ec48f7d6b/view-depth-under-test-stack/4-right-panel.png), [Dialog](https://raw.githubusercontent.com/ccheever/exact2/1134e51bd6a1617f65c47f8d25da07a873befd88/view-depth-under-test-stack/5-dialog.png), [Settings](https://raw.githubusercontent.com/ccheever/exact2/0097941146053b2526f123e417d9a450ac1447e6/view-depth-under-test-stack/6-settings.png); [evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/5425cc53bb9d66f054084a76b913236f599c3b47/view-depth-under-test-stack/evidence.txt) §5 |

## Tests

No test files changed (structural change; no new apparatus). Run during development: the Bun tests that read the
changed `.contract` files (`browser-surface`, `r4-surfaces`, `settings-a-about`, `timeline-work-rows`, `timeline`,
`menu-keys`, `r11-misc-ts-decl`, `r8-keys`, `audit-wave-followups`, `sidebar-palette-keys`, `settings-labels`,
`settings-escape`, `theme-color-picker`): 272 pass. Final head: see the PR's checks.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | the base `1b848a8be`, built in this worktree before any edit (the shared `t3-code-evidence-base` sits at `950e8e2e5`, 38 commits behind, so its tree could not diff empty) | the first run stopped at `tap command-palette-trigger` (not drawn; the dialog step became `tap add-project`); the second completed | `target/vdepth/drive/before` (worktree) |
| After drive (the live drive) | `f69ab9414`'s bundle | on the fresh lane `view-depth-under-test-stack`: thread, composer, right panel and dialog identical; shell and Settings differed only in lane data (the lane's project id and path, and Claude's effort label "Medium · 1M" with a context-window choice from the real `claude` CLI in that lane). The one retry, on the before lane: identical everywhere | the six images above |

## Found, not changed

- main's sweep also needs, from round 7 or #99's cleanup: the `now()` → `performanceNow()` rename (`app.contract` 19
  uses, `r6-device`, `settings-appearance-editor`, `theme-color-picker`) and the `.exact/` evidence probe
  `evidence/20261007-interface-font-size/rem-probe.contract` gone (main refuses its `calc(1rem + 4px)` height, and the
  sweep compiles every `.contract` under `examples/`).
- The deepest route left is the PR code-review thread (67 sites: 43 before the comment's Markdown). Merging #349 or new
  surfaces deeper than that eats the margin; rerun the 2048/1664 compile when a route grows.
- One expression in the Browser surface nests about 50 levels (919 KiB at site depth 16); harmless at that depth.

## Next action

The coordinator reviews the draft PR [#382](https://github.com/ccheever/exact2/pull/382), merges it, then starts [adopt-main-fixes-r7](20261010-adopt-main-fixes-r7.md).
