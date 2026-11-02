/-
Whole runs. The shipped `machine::run` is `loop { if let Some(v) = step(host, m)? {
return Ok(v) } }`: a run is a sequence of steps. `RRun` is that sequence for the
extracted `step`; every such run is one of the model's (`Contract.Vm.Runs`, so
`Contract.Vm.runFrom` with enough fuel), its traps the model's, and it is what
the extracted `machine::run` returns.
-/
import VmExtract.Step

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

/-- The extracted steps from `(h, m)` end, after any number of steps that run
on, in the step that returns (`.Ok v`) or traps (`.Err t`), with host `h'` and
machine `m'`. -/
inductive RRun : LHost → machine.Machine Value Unit → core.result.Result Value Trap → LHost →
    machine.Machine Value Unit → Prop
  | done {h m v h' m'} :
    machine.step valInst hostInst h m = ok (.Ok (Option.some v), h', m') → RRun h m (.Ok v) h' m'
  | trap {h m t h' m'} :
    machine.step valInst hostInst h m = ok (.Err t, h', m') → RRun h m (.Err t) h' m'
  | next {h m h₁ m₁ res h' m'} :
    machine.step valInst hostInst h m = ok (.Ok Option.none, h₁, m₁) → RRun h₁ m₁ res h' m' →
    RRun h m res h' m'

/-- What the model's interpreter does on a run the Rust finishes with `res`:
with enough fuel, the same value and effects, or the corresponding trap. -/
def RunOut (code : Contract.Vm.Code) (env : Contract.Vm.Env) (M : Contract.Vm.Machine) (h' : LHost) :
    core.result.Result Value Trap → Prop
  | .Ok v => ∃ n, Contract.Vm.runFrom code env n M = .ok (v, h'.fx)
  | .Err t => ∃ n t', Contract.Vm.runFrom code env n M = .error t' ∧ TrapRel t t'

/-- **The run refinement.** A run of the shipped machine from a state that
corresponds to the model's is a run of the model's interpreter: the same
value and effects, or the corresponding trap. -/
theorem run_ok (code env) {h m res h' m'} (hr : RRun h m res h' m') :
    ∀ M, Rel code env h m M → RunOut code env M h' res := by
  induction hr with
  | done hs =>
    intro M hR
    have := step_ok code env _ _ M hR hs
    simp only [RunOut]
    revert this
    cases hstep : Contract.Vm.step code env M with
    | error t => simp [Out]
    | ok st =>
      cases st with
      | run M' => simp [Out]
      | done v fx =>
        simp only [Out]; rintro ⟨hv, hfx⟩; cases hv
        exact ⟨1, by simp [Contract.Vm.runFrom, hstep, hfx]⟩
  | trap hs =>
    intro M hR
    have := step_ok code env _ _ M hR hs
    simp only [RunOut]
    revert this
    cases hstep : Contract.Vm.step code env M with
    | error t =>
      simp only [Out]; rintro ⟨t', ht, hrel⟩; cases ht
      exact ⟨1, t, by simp [Contract.Vm.runFrom, hstep], hrel⟩
    | ok st => cases st <;> simp [Out]
  | @next h m h₁ m₁ res h' m' hs _ ih =>
    intro M hR
    have := step_ok code env _ _ M hR hs
    revert this
    cases hstep : Contract.Vm.step code env M with
    | error t => simp [Out]
    | ok st =>
      cases st with
      | done v fx => simp [Out]
      | run M' =>
        simp only [Out, true_and]
        intro hR'
        have := ih M' hR'
        cases res with
        | Ok v =>
          obtain ⟨n, hn⟩ := this
          exact ⟨n + 1, by simp [Contract.Vm.runFrom, hstep, hn]⟩
        | Err t =>
          obtain ⟨n, t', hn, hrel⟩ := this
          exact ⟨n + 1, t', by simp [Contract.Vm.runFrom, hstep, hn], hrel⟩

/-- `RRun` is what the extracted `machine::run` returns. -/
theorem machine_run_of (h m res h' m') (hr : RRun h m res h' m') :
    machine.run valInst hostInst h m = ok (res, h', m') := by
  induction hr with
  | done hs =>
    rw [machine.run, machine.run_loop, hs]
    simp
  | trap hs =>
    rw [machine.run, machine.run_loop, hs]
    simp
  | next hs _ ih =>
    rw [machine.run, machine.run_loop, hs]
    simpa using ih

end VmExtract
