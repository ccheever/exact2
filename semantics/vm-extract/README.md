# The shipped VM, extracted and proved against the model

`runner/src/machine.rs` is the expression VM's machine: one step of
`vm::eval`'s loop (a callback body's end, else the instruction at the
position), generic over the value (`Val`) and over everything else a body
touches (`Host`: the code, the environment, the roster, the effects, the
resource bounds). `runner/src/vm.rs` instantiates it with `exact_plan::Value`
and the runner's `Env`; `compare::equal` is the machine's `equal`.

This directory extracts that file, as shipped, to Lean and proves it refines
`semantics/Contract/Vm.lean`:

```
runner/src/machine.rs ──#[path]── src/lib.rs (crate vm-extract, its own workspace)
        │ Charon (MIR → LLBC)            │ Aeneas (LLBC → pure Lean)
        └──────────────► lean/VmExtract/Types.lean, Funs.lean (generated, not committed)
lean/VmExtract/Model.lean, Host.lean   Val and Host for the model's Value, Env, Effects
lean/VmExtract/*.lean                  the proofs
```

## What is proved

All without `sorry`; `VmExtract.lean` prints each theorem's axioms, which
must be Lean's own (`propext`, `Classical.choice`, `Quot.sound`).

- `equal_ok` (`Equal.lean`): whenever the extracted `machine::equal` returns,
  it returns `Contract.Value.equal`.
- `step_ok` (`Step.lean`): on a Rust machine and host related to a model
  machine (`Rel`: same position, operand stack, locals, callbacks, effects, in
  the same code and environment), whenever the extracted `machine::step`
  returns, `Contract.Vm.step` runs on to a related machine, returns the same
  value with the same effects, or traps with the corresponding trap
  (`TrapRel`). Every opcode (`Exec*.lean`, one theorem each) and a callback
  body's end (`BodyEnd.lean`).
- `machine_run_ok` (`Fix.lean`, with `Run.lean`): whenever the extracted
  `machine::run` returns, the model's interpreter `Contract.Vm.runFrom`
  (with enough fuel) returns the same value and effects, or traps
  correspondingly. `run` is a `partial_fixpoint`; fixpoint induction shows
  what it returns is the end of a finite run of steps (`RRun`), and
  `machine_run_of` the converse.

The direction is soundness: what the shipped code computes, the model
computes. That the shipped code never panics (Aeneas's `fail`: an index out
of range, an arithmetic overflow, a vector past `usize::MAX`) is not proved.

## The trusted base

1. **Charon and Aeneas** translate the Rust faithfully (pinned below).
2. **`FunsExternal.lean`**: models of the standard functions Aeneas leaves
   out (`Vec::pop`, `truncate`, `split_off`, `is_empty`, `Default`,
   `mem::take`), each the documented behaviour over the vector's elements.
   `check.sh` refuses an extraction that asks for one not modelled.
3. **The instances.** The proofs are about the machine instantiated with
   `valInst` and `hostInst` (`Model.lean`, `Host.lean`); that the runner's
   `impl Val for Value` and `impl Host for Run` (`vm.rs`) behave as those do
   is checked by reading and by the differential tests, method by method:
   - numbers: `f64` is `Contract.F64` (`Prelude.lean`), and `Val::num`,
     `negate`, `num_eq` are `Contract.F64`'s operations (`%` is
     `Number.fmod`), which `Binary64Facts` proves correctly rounded and
     `difftest arith` checks against the hardware's `f64`;
   - `kind` is the constructor view, `str_eq` string equality (the runner's
     shared-allocation shortcut answers the same), `concat` string append;
   - the loads read what `difftest lowering` maps the runner's `Env` to
     (`Contract.Vm.Env`); `call` is `Contract.stdlib` (the corpus and random
     sweeps); `fetch` decodes as `Contract.Vm.decode` does, a jump's byte
     target being the instruction index there (`encIns`);
   - the resource bounds: the model's `steps`, `kept` and `concat` never
     refuse; the runner's may (`IterationLimit`, `ValueTooLarge`,
     `ValueTooDeep`, `StringTooLong`) — the refinement the semantics already
     names (`semantics/README.md`). `NativeProps` is outside the model: any
     outcome there corresponds to its `unsupported`.
4. **Lean.** `Contract/*.lean` is compiled here at Aeneas's Lean
   (`lean-toolchain`), not the semantics' own; `Contract.Vm` and its imports
   build under both.

## Running it

```
sh semantics/vm-extract/check.sh
```

regenerates the extraction and checks the proofs (a few minutes; Aeneas runs
`-sequential`). It skips, naming the missing tool, when one is absent. The
async lane's `semantics` step runs it.

Tools, outside the repo (as installed on the build Mac):

| | |
|---|---|
| Charon | `~/tools/charon` at `c8f15d7d658c86a95658f71ad99cddd4be002e04` (Aeneas's `charon-pin`), `cargo build --release` in `charon/` under its `nightly-2026-09-17` (`rustc-dev`, `llvm-tools`, `rust-src`), binaries copied to `charon/bin` |
| Aeneas | `~/tools/aeneas` at `557eff83ecef5083b98a52a94ca7fae63d6c1dab`, `gmake build-dev` with OCaml 5.3.0 (`opam switch create aeneas 5.3.0`, the packages its README lists; `brew install opam gmp pkgconf make`), `./charon` a link to the Charon checkout |
| Lean | Aeneas's `backends/lean`: `lake exe cache get && lake build` (Lean 4.31.0, mathlib) |

`CHARON`, `CHARON_TOOLCHAIN` and `AENEAS_DIR` override the paths.
