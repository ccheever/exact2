/-
The machine's helpers that read the callbacks and pop many: `pop_n`,
`jump_to`, and the innermost callback.
-/
import VmExtract.Lemmas

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

theorem slice_index_eq {α} (s : Slice α) (i : Usize) (hi : i.val < s.val.length) :
    s.index_usize i = ok s.val[i.val] := by
  simp only [Slice.index_usize]
  rw [show s[i]? = s.val[i.val]? from rfl, List.getElem?_eq_getElem hi]

theorem cast_u64_usize (u : Usize) : (UScalar.cast .U64 u).val = u.val := by
  apply UScalar.cast_val_mod_pow_of_inBounds_eq
  have h1 := u.hBounds
  have : UScalarTy.Usize.numBits ≤ UScalarTy.U64.numBits := by
    cases System.Platform.numBits_eq <;> simp [UScalarTy.numBits, *]
  exact Nat.lt_of_lt_of_le h1 (Nat.pow_le_pow_right (by decide) this)

theorem cast_usize_le (t : U64) (u : Usize) (h : t.val ≤ u.val) :
    (UScalar.cast .Usize t).val = t.val := by
  apply UScalar.cast_val_mod_pow_of_inBounds_eq
  exact Nat.lt_of_le_of_lt h u.hBounds

/-- The innermost callback of a vector related to the model's list. -/
theorem forall2_rev_cons {l : List (machine.Callback Value Unit)} {c' cbs}
    (h : List.Forall₂ CbRel l.reverse (c' :: cbs)) :
    ∃ c rest, l = rest ++ [c] ∧ CbRel c c' ∧ List.Forall₂ CbRel rest.reverse cbs := by
  obtain ⟨c, u, hc, hu, he⟩ := List.forall₂_cons_right_iff.mp h
  refine ⟨c, u.reverse, ?_, hc, by simpa using hu⟩
  have := congrArg List.reverse he
  simpa using this

theorem forall2_rev_nil {l : List (machine.Callback Value Unit)}
    (h : List.Forall₂ CbRel l.reverse []) : l = [] := by
  have := List.forall₂_nil_right_iff.mp h
  simpa using this

theorem pop_n_rev {v : alloc.vec.Vec Value} {S : List Value} (hv : v.val = S.reverse) (n : Usize)
    (trap : Trap) {r v'} (h : machine.pop_n v n trap = ok (r, v')) :
    match Contract.Vm.popN n.val S with
    | Option.none => r = .Err trap
    | Option.some (xs, s) => ∃ w : alloc.vec.Vec Value, r = .Ok w ∧ w.val = xs ∧ v'.val = s.reverse := by
  have hl : v.val.length = S.length := by simp [hv]
  have hl2 : v.length = S.length := hl
  simp only [machine.pop_n] at h
  unfold Contract.Vm.popN
  split at h
  · rename_i hlt
    simp only [UScalar.lt_equiv, alloc.vec.Vec.len_val] at hlt
    rw [if_pos (by omega)]
    simp at h
    exact h.1.symm
  · rename_i hge
    simp only [UScalar.lt_equiv, alloc.vec.Vec.len_val, not_lt] at hge
    rw [if_neg (by omega)]
    obtain ⟨at_, hat, hsub⟩ := usize_sub_eq (alloc.vec.Vec.len v) n (by simp; omega)
    simp only [hsub, bind_ok, alloc.vec.Vec.split_off] at h
    simp only [alloc.vec.Vec.len_val] at hat
    rw [if_pos (by omega)] at h
    simp only [bind_ok, uncurry_apply_pair, ok.injEq, Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    refine ⟨_, rfl, ?_, ?_⟩
    · simp only [alloc.vec.Vec.from_val, hv, hat, List.drop_reverse]
      congr 2
      simp; omega
    · simp only [alloc.vec.Vec.from_val, hv, hat, List.take_reverse]
      congr 2
      simp; omega

theorem jump_to_spec {len : Usize} {cs : Slice (machine.Callback Value Unit)}
    {cbs : List Contract.Vm.Callback} (hcb : List.Forall₂ CbRel cs.val.reverse cbs) (target : U64)
    (pc : Usize) {r} (h : machine.jump_to len cs target pc = ok r) :
    match Contract.Vm.jumpTo len.val cbs target.val with
    | .ok t => ∃ u : Usize, r = .Ok u ∧ u.val = t
    | .error e => ∃ e', r = .Err e' ∧ TrapRel e' e := by
  simp only [machine.jump_to, lift, bind_ok] at h
  unfold Contract.Vm.jumpTo
  rcases cbs with _ | ⟨c', cbs⟩
  · have := forall2_rev_nil hcb
    have hn : ¬ (Slice.len cs > 0#usize) := by simp [this]
    rw [if_neg hn] at h
    simp only [bind_ok, gt_iff_lt, UScalar.lt_equiv, cast_u64_usize] at h
    split at h
    · rename_i hgt
      simp at h; subst h
      simp [gt_iff_lt, hgt, TrapRel]
    · rename_i hle
      simp at h; subst h
      simp only [gt_iff_lt, hle, if_false]
      exact ⟨_, rfl, cast_usize_le target len (by omega)⟩
  · obtain ⟨c, rest, hl, hc, _⟩ := forall2_rev_cons hcb
    have hn : Slice.len cs > 0#usize := by simp [hl]
    rw [if_pos hn] at h
    obtain ⟨i1, hi1, hsub⟩ := usize_sub_eq (Slice.len cs) 1#usize (by simp [hl])
    have hidx : i1.val < cs.val.length := by simp at hi1; simp [hl] at hi1 ⊢; omega
    simp only [hsub, bind_ok, slice_index_eq _ _ hidx] at h
    have hlast : cs.val[i1.val] = c := by
      simp only [hl] at hidx ⊢
      simp at hi1
      simp [hl] at hi1
      simp [List.getElem_append_right, hi1]
    rw [hlast] at h
    obtain ⟨_, _, hend, _⟩ := hc
    simp only [bind_ok, gt_iff_lt, UScalar.lt_equiv, cast_u64_usize, hend] at h
    split at h
    · rename_i hgt
      simp at h; subst h
      simp [gt_iff_lt, hgt, TrapRel]
    · rename_i hle
      simp only [gt_iff_lt, hle, if_false]
      split at h
      · rename_i hgt
        simp at h; subst h
        simp [gt_iff_lt, hgt, TrapRel]
      · rename_i hle2
        simp at h; subst h
        simp only [gt_iff_lt, hle2, if_false]
        exact ⟨_, rfl, cast_usize_le target len (by omega)⟩

end VmExtract
