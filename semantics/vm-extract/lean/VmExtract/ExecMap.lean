/-
`Map` and `Filter`: a callback starts (the end of its body is `BodyEnd.lean`).
-/
import VmExtract.ExecData

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option maxHeartbeats 8000000

theorem item_ok {list : Value} {xs : List Value} (hl : list = .list xs) (next : Usize)
    (hn : next.val < xs.length) (hlen : xs.length ≤ Usize.max) (pc : Usize) :
    machine.item valInst list next pc = ok (.Ok xs[next.val]) := by
  subst hl
  simp only [machine.item, valInst_kind, kindOf, hlen, _root_.dite_true, bind_ok,
    UScalar.lt_equiv, Slice.len_val, Slice.length, Slice.from_val]
  rw [if_pos hn, slice_index_eq (Slice.from xs hlen) next (by simpa using hn)]
  simp

theorem begin_ok (h : LHost) (c : machine.Callback Value Unit) (locals : alloc.vec.Vec Value)
    {xs : List Value} (hl : c.list = .list xs) (hn : c.next.val < xs.length)
    (hlen : xs.length ≤ Usize.max) {r h' loc'}
    (hb : machine.begin valInst hostInst h c locals = ok (r, h', loc')) :
    r = .Ok () ∧ h' = h ∧
      loc'.val = locals.val ++ [xs[c.next.val], .num (Contract.F64.ofNat c.next.val)] := by
  simp only [machine.begin, hostInst, bind_ok, uncurry_apply_pair, branch_ok,
    item_ok hl c.next hn hlen, valInst_index, push_eq] at hb
  split at hb
  · simp only [bind_ok] at hb
    split at hb
    · simp at hb
      obtain ⟨rfl, rfl, rfl⟩ := hb
      simp
    · simp at hb
  · simp at hb

theorem take_default (v : alloc.vec.Vec Value) :
    core.mem.take (alloc.vec.Vec.Insts.CoreDefaultDefault Value) v =
      ok (v, alloc.vec.Vec.new Value) := by
  simp [core.mem.take, alloc.vec.Vec.Insts.CoreDefaultDefault,
    alloc.vec.Vec.Insts.CoreDefaultDefault.default]

set_option hygiene false in
/-- The `Map`/`Filter` arm, after the opcode's flag: the model starts the
same callback, jumps past an empty list, or refuses the same way. -/
macro "vm_callback" : tactic => `(tactic| (
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  rw [hpos] at hv
  have hnv : (UScalar.cast .U64 next).val = pc + 1 := by rw [cast_u64_usize, hn]
  simp only [lift, bind_ok, UScalar.lt_equiv, hnv, hv, hpos] at hx
  simp only [show ¬ (pc + 1 + off < pc + 1) by omega, if_false] at hx
  simp only [hostInst] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨len, hlen, hx⟩ := hx
  have hlv := codeLen_ok hlen
  simp only [bind_ok, gt_iff_lt, UScalar.lt_equiv, cast_u64_usize, hlv, hc, hv] at hx
  simp only [Contract.Vm.exec]
  by_cases hbig : code.length < pc + 1 + off
  · simp only [hbig, if_true, gt_iff_lt] at hx ⊢
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  simp only [hbig, if_false, gt_iff_lt] at hx ⊢
  have hend : (UScalar.cast .Usize x).val = pc + 1 + off := by
    rw [cast_usize_le x len (by rw [hv, hlv, hc]; omega), hv]
  vm_pop1
  simp only [Contract.Vm.pop1, except_bind_ok]
  simp only [valInst_kind, kindOf_bind] at hx
  cases a
  case list xs =>
    simp only [KindOf] at hx
    obtain ⟨_, ⟨hxl, rfl⟩, hx⟩ := hx
    rcases xs with _ | ⟨x0, rest⟩
    · have h0 : Slice.len (Slice.from ([] : List Value) hxl) = 0#usize := by
        apply UScalar.val_eq_imp; simp
      simp only [h0, if_true] at hx
      rw [bind_eq_ok'] at hx
      obtain ⟨r0, hj, hx⟩ := hx
      have hjs := jump_to_spec (cs := alloc.vec.Vec.deref m.callbacks) (by simpa using hcb) x m.pos hj
      rw [hlv, hc, hv] at hjs
      cases hJ : Contract.Vm.jumpTo code.length cbs (pc + 1 + off) <;> rw [hJ] at hjs <;>
        simp only [except_bind_ok, except_bind_error]
      · obtain ⟨e', rfl, hrel⟩ := hjs
        simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        exact ⟨_, rfl, hrel⟩
      · obtain ⟨u, rfl, hu⟩ := hjs
        simp only [branch_ok, bind_ok, push_eq] at hx
        split at hx
        · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
          simp_all [Out, Rel]
        · simp at hx
    · have hne : ¬ (Slice.len (Slice.from (x0 :: rest) hxl) = 0#usize) := by
        intro h0; have := congrArg UScalar.val h0; simp at this
      simp only [hne, if_false, take_default, bind_ok, uncurry_apply_pair,
        alloc.vec.Vec.with_capacity] at hx
      rw [bind_eq_ok'] at hx
      obtain ⟨⟨rb, hb', lb⟩, hbg, hx⟩ := hx
      obtain ⟨rfl, rfl, hlb⟩ := begin_ok _ _ _ rfl (by simp) (by simpa using hxl) hbg
      simp only [branch_ok, bind_ok, push_eq] at hx
      split at hx
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        simp only [Out, Rel, true_and]
        refine ⟨hc, he, hfx, hn, by simp, ?_, ?_⟩
        · simp [hlb, hloc]
        · simp only [alloc.vec.Vec.from_val, List.reverse_append, List.reverse_cons,
            List.reverse_nil, List.nil_append, List.cons_append]
          refine List.Forall₂.cons ?_ hcb
          simp [CbRel, hn, hend, hloc, hv1]
      · simp at hx
  all_goals (simp [KindOf] at hx; vm_close)))

theorem exec_map (off : Nat) : ExecOk (.map off) := by
  vm_intro
  obtain ⟨hv, hlt⟩ := operand_ok hx0
  rw [hpos] at hv
  have hnv : (UScalar.cast .U64 next).val = pc + 1 := by rw [cast_u64_usize, hn]
  simp only [lift, bind_ok, UScalar.lt_equiv, hnv, hv, hpos] at hx
  simp only [show ¬ (pc + 1 + off < pc + 1) by omega, if_false] at hx
  simp only [hostInst] at hx
  rw [bind_eq_ok'] at hx
  obtain ⟨len, hlen, hx⟩ := hx
  have hlv := codeLen_ok hlen
  simp only [bind_ok, gt_iff_lt, UScalar.lt_equiv, cast_u64_usize, hlv, hc, hv] at hx
  simp only [Contract.Vm.exec]
  by_cases hbig : code.length < pc + 1 + off
  · simp only [hbig, if_true, gt_iff_lt] at hx ⊢
    simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  simp only [hbig, if_false, gt_iff_lt] at hx ⊢
  have hend : (UScalar.cast .Usize x).val = pc + 1 + off := by
    rw [cast_usize_le x len (by rw [hv, hlv, hc]; omega), hv]
  vm_pop1
  simp only [Contract.Vm.pop1, except_bind_ok]
  simp only [valInst_kind, kindOf_bind] at hx
  cases a
  case list xs =>
    simp only [KindOf] at hx
    obtain ⟨_, ⟨hxl, rfl⟩, hx⟩ := hx
    rcases xs with _ | ⟨x0, rest⟩
    · have h0 : Slice.len (Slice.from ([] : List Value) hxl) = 0#usize := by
        apply UScalar.val_eq_imp; simp
      simp only [h0, if_true] at hx
      rw [bind_eq_ok'] at hx
      obtain ⟨r0, hj, hx⟩ := hx
      have hjs := jump_to_spec (cs := alloc.vec.Vec.deref m.callbacks) (by simpa using hcb) x m.pos hj
      rw [hlv, hc, hv] at hjs
      cases hJ : Contract.Vm.jumpTo code.length cbs (pc + 1 + off) <;> rw [hJ] at hjs <;>
        simp only [except_bind_ok, except_bind_error]
      · obtain ⟨e', rfl, hrel⟩ := hjs
        simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        exact ⟨_, rfl, hrel⟩
      · obtain ⟨u, rfl, hu⟩ := hjs
        simp only [branch_ok, bind_ok, push_eq] at hx
        split at hx
        · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
          simp_all [Out, Rel]
        · simp at hx
    · have hne : ¬ (Slice.len (Slice.from (x0 :: rest) hxl) = 0#usize) := by
        intro h0; have := congrArg UScalar.val h0; simp at this
      simp only [hne, if_false, take_default, bind_ok, uncurry_apply_pair,
        alloc.vec.Vec.with_capacity] at hx
      rw [bind_eq_ok'] at hx
      obtain ⟨⟨rb, hb', lb⟩, hbg, hx⟩ := hx
      obtain ⟨rfl, rfl, hlb⟩ := begin_ok _ _ _ rfl (by simp) (by simpa using hxl) hbg
      simp only [branch_ok, bind_ok, push_eq] at hx
      split at hx
      · simp at hx; obtain ⟨rfl, rfl, rfl⟩ := hx
        simp only [Out, Rel, true_and]
        refine ⟨hc, he, hfx, hn, by simp, ?_, ?_⟩
        · simp [hlb, hloc]
        · simp only [alloc.vec.Vec.from_val, List.reverse_append, List.reverse_cons,
            List.reverse_nil, List.nil_append, List.cons_append]
          refine List.Forall₂.cons ?_ hcb
          simp [CbRel, hn, hend, hloc, hv1]
      · simp at hx
  all_goals (simp [KindOf] at hx; vm_close)

theorem exec_filter (off : Nat) : ExecOk (.filter off) := by
  vm_callback

end VmExtract
