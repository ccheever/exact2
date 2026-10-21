/-
What the refinement proofs rewrite the extracted code with: Aeneas's
primitives (vectors, slices, scalars) as plain list and number facts.
-/
import VmExtract.Rel

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

theorem bind_eq_ok' {α β} {x : Result α} {f : α → Result β} {y : β} :
    Std.bind x f = ok y ↔ ∃ a, x = ok a ∧ f a = ok y := by
  cases x using Result.cases with
  | ret a => simp
  | vis i k => simp
  | div => simp

@[simp] theorem arr3_0 {α} (a b c : α) (hl) :
    (Array.make 3#usize [a, b, c] hl).index_usize 0#usize = ok a := by
  simp [Array.index_usize, Array.getElem?_Usize_eq]
@[simp] theorem arr3_1 {α} (a b c : α) (hl) :
    (Array.make 3#usize [a, b, c] hl).index_usize 1#usize = ok b := by
  simp [Array.index_usize, Array.getElem?_Usize_eq]
@[simp] theorem arr3_2 {α} (a b c : α) (hl) :
    (Array.make 3#usize [a, b, c] hl).index_usize 2#usize = ok c := by
  simp [Array.index_usize, Array.getElem?_Usize_eq]

@[simp] theorem branch_ok {T E} (v : T) :
    core.result.Result.Insts.CoreOpsTry.branch (E := E) (.Ok v) = ok (.Continue v) := rfl
@[simp] theorem branch_err {T E} (e : E) :
    core.result.Result.Insts.CoreOpsTry.branch (T := T) (.Err e) = ok (.Break (.Err e)) := rfl
@[simp] theorem residual_err {T E} (e : E) :
    core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual T
      (core.convert.FromSame E) (.Err e) = ok (.Err e) := by
  simp [core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual]

@[simp] theorem except_bind_error {ε α β} (e : ε) (f : α → Except ε β) :
    (Except.error e >>= f) = Except.error e := rfl
@[simp] theorem except_bind_ok {ε α β} (a : α) (f : α → Except ε β) :
    (Except.ok a >>= f : Except ε β) = f a := rfl

@[simp] theorem usize_max_pos : 0 < Usize.max := by scalar_tac
theorem u32_le_usize : U32.max ≤ Usize.max := by scalar_tac

theorem push_eq {α} (v : alloc.vec.Vec α) (x : α) :
    v.push x = if h : v.val.length < Usize.max then
      ok (alloc.vec.Vec.from (v.val ++ [x]) (by simp; omega)) else fail .maximumSizeExceeded := by
  unfold alloc.vec.Vec.push
  have hu : U32.max ≤ Usize.max := by scalar_tac
  by_cases hc : v.val.length < Usize.max
  · rw [dif_pos hc, dif_pos (by simp; omega)]
    simp
  · rw [dif_neg hc, dif_neg (by simp; omega)]

@[simp] theorem deref_val {α} (v : alloc.vec.Vec α) : (alloc.vec.Vec.deref v).val = v.val := by
  simp [alloc.vec.Vec.deref]

theorem tryMk_ok {ty x} {y : UScalar ty} (h : UScalar.tryMk ty x = ok y) : y.val = x := by
  have := UScalar.tryMk_eq ty x
  rw [h] at this
  simp at this
  exact this.1

theorem operand_ok {bits x} {a : U64} (h : operand bits x = ok a) : a.val = x ∧ x < 2 ^ bits := by
  simp only [operand] at h
  split at h
  · exact ⟨tryMk_ok h, by assumption⟩
  · simp at h

@[simp] theorem zero_eq : zero = ok 0#u64 := rfl

theorem mkIns_ok {pc op a b c num ins} (h : mkIns pc op a b c num = ok ins) :
    ∃ x y z hl, a = ok x ∧ b = ok y ∧ c = ok z ∧
      ins = { pc, op, args := Array.make 3#usize [x, y, z] hl, number := num } := by
  simp only [mkIns, bind_eq_ok'] at h
  obtain ⟨x, hx, y, hy, z, hz, h⟩ := h
  simp only [ok.injEq] at h
  exact ⟨x, y, z, _, hx, hy, hz, h.symm⟩

theorem usize_add_ok' {x y z : Usize} (h : x + y = ok z) : z.val = x.val + y.val := by
  have := UScalar.add_equiv x y
  rw [h] at this
  simp at this
  exact this.2.1

theorem usize_sub_ok {x y z : Usize} (h : x - y = ok z) : z.val = x.val - y.val ∧ y.val ≤ x.val := by
  have := UScalar.sub_equiv x y
  rw [h] at this
  simp at this
  omega

theorem trapRel_tm (pc : Usize) (opc : Op) (h1 : opc ≠ .Call) (h2 : opc ≠ .NativeProps) :
    TrapRel (.TypeMismatch pc opc) (.typeMismatch (opName opc)) := by
  cases opc <;> simp_all [TrapRel]

theorem pow32_le : 2 ^ 32 ≤ 2 ^ UScalarTy.Usize.numBits := by
  have : 32 ≤ UScalarTy.Usize.numBits := by
    cases System.Platform.numBits_eq <;> simp [UScalarTy.numBits, *]
  exact Nat.pow_le_pow_right (by decide) this

theorem cast_usize_val (x : U64) (h : x.val < 2 ^ 32) : (UScalar.cast .Usize x).val = x.val :=
  UScalar.cast_val_mod_pow_of_inBounds_eq _ _ (Nat.lt_of_lt_of_le h pow32_le)

theorem vec_index_eq {α} (v : alloc.vec.Vec α) (i : Usize) (hi : i.val < v.val.length) :
    alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice α) v i = ok v.val[i.val] := by
  simp [alloc.vec.Vec.index, Slice.index_usize]
  rw [show ((v.slice : Slice α) : List α)[i.val]? = v.val[i.val]? from rfl,
    List.getElem?_eq_getElem hi]

theorem vec_index_mut_eq {α} (v : alloc.vec.Vec α) (i : Usize) (hi : i.val < v.val.length) :
    ∃ back, alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice α) v i =
      ok (v.val[i.val], back) ∧ ∀ x, (back x).val = v.val.set i.val x := by
  refine ⟨fun o' => { slice := Slice.set v.slice i o' }, ?_, ?_⟩
  · simp only [alloc.vec.Vec.index_mut, core.slice.index.SliceIndexUsizeSlice,
      core.slice.index.Usize.index_mut, Slice.index_mut_usize, Slice.index_usize]
    rw [show v.slice[i]? = v.val[i.val]? from rfl, List.getElem?_eq_getElem hi]
    simp only [← bind_pure_comp, bind_tc_ok, pure_tc_eq, Function.comp]
  · intro x
    simp [alloc.vec.Vec.val, Slice.set_val_eq]

theorem usize_sub_eq (x y : Usize) (h : y.val ≤ x.val) :
    ∃ z : Usize, z.val = x.val - y.val ∧ x - y = ok z := by
  have := UScalar.sub_equiv x y
  cases hs : x - y using Result.cases with
  | ret z => rw [hs] at this; simp at this; exact ⟨z, by omega, rfl⟩
  | vis e k => rw [hs] at this; simp at this; omega
  | div => rw [hs] at this; simp at this

/-! ## The machine's helpers -/

theorem pop_rev {v : alloc.vec.Vec Value} {S : List Value} (hv : v.val = S.reverse) (pc : Usize) :
    ∃ v' : alloc.vec.Vec Value, v'.val = S.tail.reverse ∧
      machine.pop v pc = ok ((match S.head? with
        | Option.none => .Err (.StackUnderflow pc) | Option.some x => .Ok x), v') := by
  refine ⟨alloc.vec.Vec.from v.val.dropLast (by have := v.property; simp; omega), ?_, ?_⟩
  · simp [hv, List.dropLast_reverse]
  · have : v.val.getLast? = S.head? := by rw [hv]; simp
    simp only [machine.pop, alloc.vec.Vec.pop, bind_ok, this]
    cases S.head? <;> rfl

theorem num2_rev {v : alloc.vec.Vec Value} {S : List Value} (hv : v.val = S.reverse)
    (op : machine.Num) (opcode : Op) (pc : Usize) {r v'}
    (h : machine.num2 valInst v op opcode pc = ok (r, v')) :
    match S with
    | b :: a :: s =>
      (match a, b with
       | .num x, .num y => r = .Ok () ∧ v'.val = (numOp op x y :: s).reverse
       | _, _ => r = .Err (.TypeMismatch pc opcode))
    | _ => r = .Err (.StackUnderflow pc) := by
  obtain ⟨v1, hv1, hp1⟩ := pop_rev hv pc
  simp only [machine.num2, hp1, bind_ok, uncurry_apply_pair] at h
  rcases S with _ | ⟨b, S⟩
  · simp at h; exact h.1.symm
  · obtain ⟨v2, hv2, hp2⟩ := pop_rev (S := S) (by simpa using hv1) pc
    simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair, hp2] at h
    rcases S with _ | ⟨a, s⟩
    · simp at h; exact h.1.symm
    · simp only [List.head?_cons, branch_ok, bind_ok, uncurry_apply_pair, kindOf_bind] at h
      simp only [List.tail_cons] at hv2
      cases a <;> cases b <;> simp [KindOf, push_eq] at h <;>
        first
        | exact h.1.symm
        | (obtain ⟨_, h⟩ := h; exact h.1.symm)
        | (split at h
           · simp at h
             obtain ⟨rfl, rfl⟩ := h
             simp [hv2]
           · simp at h)

end VmExtract
