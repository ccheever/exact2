/-
The shipped VM machine (`runner/src/machine.rs`, extracted by Charon and
Aeneas into `VmExtract.Funs`) refines the semantics' VM model
(`semantics/Contract/Vm.lean`). The theorems, by file:

* `VmExtract.Equal`: `equal_ok` — `machine::equal` is `Value.equal`.
* `VmExtract.Step`: `step_ok` — one `machine::step` is one `Contract.Vm.step`.
* `VmExtract.Fix`: `machine_run_ok` — what `machine::run` returns, the model's
  interpreter returns (or the corresponding trap).

`#print axioms` below must list only Lean's own (`propext`,
`Classical.choice`, `Quot.sound`): no `sorryAx`.
-/
import VmExtract.Fix

#print axioms VmExtract.equal_ok
#print axioms VmExtract.step_ok
#print axioms VmExtract.machine_run_ok
