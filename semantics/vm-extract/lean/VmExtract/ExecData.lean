/-
Opcodes that build, call, write and return: `List`, `Record`, `Call`,
`StoreSlot`, `Command`, `Send`, `NativeProps`, `Field`, `DropLocal`,
`Return`.
-/
import VmExtract.ExecJump

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option maxHeartbeats 8000000

theorem cast_small {ty ty'} (x : UScalar ty) (h : x.val < 2 ^ ty'.numBits) :
    (UScalar.cast ty' x).val = x.val :=
  UScalar.cast_val_mod_pow_of_inBounds_eq _ _ h

set_option hygiene false in
/-- `pop_n` of `n` (`hcv : cnt.val = n`) and what follows a push of the
built value; `hs` is what `pop_n_rev` says. -/
macro "vm_popn" : tactic => `(tactic| (
  rw [bind_eq_ok'] at hx
  obtain ⟨⟨r1, v1⟩, hpn, hx⟩ := hx
  have hs := pop_n_rev hst _ _ hpn
  rw [hcv] at hs))

theorem exec_list (n : Nat) : ExecOk (.list n) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  have hcv : (UScalar.cast .Usize x).val = n := by rw [cast_usize_val x (by omega), hv]
  simp only [lift, bind_ok] at hx
  vm_popn
  simp only [Contract.Vm.exec]
  cases hP : Contract.Vm.popN n S with
  | none =>
    rw [hP] at hs; subst hs
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  | some p =>
    obtain ⟨xs, s⟩ := p
    rw [hP] at hs
    obtain ⟨w, rfl, hw, hv1⟩ := hs
    simp only [branch_ok, bind_ok, hostInst, uncurry_apply_pair, push_eq] at hx
    split at hx
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Out, Rel]
    · simp at hx

theorem exec_record (shape : String) (n : Nat) : ExecOk (.record shape n) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  have hat' : lookup h.code x = some (.record shape n) := by simp [lookup, hc, hv, hpos, hat]
  simp only [hostInst, LHost.recordLen, LHost.record, hat'] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨rn, hrn, hx⟩ := hx
  rw [bind_eq_ok'] at hrn
  obtain ⟨n', hn', hrn⟩ := hrn
  simp only [ok.injEq] at hrn
  subst hrn
  have hcv : n'.val = n := tryMk_ok hn'
  simp only [branch_ok, bind_ok] at hx
  vm_popn
  simp only [Contract.Vm.exec]
  cases hP : Contract.Vm.popN n S with
  | none =>
    rw [hP] at hs; subst hs
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  | some p =>
    obtain ⟨xs, s⟩ := p
    rw [hP] at hs
    obtain ⟨w, rfl, hw, hv1⟩ := hs
    simp only [branch_ok, bind_ok, uncurry_apply_pair, push_eq] at hx
    split at hx
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Out, Rel]
    · simp at hx

theorem exec_command (nm : String) (n : Nat) : ExecOk (.command nm n) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  obtain ⟨hyv, hylt⟩ := operand_ok hy0
  have hcv : (UScalar.cast .Usize y).val = n := by rw [cast_usize_val y (by omega), hyv]
  have hat' : lookup h.code x = some (.command nm n) := by simp [lookup, hc, hv, hpos, hat]
  simp only [arr3_1, lift, bind_ok] at hx
  vm_popn
  simp only [Contract.Vm.exec]
  cases hP : Contract.Vm.popN n S with
  | none =>
    rw [hP] at hs; subst hs
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  | some p =>
    obtain ⟨xs, s⟩ := p
    rw [hP] at hs
    obtain ⟨w, rfl, hw, hv1⟩ := hs
    simp only [branch_ok, bind_ok, hostInst, LHost.command, hat'] at hx
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp_all [Out, Rel]

theorem exec_nativeProps (n : Nat) : ExecOk (.nativeProps n) := by
  vm_intro
  simp only [Contract.Vm.exec, Out]
  simp only [lift, bind_ok] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨k, _, hx⟩ := hx
  rw [bind_eq_ok'] at hx
  obtain ⟨⟨r1, v1⟩, _, hx⟩ := hx
  rcases r1 with w | e
  · simp [hostInst] at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    exact ⟨_, rfl, trivial⟩
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    exact ⟨_, rfl, trivial⟩

theorem exec_storeSlot (j : Nat) : ExecOk (.storeSlot j) := by
  vm_intro
  have hv := (operand_ok hx0).1
  simp only [hostInst, he, hv, bind_ok] at hx
  simp only [Contract.Vm.exec]
  by_cases hw : env.writable.contains j
  · simp only [hw, if_true, Bool.not_true, Bool.false_eq_true] at hx ⊢
    obtain ⟨v1, hv1, hp⟩ := pop_rev hst m.pos
    simp only [hp, bind_ok, uncurry_apply_pair] at hx
    rcases S with _ | ⟨a, S⟩
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp [Contract.Vm.pop1, Out, TrapRel]
    simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair, LHost.store, lookup,
      he, hv] at hx
    simp only [List.tail_cons] at hv1
    simp only [Contract.Vm.pop1, except_bind_ok]
    split at hx <;> rename_i ho <;> simp only [ho]
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp_all [Out, Rel]
    · split at hx <;> rename_i hs2
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp_all [Out, Rel]
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        split <;> simp_all [Out, TrapRel]
  · simp only [hw, if_false, Bool.not_false, if_true] at hx ⊢
    simp [lift] at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]

theorem exec_send (k : Nat) (src : String) (n : Nat) : ExecOk (.send k src n) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  obtain ⟨hyv, hylt⟩ := operand_ok hy0
  obtain ⟨hzv, hzlt⟩ := operand_ok hz0
  have hcv : (UScalar.cast .Usize z).val = n := by rw [cast_usize_val z (by omega), hzv]
  have hat' : lookup h.code y = some (.send k src n) := by simp [lookup, hc, hyv, hpos, hat]
  simp only [hostInst, LHost.mutationSlot, lookup, he, hv] at hx
  simp only [Contract.Vm.exec]
  split at hx <;> rename_i hms <;> simp only [hms]
  · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
  rename_i slot
  rw [bind_eq_ok'] at hx
  obtain ⟨rs, hrs, hx⟩ := hx
  rw [bind_eq_ok'] at hrs
  obtain ⟨s', hs', hrs⟩ := hrs
  simp only [ok.injEq] at hrs
  subst hrs
  have hsv : s'.val = slot := tryMk_ok hs'
  simp only [branch_ok, bind_ok, hsv] at hx
  by_cases hw : env.writable.contains slot
  · simp only [hw, if_true, Bool.not_true, Bool.false_eq_true, arr3_2, arr3_1, lift,
      bind_ok] at hx ⊢
    vm_popn
    cases hP : Contract.Vm.popN n S with
    | none =>
      rw [hP] at hs; subst hs
      simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp [Out, TrapRel]
    | some p =>
      obtain ⟨xs, s⟩ := p
      rw [hP] at hs
      obtain ⟨w, rfl, hw', hv1⟩ := hs
      simp only [branch_ok, bind_ok, LHost.send, hat'] at hx
      simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp_all [Out, Rel]
  · simp only [hw, if_false, Bool.not_false, if_true] at hx ⊢
    simp [lift] at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]

theorem exec_call (f : String) (n : Nat) : ExecOk (.call f n) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  have hat' : lookup h.code x = some (.call f n) := by simp [lookup, hc, hv, hpos, hat]
  simp only [hostInst, LHost.arity, LHost.call, hat'] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨rn, hrn, hx⟩ := hx
  rw [bind_eq_ok'] at hrn
  obtain ⟨n', hn', hrn⟩ := hrn
  simp only [ok.injEq] at hrn
  subst hrn
  have hcv : n'.val = n := tryMk_ok hn'
  simp only [branch_ok, bind_ok] at hx
  have hlen : (alloc.vec.Vec.len m.stack).val = S.length := by simp [hst]
  simp only [Contract.Vm.exec, Contract.Vm.popN]
  split at hx <;> rename_i hlt2
  · simp only [UScalar.lt_equiv, hlen, hcv] at hlt2
    simp only [hlt2, if_true]
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  · simp only [UScalar.lt_equiv, hlen, hcv, not_lt] at hlt2
    simp only [show ¬ S.length < n by omega, if_false]
    obtain ⟨at_, hat_, hsub⟩ := usize_sub_eq (alloc.vec.Vec.len m.stack) n' (by omega)
    rw [hlen, hcv] at hat_
    simp only [hsub, bind_ok, deref_val, hst] at hx
    have hargs : (S.reverse.drop at_.val) = (S.take n).reverse := by
      rw [List.drop_reverse]; congr 2; omega
    simp only [hargs, he] at hx ⊢
    split at hx <;> rename_i hr <;> simp only [hr]
    · simp only [branch_ok, bind_ok, uncurry_apply_pair, alloc.vec.Vec.truncate, push_eq] at hx
      split at hx
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        refine ⟨rfl, hc, he, hfx, hn, ?_, hloc, hcb⟩
        simp [hst, hat_, List.take_reverse]
        congr 2; omega
      · simp at hx
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp [Out, TrapRel]

theorem exec_field (j : Nat) : ExecOk (.field j) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  have hi16 : (UScalar.cast .U16 x).val = j := by
    rw [cast_small x (by rw [hv]; simpa [UScalarTy.numBits] using hlt), hv]
  have hiu : (UScalar.cast .Usize (UScalar.cast .U16 x)).val = j := by
    rw [cast_small _ (by rw [hi16]; exact Nat.lt_of_lt_of_le hlt (Nat.le_trans
      (Nat.pow_le_pow_right (by decide) (by decide)) pow32_le)), hi16]
  simp only [lift, bind_ok] at hx
  vm_pop1
  simp only [valInst_kind, kindOf_bind] at hx
  cases a
  case record shape fs =>
    simp only [KindOf] at hx
    obtain ⟨_, ⟨hfl, rfl⟩, hx⟩ := hx
    simp only [lift, bind_ok, UScalar.lt_equiv, hiu, Slice.len_val, Slice.from_val] at hx
    simp only [Contract.Vm.exec, Contract.Vm.pop1, except_bind_ok]
    split at hx <;> rename_i hj
    · simp only [Slice.length, Slice.from_val] at hj
      rw [slice_index_eq _ _ (by rw [hiu]; simpa using hj)] at hx
      simp only [bind_ok, valInst_clone, push_eq] at hx
      have : fs[j]? = some fs[j] := List.getElem?_eq_getElem (by simpa using hj)
      simp only [this]
      split at hx
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        simp_all only [Out, Rel, hiu, hi16, true_and]
        simp_all
        congr 1
        exact Nat.mod_eq_of_lt hlt
      · simp at hx
    · simp only [Slice.length, Slice.from_val, not_lt] at hj
      simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
      simp [List.getElem?_eq_none (by simpa using hj), Out, TrapRel]
  all_goals (simp [KindOf] at hx; vm_close)

theorem exec_ret : ExecOk .ret := by
  vm_intro
  simp only [alloc.vec.Vec.is_empty, bind_ok] at hx
  simp only [Contract.Vm.exec]
  rcases cbs with _ | ⟨c', cbs'⟩
  · have hnil := forall2_rev_nil hcb
    simp only [hnil, List.isEmpty_nil, if_true, alloc.vec.Vec.pop, bind_ok,
      uncurry_apply_pair, hst, List.getLast?_reverse] at hx
    rcases S with _ | ⟨a, S⟩
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp_all [Out]
    · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp_all [Out]
  · obtain ⟨c, rest, hl, _, _⟩ := forall2_rev_cons hcb
    simp [hl] at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]

theorem exec_dropLocal : ExecOk .dropLocal := by
  vm_intro
  simp only [Contract.Vm.exec]
  rcases cbs with _ | ⟨c', cbs'⟩
  · have hnil := forall2_rev_nil hcb
    have hn : ¬ (alloc.vec.Vec.len m.callbacks > 0#usize) := by simp [hnil]
    simp only [hn, if_false, alloc.vec.Vec.pop, bind_ok, uncurry_apply_pair, hloc] at hx
    rcases hL : L with _ | ⟨a, L'⟩
    · subst hL
      simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
    · rw [← hL]
      have hne : L.getLast? ≠ none := by simp [hL]
      have hne' : L.isEmpty = false := by simp [hL]
      simp only [hne', Bool.false_eq_true, if_false]
      cases hg : L.getLast? with
      | none => exact absurd hg hne
      | some w =>
        simp [hg] at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        simp_all [Out, Rel]
  · obtain ⟨c, rest, hl, hcr, _⟩ := forall2_rev_cons hcb
    have hn : alloc.vec.Vec.len m.callbacks > 0#usize := by simp [hl]
    simp only [hn, if_true] at hx
    obtain ⟨i1, hi1, hsub⟩ := usize_sub_eq (alloc.vec.Vec.len m.callbacks) 1#usize (by simp [hl])
    have hidx : i1.val < m.callbacks.val.length := by simp at hi1; simp [hl] at hi1 ⊢; omega
    simp only [hsub, bind_ok, vec_index_eq _ _ hidx] at hx
    have hlast : m.callbacks.val[i1.val] = c := by
      simp only [hl] at hidx ⊢
      simp at hi1; simp [hl] at hi1
      simp [List.getElem_append_right, hi1]
    rw [hlast] at hx
    rw [bind_eq_ok'] at hx
    obtain ⟨i2, hi2, hx⟩ := hx
    have hi2v := usize_add_ok' hi2
    obtain ⟨_, _, _, _, hbase, _⟩ := hcr
    simp only [UScalar.le_equiv, alloc.vec.Vec.len_val, hi2v, hbase, hloc] at hx
    split at hx <;> rename_i hle
    · simp only [show L.length ≤ c'.base + 2 by rw [← hloc]; simpa using hle, if_true]
      simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx; simp [Out, TrapRel]
    · have hgt : ¬ L.length ≤ c'.base + 2 := by rw [← hloc]; simpa using hle
      simp only [hgt, if_false]
      simp only [alloc.vec.Vec.pop, bind_ok, uncurry_apply_pair, hloc] at hx
      cases hg : L.getLast? with
      | none => simp at hg; subst hg; simp at hgt
      | some w =>
        simp [hg] at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        simp_all [Out, Rel]

end VmExtract
