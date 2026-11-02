/-
The jumps: `Jump`, `JumpIfFalse`, `JumpIfNone`.
-/
import VmExtract.ExecValue
import VmExtract.Helpers

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option maxHeartbeats 8000000

theorem codeLen_ok {h : LHost} {len : Usize} (hl : LHost.codeLen h = ok len) :
    len.val = h.code.length := tryMk_ok hl

set_option hygiene false in
/-- The arm jumps to operand `x` (the model's `pc + 1 + off`): the same target
or the same refusal. `hk` names the machine the arm continues with. -/
macro "vm_jump" : tactic => `(tactic| (
  simp only [hostInst] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨len, hlen, hx⟩ := hx
  have hlv := codeLen_ok hlen
  rw [bind_eq_ok'] at hx
  obtain ⟨r0, hj, hx⟩ := hx
  have hjs := jump_to_spec (cs := alloc.vec.Vec.deref m.callbacks) (by simpa using hcb) x m.pos hj
  rw [hlv, hc, hv, hpos] at hjs
  cases hJ : Contract.Vm.jumpTo code.length cbs (pc + 1 + off) <;> rw [hJ] at hjs <;>
    simp only [except_bind_ok, except_bind_error]
  · obtain ⟨e', rfl, hrel⟩ := hjs
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    exact ⟨_, rfl, hrel⟩
  · obtain ⟨u, rfl, hu⟩ := hjs
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp only [Out, Rel, true_and]
    simp_all))

theorem exec_jump (off : Nat) : ExecOk (.jump off) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [Contract.Vm.exec]
  vm_jump

theorem exec_jumpIfFalse (off : Nat) : ExecOk (.jumpIfFalse off) := by
  vm_intro
  have hv := (operand_ok hx0).1
  vm_pop1
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> simp only [KindOf, exists_eq_left] at hx
  case bool b =>
    cases b
    · simp only [Contract.Vm.exec, Contract.Vm.pop1, except_bind_ok, Bool.false_eq_true,
        if_false] at hx ⊢
      vm_jump
    · simp at hx ⊢
      obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Contract.Vm.exec, Contract.Vm.pop1, Out, Rel]
  all_goals (simp at hx; vm_close)

theorem exec_jumpIfNone (off : Nat) : ExecOk (.jumpIfNone off) := by
  vm_intro
  have hv := (operand_ok hx0).1
  rcases S with _ | ⟨a, S⟩
  · have h0 : alloc.vec.Vec.len m.stack = 0#usize := by apply UScalar.val_eq_imp; simp [hst]
    simp only [h0, if_true] at hx
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Out, TrapRel]
  have hne : ¬ (alloc.vec.Vec.len m.stack = 0#usize) := by
    intro h0; have := congrArg UScalar.val h0; simp [hst] at this
  simp only [hne, if_false] at hx
  obtain ⟨i1, hi1, hsub⟩ := usize_sub_eq (alloc.vec.Vec.len m.stack) 1#usize (by simp [hst])
  have hidx : i1.val < m.stack.val.length := by simp at hi1; simp [hst] at hi1 ⊢; omega
  simp only [hsub, bind_ok, vec_index_eq _ _ hidx] at hx
  have htop : m.stack.val[i1.val] = a := by
    simp only [hst] at hidx ⊢
    simp at hi1; simp [hst] at hi1
    simp [List.getElem_append_right, hi1]
  rw [htop] at hx
  simp only [valInst_kind, kindOf_bind] at hx
  cases a <;> simp only [KindOf, exists_eq_left] at hx
  case none =>
    simp only [Contract.Vm.exec] at hx ⊢
    vm_jump
  case some w =>
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp_all [Contract.Vm.exec, Out, Rel]
  all_goals (simp at hx; vm_close)

end VmExtract
