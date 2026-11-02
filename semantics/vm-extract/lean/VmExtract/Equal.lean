/-
`machine::equal`, as extracted, is the semantics' `Value.equal`: whenever
the extracted function returns, it returns what `Value.equal` answers.
-/
import VmExtract.Model

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

theorem bind_eq_ok {α β} {x : Result α} {f : α → Result β} {y : β} :
    Std.bind x f = ok y ↔ ∃ a, x = ok a ∧ f a = ok y := by
  cases x using Result.cases with
  | ret a => simp
  | vis i k => simp
  | div => simp

theorem bind_tc_eq_ok {α β} {x : Result α} {f : α → Result β} {y : β} :
    (x >>= f) = ok y ↔ ∃ a, x = ok a ∧ f a = ok y := bind_eq_ok

/-- What `kindOf v` returns, when it returns. -/
def KindOf : Value → machine.Kind Value → Prop
  | .num f, k => k = .Number f
  | .bool b, k => k = .Bool b
  | .str _, k => k = .Str
  | .unit, k => k = .Unit
  | .none, k => k = .None
  | .some v, k => k = .Some v
  | .list xs, k => ∃ h, k = .List (Slice.from xs h)
  | .record _ fs, k => ∃ h, k = .Record (Slice.from fs h)

theorem kindOf_eq {v k} (h : kindOf v = ok k) : KindOf v k := by
  cases v <;> simp only [kindOf, ok.injEq] at h <;> simp only [KindOf] <;>
    first
    | exact h.symm
    | (split at h
       · simp only [ok.injEq] at h; exact ⟨_, h.symm⟩
       · simp at h)

@[simp] theorem valInst_kind : valInst.kind = kindOf := rfl
@[simp] theorem valInst_num_eq (x y : F64) : valInst.num_eq x y = ok (x == y) := rfl
@[simp] theorem valInst_str_eq (a b : Value) : valInst.str_eq a b = ok (strEq a b) := rfl
@[simp] theorem valInst_clone (v : Value) : valInst.corecloneCloneInst.clone v = ok v := rfl

theorem usize_add_ok {x y z : Usize} (h : x + y = ok z) : z.val = x.val + y.val := by
  have := UScalar.add_equiv x y
  rw [h] at this
  simp at this
  exact this.2.1

theorem slice_index_ok {xs : List α} {hx} {i : Usize} {t : α}
    (h : (Slice.from xs hx).index_usize i = ok t) : ∃ hi : i.val < xs.length, t = xs[i.val] := by
  simp only [Slice.index_usize] at h
  split at h
  · simp at h
  · rename_i x hx'
    simp only [ok.injEq] at h
    subst h
    simp at hx'
    obtain ⟨hi, e⟩ := List.getElem?_eq_some_iff.mp hx'
    exact ⟨hi, e.symm⟩

/-- The loop over two equally long lists, given `equal` correct on the items. -/
theorem items_loop_ok (xs ys : List Value) (hx hy) (heq : xs.length = ys.length)
    (ih : ∀ a ∈ xs, ∀ b r, machine.equal valInst a b = ok r → r = Value.equal a b) :
    ∀ (k : Nat) (i : Usize) (r : Option Bool), xs.length - i.val = k →
      machine.equal_items_loop valInst (Slice.from xs hx) (Slice.from ys hy) i = ok r →
      r = Value.equalItems (xs.drop i.val) (ys.drop i.val) := by
  intro k
  induction k with
  | zero =>
    intro i r hk h
    rw [machine.equal_items_loop] at h
    have hi : ¬ (i < (Slice.from xs hx).len) := by simp; omega
    rw [if_neg hi] at h
    simp only [ok.injEq] at h
    subst h
    rw [List.drop_eq_nil_of_le (by omega), List.drop_eq_nil_of_le (by omega)]
    simp [Value.equalItems]
  | succ k ihk =>
    intro i r hk h
    rw [machine.equal_items_loop] at h
    have hi : i < (Slice.from xs hx).len := by simp; omega
    rw [if_pos hi] at h
    have hix : i.val < xs.length := by omega
    have hiy : i.val < ys.length := by omega
    simp only [bind_eq_ok] at h
    obtain ⟨t, ht, t1, ht1, o, ho, h⟩ := h
    obtain ⟨_, rfl⟩ := slice_index_ok ht
    obtain ⟨_, rfl⟩ := slice_index_ok ht1
    have hoe := ih _ (List.getElem_mem hix) _ o ho
    rw [List.drop_eq_getElem_cons hix, List.drop_eq_getElem_cons hiy]
    simp only [Value.equalItems]
    rw [← hoe]
    rcases o with _ | _ | _
    · simp only [ok.injEq] at h; exact h.symm
    · simp only [Bool.false_eq_true, if_false, ok.injEq] at h; exact h.symm
    · simp only [if_true, bind_eq_ok] at h
      obtain ⟨i2, hi2, h⟩ := h
      have hv := usize_add_ok hi2
      have := ihk i2 r (by simp at hv; omega) h
      rw [this, hv]
      simp

theorem equal_items_ok (xs ys : List Value) (hx hy)
    (ih : ∀ a ∈ xs, ∀ b r, machine.equal valInst a b = ok r → r = Value.equal a b) (r : Option Bool)
    (h : machine.equal_items valInst (Slice.from xs hx) (Slice.from ys hy) = ok r) :
    r = if xs.length = ys.length then Value.equalItems xs ys else some false := by
  rw [machine.equal_items] at h
  simp at h
  split at h
  · rename_i heq
    rw [if_pos heq]
    simpa using items_loop_ok xs ys hx hy heq ih _ 0#usize r rfl h
  · rename_i hne
    rw [if_neg hne]
    simp only [ok.injEq] at h
    exact h.symm

theorem equal_ok_lt : ∀ (n : Nat) (a : Value), sizeOf a < n →
    ∀ (b : Value) (r : Option Bool), machine.equal valInst a b = ok r → r = Value.equal a b := by
  intro n
  induction n with
  | zero => intro a ha; omega
  | succ n ihn =>
  intro a ha b r h
  rw [machine.equal] at h
  simp only [valInst_kind, bind_eq_ok] at h
  obtain ⟨ka, hka, kb, hkb, h⟩ := h
  have ha' := kindOf_eq hka
  have hb := kindOf_eq hkb
  cases a <;> cases b <;> simp only [KindOf] at ha' hb <;>
    (repeat (first | (obtain ⟨_, rfl⟩ := ha') | (obtain ⟨_, rfl⟩ := hb))) <;>
    simp [Value.equal] at h ⊢ <;> (try subst h) <;> (try simp [strEq, Bool.beq_eq_decide_eq])
  case some.some x y => exact ihn x (by simp at ha; omega) y r h
  case list.list xs ys _ _ =>
    refine equal_items_ok xs ys _ _ (fun a hm b r h => ihn a ?_ b r h) r h
    have := List.sizeOf_lt_of_mem hm
    simp at ha
    omega
  case record.record xs _ ys _ _ =>
    refine equal_items_ok xs ys _ _ (fun a hm b r h => ihn a ?_ b r h) r h
    have := List.sizeOf_lt_of_mem hm
    simp at ha
    omega

/-- Whenever the extracted `machine::equal` returns, it returns `Value.equal`. -/
theorem equal_ok (a b : Value) (r : Option Bool) (h : machine.equal valInst a b = ok r) :
    r = Value.equal a b :=
  equal_ok_lt _ a (Nat.lt_succ_self _) b r h

end VmExtract
