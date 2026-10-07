/-
Opcodes that push a constant or a read of the environment, and the stack's
own: `Number`, `Bool`, `Str`, `None`, `Unit`, `Some`, the loads, `Pop`.
-/
import VmExtract.ExecArith

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option hygiene false in
/-- Introduce a step: the model's machine taken apart, the instruction's
operands named, `machine::exec` reduced to the opcode's arm. -/
macro "vm_intro" : tactic => `(tactic| (
  intro code env h m M ins next r h' m' hR hins hn hat hx
  obtain ⟨hc, he, hfx, hpos, hst, hloc, hcb⟩ := hR
  simp only [encIns] at hins
  obtain ⟨x, y, z, hl, hx0, hy0, hz0, rfl⟩ := mkIns_ok hins
  rw [machine.exec] at hx
  simp only [arr3_0, bind_ok] at hx
  obtain ⟨pc, S, L, cbs, fx⟩ := M
  simp only at hfx hpos hst hloc hcb hn hat ⊢))

set_option hygiene false in
/-- The arm pushed `v` onto the stack and ran on: the model did the same. -/
macro "vm_push" : tactic => `(tactic| (
  simp only [push_eq] at hx
  split at hx
  · simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp_all [Contract.Vm.exec, Out, Rel]
  · simp at hx))

set_option maxHeartbeats 4000000

theorem exec_numc (b : UInt64) : ExecOk (.num b) := by
  vm_intro
  simp only [valInst_number, bind_ok] at hx
  vm_push

theorem exec_none : ExecOk .none := by
  vm_intro
  simp only [valInst_none, bind_ok] at hx
  vm_push

theorem exec_unit : ExecOk .unit := by
  vm_intro
  simp only [valInst_unit, bind_ok] at hx
  vm_push

theorem exec_bool (b : Bool) : ExecOk (.bool b) := by
  vm_intro
  have hv := (operand_ok hx0).1
  have : (x != 0#u64) = b := by
    cases b <;> simp at hv <;> simp [UScalar.eq_equiv, hv]
  simp only [valInst_boolean, bind_ok, this] at hx
  vm_push

theorem exec_pop : ExecOk .pop := by
  vm_intro
  obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
  simp only [hp, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨a, S⟩
  · simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop1, Out, TrapRel]
  · simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp_all [Contract.Vm.exec, Contract.Vm.pop1, Out, Rel]

theorem exec_str (s : String) : ExecOk (.str s) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.string, lookup, hc, hv, hpos, hat] at hx
  simp only [bind_ok, branch_ok] at hx
  vm_push

theorem exec_some : ExecOk .some := by
  vm_intro
  obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
  simp only [hp, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨a, S⟩
  · simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop1, Out, TrapRel]
  · simp only [List.head?_cons, branch_ok, bind_ok, hostInst, uncurry_apply_pair, push_eq] at hx
    split at hx
    · simp at hx
      obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Contract.Vm.exec, Contract.Vm.pop1, Out, Rel]
    · simp at hx

theorem exec_loadSlot (j : Nat) : ExecOk (.loadSlot j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadSlot, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadDerive (j : Nat) : ExecOk (.loadDerive j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadSettled, lookup, he, hv, bind_ok, uncurry_apply_pair] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadResource (j : Nat) : ExecOk (.loadResource j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadSettled, lookup, he, hv, bind_ok, uncurry_apply_pair] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadParam (j : Nat) : ExecOk (.loadParam j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadParam, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadItem (j : Nat) : ExecOk (.loadItem j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadFrame, LHost.frameRead, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadBound (j : Nat) : ExecOk (.loadBound j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadFrame, LHost.frameRead, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_loadIndex (j : Nat) : ExecOk (.loadIndex j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.loadFrame, LHost.frameRead, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  cases heq : (env.frames[j]?.bind (·.index)) <;>
    simp only [heq, Option.map_none, Option.map_some, bind_ok, branch_ok, branch_err,
      residual_err] at hx ⊢
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  · vm_push

theorem exec_pendingMutation (k : Nat) : ExecOk (.pendingMutation k) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, lookup, he, hv, bind_ok, valInst_boolean] at hx
  vm_push

theorem exec_pendingResource (k : Nat) : ExecOk (.pendingResource k) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.resourceFlag, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp only [valInst_boolean, bind_ok] at hx; vm_push
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    first | simp [Out, TrapRel] | (split <;> simp_all [Out, TrapRel])

theorem exec_failedResource (k : Nat) : ExecOk (.failedResource k) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, LHost.resourceFlag, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i heq <;> simp only [heq, bind_ok, branch_ok, branch_err, residual_err] at hx ⊢
  · simp only [valInst_boolean, bind_ok] at hx; vm_push
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    first | simp [Out, TrapRel] | (split <;> simp_all [Out, TrapRel])

theorem exec_refresh (k : Nat) : ExecOk (.refresh k) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, bind_ok] at hx
  simp at hx
  obtain ⟨rfl, rfl, rfl⟩ := hx
  simp_all [Contract.Vm.exec, Out, Rel]

theorem exec_bindLocal : ExecOk .bindLocal := by
  vm_intro
  obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
  simp only [hp, bind_ok, uncurry_apply_pair] at hx
  rcases S with _ | ⟨a, S⟩
  · simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Contract.Vm.exec, Contract.Vm.pop1, Out, TrapRel]
  · simp only [List.head?_cons, branch_ok, bind_ok, push_eq] at hx
    split at hx
    · simp at hx
      obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Contract.Vm.exec, Contract.Vm.pop1, Out, Rel]
    · simp at hx

theorem exec_loadLocal (j : Nat) : ExecOk (.loadLocal j) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  have hc' := cast_usize_val x (by omega)
  simp only [lift, bind_ok] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i hi
  · simp only [UScalar.lt_equiv, alloc.vec.Vec.len_val, hc', hv] at hi
    have hi' : j < L.length := by rw [← hloc]; exact hi
    rw [vec_index_eq _ _ (by rw [hc', hv]; exact hi)] at hx
    simp only [bind_ok, valInst_clone] at hx
    simp only [List.getElem?_eq_getElem hi']
    have he' : m.locals.val[(UScalar.cast .Usize x).val]'(by rw [hc', hv]; exact hi) = L[j] := by
      simp [hc', hv, hloc]
    rw [he'] at hx
    vm_push
  · simp only [UScalar.lt_equiv, alloc.vec.Vec.len_val, hc', hv, not_lt] at hi
    have hi' : L.length ≤ j := by rw [← hloc]; exact hi
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [List.getElem?_eq_none hi', Out, TrapRel]

end VmExtract
