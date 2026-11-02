/-
Opcodes on values: `Neg`, `Not`, `Unwrap`, `Field`, `Concat`, `Eq`, `Ne`.
-/
import VmExtract.ExecBasic

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option hygiene false in
/-- Close a step whose arm has run to its end: a trap or a push. -/
macro "vm_close" : tactic => `(tactic| (
  first
  | (casesm* _ ∧ _, Exists _
     subst_vars
     simp_all [Contract.Vm.exec, Contract.Vm.pop1, Contract.Vm.pop2, Contract.Vm.binary, Out,
       TrapRel, opName, Rel])
  | (split at hx
     · simp at hx
       casesm* _ ∧ _, Exists _
       subst_vars
       simp_all [Contract.Vm.exec, Contract.Vm.pop1, Contract.Vm.pop2, Contract.Vm.binary, Out,
         TrapRel, opName, Rel]
     · simp at hx)))

set_option hygiene false in
/-- Pop the top: an empty stack traps on both sides; else `a` and `S`. -/
macro "vm_pop1" : tactic => `(tactic| (
  obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
  simp only [hp, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨a, S⟩
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop1, Contract.Vm.pop2, Out, TrapRel]
  simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair] at hx
  simp only [List.tail_cons] at hv1))

set_option hygiene false in
/-- Pop the second: `b` the top, `a` below it, `S` the rest. -/
macro "vm_pop2" : tactic => `(tactic| (
  obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
  simp only [hp, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨b, S⟩
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop2, Out, TrapRel]
  simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair] at hx
  simp only [List.tail_cons] at hv1
  obtain ⟨v2, hv2, hp2⟩ := pop_rev hv1 m.pos
  simp only [hp2, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨a, S⟩
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop2, Out, TrapRel]
  simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair] at hx
  simp only [List.tail_cons] at hv2))

set_option maxHeartbeats 8000000

theorem exec_neg : ExecOk .neg := by
  vm_intro
  vm_pop1
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> simp [KindOf, push_eq] at hx <;> vm_close

theorem exec_not : ExecOk .not := by
  vm_intro
  vm_pop1
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> simp [KindOf, push_eq] at hx <;> vm_close

theorem exec_unwrap : ExecOk .unwrap := by
  vm_intro
  vm_pop1
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> simp [KindOf, push_eq] at hx <;> vm_close

theorem exec_concat : ExecOk .concat := by
  vm_intro
  vm_pop2
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> cases b <;> simp [KindOf, push_eq, concatOf] at hx <;> vm_close

theorem exec_eq : ExecOk .eq := by
  vm_intro
  vm_pop2
  rw [bind_eq_ok'] at hx
  obtain ⟨o, ho, hx⟩ := hx
  have := equal_ok _ _ _ ho
  subst this
  simp only [Contract.Vm.exec, Contract.Vm.pop2, Contract.Vm.binary, except_bind_ok]
  rcases hq : Value.equal a b with _ | e <;> simp only [hq] at hx ⊢ <;>
    simp [push_eq, Option.elim] at hx ⊢ <;> vm_close

theorem exec_ne : ExecOk .ne := by
  vm_intro
  vm_pop2
  rw [bind_eq_ok'] at hx
  obtain ⟨o, ho, hx⟩ := hx
  have := equal_ok _ _ _ ho
  subst this
  simp only [Contract.Vm.exec, Contract.Vm.pop2, Contract.Vm.binary, except_bind_ok]
  rcases hq : Value.equal a b with _ | e <;> simp only [hq] at hx ⊢ <;>
    simp [push_eq, Option.elim] at hx ⊢ <;> vm_close

end VmExtract
