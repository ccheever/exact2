# Contract compiler: unoptimized, 103 nested view sites overflow a 2 MiB thread (the parser admits 255), so the examples sweep aborts the whole test binary

**Status:** Open
**Systems:** Contract compiler
**Severity:** P1
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/320

## Current scope

Make admitted view/component depth fit a 2 MiB thread unoptimized; refuse excessive nesting by name before recursion aborts. Cover 255/256 sites, the 120-site component case and excessive chain in the example sweep. Do not merely enlarge test stacks.

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

The full file is [x67-deep.contract.txt](https://raw.githubusercontent.com/ccheever/exact2/3cfb286417554abc39f4abc842f5478352fd4356/file-x48-x68/x67-deep.contract.txt). The generators and every run are in the [record](https://raw.githubusercontent.com/ccheever/exact2/fe9850be2a881718e566952874aedcb017f8c297/file-x48-x68/x67-record.txt).

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Sweep test over `examples/` | copy the file to `examples/x67-deep/app.contract`; `cargo test -p contract --test it button_migration -- --nocapture` | macOS 26.6.2, Apple Silicon | main `b896050d7` | `thread 'button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete' has overflowed its stack`, `fatal runtime error: stack overflow, aborting`, `signal: 6, SIGABRT` | the file compiles (120 < 256), and the test passes | [record](https://raw.githubusercontent.com/ccheever/exact2/fe9850be2a881718e566952874aedcb017f8c297/file-x48-x68/x67-record.txt) §4 |
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
