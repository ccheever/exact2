# Contract compiler: unoptimized, 103 nested view sites overflow a 2 MiB thread (the language admits 256), so the examples sweep aborts the whole test binary

**Status:** Closed
**Resolution:** Every pass fits 256 nested sites on a 2 MiB unoptimized thread (lowerer ~19.7 to 5.3 KiB/site, parser 16.1 to 5.8) and 257 is refused by name; nested match/when/buttons, component chains, discarded and slotless fills are bounded before copying (contract/cli/tests/stack_depth.rs); the source sweep compiles the T3 Code clone (examples/t3-code from feat(example)/t3-code) without overflow
**Systems:** Contract compiler
**Severity:** P1
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/320

## Current scope

Make admitted view/component depth fit a 2 MiB thread unoptimized; refuse excessive nesting by name before recursion aborts. Cover 256 accepted/257 refused sites, the 120-site component case and excessive chain in the example sweep. Do not merely enlarge test stacks.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The compiler states its own stack budget in `contract/cli/tests/it/corpus.rs:1344-1347`: "Every pass recurses over the tree; all of it fits a 2 MB thread, as the test harness gives, even unoptimized." It then bounds its depth by name:
- the parser refuses views nested more than 256 sites deep (`syntax-view-depth`, `contract/syntax/src/parser.rs:1058-1061`);
- the inliner refuses component uses nested more than 32 deep (`syntax-inline-depth`, `contract/syntax/src/inline.rs:425`);
- the plan refuses more than 256 nested sites (`SiteTooDeep`).

The recursive passes run out of stack well before those limits, and a stack overflow aborts the whole process. There is no diagnostic to catch.

- **Unoptimized:** `contract_lower::Lowerer::node` (`contract/lower/src/lib.rs:620`) has an 18,960-byte frame, and `nodes` (`:612`) another 512, so each nested view site costs about 19 KiB. **103 nested sites abort on a 2 MiB thread**, though the parser admits 255. Nesting through component uses counts the same: 8 components of 15 columns each (120 sites) abort.
- **Optimized:** `contract_syntax::inline::inline_nodes` (about 1.5 KiB a frame) recurses through every node of every inlined component. Its 32-component limit and the plan's 256-site limit are checked too late. A chain of 40 components of 60 columns each aborts on a 2 MiB thread even in an optimized build, and is refused by name (`syntax-inline-depth`) only on 8 MiB.

The CLI runs on the main thread (8 MiB), which hides this. A Rust test thread gets 2 MiB. main's new source sweep, `every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` (`contract/cli/tests/it/button_migration.rs`, added in `cbd76f7d7`), compiles every `.contract` under `apps/`, `contract/corpus/` and `examples/` in-process. One deep example is enough to SIGABRT the whole `it` test binary of about 1,000 tests. The existing depth test, `deep_views_are_refused_without_aborting_the_compiler` (`contract/cli/tests/it/diagnostics.rs:1323`), runs the CLI on 8 MiB, so it does not see this.

The consumer is the T3 Code clone (`examples/t3-code` on `feat(example)/t3-code`): one root of 75,582 nodes after inlining. Its own branch's debug compiler aborts on it at 2 MiB and passes at 4 MiB, and main's compiler does the same. Merging main into that branch therefore makes `cargo test` abort in the sweep. The clone cannot land on main until the compiler fits its stated budget, or until the clone flattens its views to suit a test thread.

### Current and expected behavior

- **Current:** with `ulimit -s 2048`, `contract build` of a 103-deep single-file nesting exits 134 (`fatal runtime error: stack overflow, aborting`) in a debug build. A 102-deep one compiles. In the test harness, the sweep aborts with SIGABRT, and no test result is printed.
- **Expected:** every pass fits a 2 MiB thread, unoptimized, up to the limits the compiler admits:
  - 255 nested sites compile;
  - 256 or more are refused by name (`syntax-view-depth`), whether the nesting is in one file or built up through component uses;
  - a too-deep chain of uses is refused (`syntax-inline-depth` or a site-depth diagnostic), not aborted, on any stack the harness gives.

### Reproduction and evidence

The input is generated (`gen-chain.py 8 15`): 8 components, each 15 columns deep, each using the next at its deepest point, 120 nested sites in all. It starts:

```text
component App
  view
    column
      column
        …                       (15 columns)
                                  C1()

component C1
  view
    column
      …                         (15 columns, then C2(), … C7 ends in text "leaf")
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Sweep test over `examples/` | copy the file to `examples/x67-deep/app.contract`; `cargo test -p contract --test it button_migration -- --nocapture` | macOS 26.6.2, Apple Silicon | main `b896050d7` | `thread 'button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete' has overflowed its stack`, `fatal runtime error: stack overflow, aborting`, `signal: 6, SIGABRT` | the file compiles (120 < 256), and the test passes | record §4 |
| Control: the sweep without the file | remove `examples/x67-deep`; same command | same | same | `source sweep: 88 roots, 13 imported source files, 0 failures`, `ok` | — | same |
| Debug CLI, one file | `cargo build -p contract --bin contract`; `(ulimit -s 2048; target/debug/contract build nest103.contract -o p.plan)` with 103 nested `column`s | same | same | exit 134, stack overflow; 102 nested compile; 4 MiB passes 103 to 209; 8 MiB passes 255 | 255 compile on 2 MiB | same, §1–2 |
| Debug CLI, the 8×15 chain | `(ulimit -s 2048; target/debug/contract build deep.contract -o p.plan)` | same | same | exit 134; 8 MiB and the optimized build pass | compiles on 2 MiB | same, §1 |
| Optimized CLI, 40×60 chain | `cargo build --profile host-dev -p contract --bin contract`; `(ulimit -s 2048; target/host-dev/contract build chain-40-60.contract -o p.plan)` | same | same | exit 134; on 8 MiB: `` [syntax-inline-depth] component `C33` nests too deeply (a cycle?) `` | refused by name on 2 MiB | same, §1, §3b |
| Where the stack goes | `lldb` on the debug binary, 2 MiB, 103 nested | same | same | 231 frames: `Lowerer::node` 18,960 bytes and `Lowerer::nodes` 512 bytes per level, alternating, under `lower_with_sites` | — | same, §3 |

### Acceptance criteria

- In a 2 MiB thread, in an unoptimized build: 255 nested sites compile; 256 are refused with `syntax-view-depth`; a 120-site nesting built from component uses compiles; a 2,400-site chain of uses is refused by name.
- A test that compiles these in `std::thread::Builder::new().stack_size(2 << 20)`, as `corpus.rs:1344` does for expressions, guards it.
- main's sweep passes with the 8×15 file under `examples/`, and with the T3 Code clone merged in.

### Constraints and related work

- Workarounds:
  - run the sweep (or every test that compiles in-process) on a bigger thread, or set `RUST_MIN_STACK` for the `it` binary. This hides the bug from the tests only.
  - flatten an app's views to fewer than about 100 nested sites after inlining.
- Possible directions:
  - shrink `Lowerer::node`'s frame by moving its per-tag arms into separate non-inlined functions;
  - lower iteratively;
  - make the inliner count total site depth (not only component depth) and refuse before recursing.
- Not tested: compiles that run in-process outside the tests (the web build, the dev loop), and the other passes (types, analyze) at their own limits. The expression passes are already guarded by `deep_expressions_are_refused_by_name_on_a_small_stack`.

## Fix lane, 2026-10-11

The branch `issues/20261011-stack` adds a 2 MiB unoptimized integration test and
an ignored per-pass depth probe. Lowering prepares an element in a separate
nonrecursive frame; parsing splits view arms into separate frames; inlining
isolates component-use expansion. The first commits refused a 256th site;
review found this tightened the language, whose plan admits 256. The review
fix restores 256 accepted/257 refused in the parser, inliner, and provider
check. The parser counts a positional button's implicit text child, nested
match arm headers finish before recursive arm bodies, and the inliner checks
the remaining site budget iteratively before renaming a component view. The
provider path check borrows slot fill nodes instead of cloning their deep tree
with the caller's recursive frames live.

The probe binary-searches the largest successful depth for each pass, with
setup on an 8 MiB thread and the measured pass on the stated small stack.
Per-site costs are estimates from threshold differences (1 and 2 MiB before,
0.5 and 1 MiB after), not Rust frame-layout claims. The column rows' new
`≥256` entries are capped by the admitted language depth. The match row's
1 MiB threshold was remeasured after review; its before cost is Reviewer A's
binary measurement.

| Pass | Before: 2 MiB max | Before KiB/site | After: 2 MiB max | After KiB/site |
|---|---:|---:|---:|---:|
| Parse (columns) | 128 | 16.1 | ≥256 | 5.8 |
| Parse (nested match) | not probed | 9.6 (binary frame measurement) | 255 matches plus leaf (256 sites) | ~7.5 (136 matches fit 1 MiB; 137 overflow) |
| Inline | 125 | 16.3 | ≥256 | 5.3 |
| Types | 125 | 16.3 | ≥256 | 6.5 |
| Analyze | ≥255 | 4.1 | ≥256 | 4.1 |
| Lower | 102 | 19.7 | ≥256 | 5.3 |
| Plan validation | ≥255 | no measured growth | ≥256 | no measured growth |

The 2 MiB test now passes for 256 authored sites (columns, nested match and
when with their leaf, and positional buttons with their implicit text), a
named refusal at 257 sites, an 8×15 and 17×15 component chain, and a named
refusal for a 40×60 chain, App(254 columns)→Child(255 columns plus leaf), and
the same caller depth with a 255-site slot fill under Child's nested use.
The source sweep also passed with the 8×15 fixture temporarily under `examples/`
(91 roots, zero failures). The requested Contract, lower, and syntax suites,
Clippy, formatting, and caps passed. The semantics quick check could not run:
`lake` is not installed. The T3 Code clone is absent from this worktree, so
the merged-clone acceptance item remains unverified and this issue stays open.

## Review fix round 2, 2026-10-11

The pre-expansion site guard counted a 255-column slot fill even when its
component never rendered `children`, refusing an emitted view only three sites
deep. A 2 MiB regression test failed with `syntax-inline-depth` before the fix;
the guard now follows only fills that a callee renders, including forwarded
fills. A second regression showed a numeric provider accepted for a string
inject when `children` was forwarded through another slot component. The
provider walk now retains the enclosing fill and refuses that use with
`type-provide`. The same lost-fill handoff exists on `origin/main`.

The focused regressions, the Contract/lower/syntax suites with the T3 source
sweep skipped, Clippy, and formatting passed. The full source sweep was
started but did not finish with the untracked `examples/t3-code/` clone present;
it remains outside the branch and out of this commit. The semantics quick
check still cannot run because `lake` is unavailable. The issue remains open
until the clone acceptance item is verified on a merged tree.

## Review fix round 3, 2026-10-11

Both review findings reproduced. A discarded 255-column fill beneath 255
caller columns and 31 component uses aborted an unoptimized 2 MiB test thread;
the regression now compiles. A slot component with 128 nested columns and two
empty recursive uses did not finish within six seconds; it now returns
`syntax-inline-depth` promptly on the same small stack. The inliner computes
which slot components can render a fill with a bounded fixed point, including
recursive component graphs, then omits discarded fills before copying or
renaming them. The 2 MiB depth suite and Contract/lower/syntax suites passed,
including the app/example source sweep; Clippy and formatting passed. The
semantics quick check still cannot start because `lake` is unavailable. The
issue remains open for the merged T3 Code clone acceptance item.
