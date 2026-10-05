/-
`machine::run` returns only what a finite run of steps returns: the extracted
`run_loop` is a `partial_fixpoint`, and fixpoint induction (for a predicate
on its returned values, which is admissible) gives `RRun`.
-/
import VmExtract.Run

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract Lean.Order
open Aeneas.Data.Coinductive
open Contract (Value)

unseal Result in
/-- A property of every returned value survives the supremum of a chain:
a chain whose supremum returns `y` has an element returning `y`. -/
theorem ok_admissible {α} (P : α → Prop) :
    admissible (fun x : Result α => ∀ y, x = ok y → P y) := by
  intro c hc hall y hy
  by_cases hex : ∃ a, c (ok a)
  · obtain ⟨a, ha⟩ := hex
    have hle := le_csup hc ha
    rw [hy] at hle
    have : a = y := ITree.le_ret_inj _ _ hle
    subst this
    exact hall _ ha a rfl
  · exfalso
    have hdiv : ∀ x, c x → PartialOrder.rel x (Result.div : Result α) := by
      intro x hx
      have hle := le_csup hc hx
      rw [hy] at hle
      have hle' := Eq.mp (ITree.le_unfold x (ok y)) hle
      rcases hle' with h | ⟨r, h1, h2⟩ | ⟨i, _, _, _, h2, _⟩
      · rw [h]; exact PartialOrder.rel_refl
      · have hr : c (ITree.ret r) := h1 ▸ hx
        exact absurd ⟨r, hr⟩ hex
      · exact absurd h2 (by simp [Result.ok])
    have hle := csup_le hc hdiv
    rw [hy] at hle
    have := ITree.le_div_is_div _ hle
    simp [Result.ok] at this

/-- What `run` returns: the outcome, the host, the machine. -/
abbrev RT := core.result.Result Value machine.Trap × LHost × machine.Machine Value Unit

/-- **Whatever the extracted `machine::run` returns is the end of a finite
run of extracted steps.** With `run_ok`: whatever the shipped machine's run
returns, from a state corresponding to the model's, the model's interpreter
returns too (or traps correspondingly). -/
theorem rrun_of_machine_run (h : LHost) (m : machine.Machine Value Unit) {res h' m'}
    (hr : machine.run valInst hostInst h m = ok (res, h', m')) : RRun h m res h' m' := by
  have key : ∀ (h : LHost) (m : machine.Machine Value Unit) y,
      machine.run_loop valInst hostInst h m = ok y → RRun h m y.1 y.2.1 y.2.2 := by
    refine machine.run_loop.fixpoint_induct valInst hostInst
      (fun f => ∀ h m y, f h m = ok y → RRun h m y.1 y.2.1 y.2.2) ?_ ?_
    · apply admissible_pi_apply (β := fun _ : LHost => machine.Machine Value Unit → Result RT)
        (P := fun h (g : machine.Machine Value Unit → Result RT) =>
          ∀ m (y : RT), g m = ok y → RRun h m y.1 y.2.1 y.2.2)
      intro h
      apply admissible_pi_apply (β := fun _ : machine.Machine Value Unit => Result RT)
        (P := fun m (x : Result RT) => ∀ (y : RT), x = ok y → RRun h m y.1 y.2.1 y.2.2)
      intro m
      exact ok_admissible _
    · intro f ih h m y hy
      rw [bind_eq_ok'] at hy
      obtain ⟨⟨r, h1, m1⟩, hs, hy⟩ := hy
      rcases r with v | e
      · rcases v with _ | v
        · simp only [branch_ok, bind_ok, uncurry_apply_pair] at hy
          exact .next hs (ih h1 m1 y hy)
        · simp only [branch_ok, bind_ok, uncurry_apply_pair, ok.injEq] at hy
          subst hy
          exact .done hs
      · simp only [branch_err, bind_ok, uncurry_apply_pair, residual_err, ok.injEq] at hy
        subst hy
        exact .trap hs
  exact key h m (res, h', m') hr

/-- **The whole-run refinement**, for the extracted `machine::run` as Aeneas
translates it: from a state corresponding to the model's, when the shipped
`run` returns value `v`, the model's interpreter (`Contract.Vm.runFrom`, with
enough fuel) returns `v` with the same effects; when it traps, the model traps
correspondingly. -/
theorem machine_run_ok (code env) (h : LHost) (m : machine.Machine Value Unit)
    (M : Contract.Vm.Machine) (hR : Rel code env h m M) {res h' m'}
    (hr : machine.run valInst hostInst h m = ok (res, h', m')) : RunOut code env M h' res :=
  run_ok code env (rrun_of_machine_run h m hr) M hR

end VmExtract
