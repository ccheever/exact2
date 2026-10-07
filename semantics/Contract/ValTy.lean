/-
Types as sets of values, and the order the checker's `?` induces on them.

`ValTy p v τ` says the value `v` has the type `τ` in program `p`. It is
`conforms` without finiteness: a number inside an evaluation may be an
infinity or NaN (`1 / 0` is a value), and only a boundary — a slot written,
a derive settled, a source's answer — refuses a non-finite one, at run time.

The checker's `?` (`Ty.unknown`) is the empty type here: `[]` is a
`list<?>` and `none` an `option<?>` because nothing is in the list or the
option, so no value has type `?`. That reading makes the checker's
compatibility (`Ty::unify`, `can_unify`) sound: a value of `list<?>` is the
empty list, which is a `list<T>` for every `T`. `Ty.le a b` is the order
(`?` below everything, `option`/`list` monotone), `Ty.unify` its join,
mirroring `Ty::unify` in contract/types/src/lib.rs.
-/
import Contract.Runtime

namespace Contract

namespace Ty

/-- `a` is at most `b`: every value of `a` is one of `b`. -/
def le : Ty → Ty → Bool
  | .unknown, _ => true
  | .number, .number => true
  | .bool, .bool => true
  | .string, .string => true
  | .unit, .unit => true
  | .option a, .option b => le a b
  | .list a, .list b => le a b
  | .record s, .record s' => s == s'
  | _, _ => false

/-- The most specific type both agree on (`Ty::unify`), or `none` when
they conflict. -/
def unify : Ty → Ty → Option Ty
  | .unknown, t => .some t
  | .number, .number => .some .number
  | .number, .unknown => .some .number
  | .bool, .bool => .some .bool
  | .bool, .unknown => .some .bool
  | .string, .string => .some .string
  | .string, .unknown => .some .string
  | .unit, .unit => .some .unit
  | .unit, .unknown => .some .unit
  | .option a, .option b => (unify a b).map .option
  | .option a, .unknown => .some (.option a)
  | .list a, .list b => (unify a b).map .list
  | .list a, .unknown => .some (.list a)
  | .record s, .record s' => if s = s' then .some (.record s) else .none
  | .record s, .unknown => .some (.record s)
  | _, _ => .none

/-- `can_unify`: the two have a join. -/
def compat (a b : Ty) : Bool := (unify a b).isSome

/-- No `?` anywhere (`Ty::is_complete`). -/
def complete : Ty → Bool
  | .unknown => false
  | .option t => complete t
  | .list t => complete t
  | _ => true

/-- What a template part, `toString` and a `text` print. -/
def displayable : Ty → Bool
  | .number | .bool | .string => true
  | _ => false

theorem le_refl : ∀ t : Ty, le t t = true
  | .unknown | .number | .bool | .string | .unit => rfl
  | .option t => le_refl t
  | .list t => le_refl t
  | .record s => by simp [le]

theorem unify_refl : ∀ t : Ty, unify t t = .some t
  | .unknown | .number | .bool | .string | .unit => rfl
  | .option t => by simp [unify, unify_refl t]
  | .list t => by simp [unify, unify_refl t]
  | .record s => by simp [unify]

theorem compat_refl (t : Ty) : compat t t = true := by simp [compat, unify_refl]

/-- The join is above both sides. -/
theorem unify_le : ∀ {a b u : Ty}, unify a b = .some u → le a u = true ∧ le b u = true
  | .unknown, b, u, h => by simp [unify] at h; subst h; exact ⟨rfl, le_refl _⟩
  | .number, b, u, h | .bool, b, u, h | .string, b, u, h | .unit, b, u, h => by
    cases b <;> simp [unify] at h <;> subst h <;> simp [le]
  | .option a, b, u, h => by
    cases b <;> simp [unify] at h
    case option b =>
      obtain ⟨w, hw, rfl⟩ := h
      simpa [le] using unify_le hw
    case unknown => subst h; exact ⟨le_refl _, rfl⟩
  | .list a, b, u, h => by
    cases b <;> simp [unify] at h
    case list b =>
      obtain ⟨w, hw, rfl⟩ := h
      simpa [le] using unify_le hw
    case unknown => subst h; exact ⟨le_refl _, rfl⟩
  | .record s, b, u, h => by
    cases b <;> simp [unify] at h
    case record s' => obtain ⟨rfl, rfl⟩ := h; simp [le]
    case unknown => subst h; simp [le]

/-- A list literal's items met in turn (`Ty::unify`, item by item, LLP
1088 §9.1): `?` for `[]`. -/
def unifyAll : List Ty → Option Ty
  | [] => .some .unknown
  | t :: ts => (unifyAll ts).bind (unify t)

theorem compat_option {a b : Ty} (h : compat (.option a) (.option b) = true) : compat a b = true := by
  simp only [compat, unify, Option.isSome_map] at h; exact h

theorem compat_list {a b : Ty} (h : compat (.list a) (.list b) = true) : compat a b = true := by
  simp only [compat, unify, Option.isSome_map] at h; exact h

end Ty

/-! ## Values of a type -/

mutual
/-- `v` is a value of `τ`. A record is of the shape it names, its fields of
the field types; no value is of `?`. -/
def ValTy (p : Program) : Value → Ty → Prop
  | .num _, .number => True
  | .bool _, .bool => True
  | .str _, .string => True
  | .unit, .unit => True
  | .none, .option _ => True
  | .some v, .option t => ValTy p v t
  | .list xs, .list t => ValTys p xs t
  | .record s vs, .record s' => s = s' ∧ ∃ sh, p.shapes.find? (·.name == s') = .some sh ∧ FieldsTy p vs sh.fields
  | _, _ => False
/-- Every item of a list is of `t`. -/
def ValTys (p : Program) : List Value → Ty → Prop
  | [], _ => True
  | v :: vs, t => ValTy p v t ∧ ValTys p vs t
/-- A record's fields, pairwise of the declared types, as many as declared. -/
def FieldsTy (p : Program) : List Value → List Field → Prop
  | [], [] => True
  | v :: vs, f :: fs => ValTy p v f.ty ∧ FieldsTy p vs fs
  | _, _ => False
end

/-- Every field of every shape is of a complete type (no `?`). -/
def ShapesComplete (p : Program) : Prop :=
  ∀ sh ∈ p.shapes, ∀ f ∈ sh.fields, f.ty.complete = true

theorem ValTy.unknown {p : Program} {v : Value} : ¬ ValTy p v .unknown := by
  cases v <;> simp [ValTy]

theorem ValTys.mem {p : Program} {t : Ty} : ∀ {xs : List Value}, ValTys p xs t → ∀ x ∈ xs, ValTy p x t
  | [], _, _, hx => by simp at hx
  | y :: ys, h, x, hx => by
    simp only [ValTys] at h
    simp only [List.mem_cons] at hx
    rcases hx with rfl | hx
    · exact h.1
    · exact ValTys.mem h.2 x hx

theorem ValTys.of_mem {p : Program} {t : Ty} : ∀ {xs : List Value}, (∀ x ∈ xs, ValTy p x t) → ValTys p xs t
  | [], _ => trivial
  | y :: ys, h => ⟨h y (by simp), ValTys.of_mem fun x hx => h x (by simp [hx])⟩

theorem ValTys.iff {p : Program} {t : Ty} {xs : List Value} : ValTys p xs t ↔ ∀ x ∈ xs, ValTy p x t :=
  ⟨fun h => h.mem, ValTys.of_mem⟩

theorem FieldsTy.length {p : Program} : ∀ {vs : List Value} {fs : List Field}, FieldsTy p vs fs → vs.length = fs.length
  | [], [], _ => rfl
  | _ :: _, _ :: _, h => by simp only [FieldsTy] at h; simp [FieldsTy.length h.2]
  | [], _ :: _, h | _ :: _, [], h => by simp [FieldsTy] at h

/-- The `i`th field of a record of a shape is of the `i`th field's type. -/
theorem FieldsTy.get {p : Program} : ∀ {vs : List Value} {fs : List Field} {i : Nat} {f : Field},
    FieldsTy p vs fs → fs[i]? = .some f → ∃ v, vs[i]? = .some v ∧ ValTy p v f.ty
  | [], [], _, _, _, h => by simp at h
  | v :: vs, g :: fs, 0, f, hv, h => by
    simp only [FieldsTy] at hv; simp at h; subst h; exact ⟨v, rfl, hv.1⟩
  | _ :: vs, _ :: fs, i + 1, f, hv, h => by
    simp only [FieldsTy] at hv; simp at h; simpa using FieldsTy.get hv.2 h
  | [], _ :: _, _, _, h, _ | _ :: _, [], _, _, h, _ => by simp [FieldsTy] at h

theorem FieldsTy.drop {p : Program} : ∀ {vs : List Value} {fs : List Field} (i : Nat),
    FieldsTy p vs fs → FieldsTy p (vs.drop i) (fs.drop i)
  | _, _, 0, h => by simpa using h
  | [], [], _ + 1, _ => trivial
  | _ :: vs, _ :: fs, i + 1, h => by simp only [FieldsTy] at h; simpa using FieldsTy.drop i h.2
  | [], _ :: _, _ + 1, h | _ :: _, [], _ + 1, h => by simp [FieldsTy] at h

/-- A value of `a` is one of every type above `a`. -/
theorem ValTy.mono {p : Program} : ∀ {v : Value} {a b : Ty}, ValTy p v a → Ty.le a b = true → ValTy p v b
  | .num _, a, b, h, hl | .bool _, a, b, h, hl | .str _, a, b, h, hl | .unit, a, b, h, hl
  | .none, a, b, h, hl => by
    cases a <;> simp [ValTy] at h <;> cases b <;> simp_all [Ty.le, ValTy]
  | .some v, a, b, h, hl => by
    cases a <;> simp [ValTy] at h
    case option a => cases b <;> simp [Ty.le] at hl; simp only [ValTy]; exact ValTy.mono h hl
  | .list xs, a, b, h, hl => by
    cases a <;> simp [ValTy] at h
    case list a =>
      cases b <;> simp [Ty.le] at hl
      case list b =>
        simp only [ValTy]
        exact ValTys.of_mem fun x hx => ValTy.mono (h.mem x hx) hl
  | .record s vs, a, b, h, hl => by
    cases a <;> simp [ValTy] at h
    case record s' => cases b <;> simp [Ty.le] at hl; subst hl; exact h

/-- Unifiable types give values `==` can compare: `Value.equal` answers. -/
theorem equal_isSome {p : Program} : ∀ (v w : Value) {a b : Ty}, ValTy p v a → ValTy p w b →
    Ty.compat a b = true → (Value.equal v w).isSome = true
  | .num _, w, a, b, h1, h2, hc | .bool _, w, a, b, h1, h2, hc | .str _, w, a, b, h1, h2, hc
  | .unit, w, a, b, h1, h2, hc => by
    cases a <;> simp [ValTy] at h1 <;> cases b <;> simp [Ty.compat, Ty.unify] at hc <;>
      cases w <;> simp_all [ValTy, Value.equal]
  | .none, w, a, b, h1, h2, hc => by
    cases a <;> simp [ValTy] at h1
    cases b <;> simp [Ty.compat, Ty.unify] at hc <;> cases w <;> simp_all [ValTy, Value.equal]
  | .some v, w, a, b, h1, h2, hc => by
    cases a <;> simp [ValTy] at h1
    case option a =>
      cases b <;> simp [Ty.compat, Ty.unify] at hc
      case option b =>
        cases w <;> simp [ValTy] at h2
        case none => simp [Value.equal]
        case some w =>
          simp only [Value.equal]
          exact equal_isSome v w h1 h2 (by simpa [Ty.compat] using hc)
      case unknown => exact absurd h2 ValTy.unknown
  | .list xs, w, a, b, h1, h2, hc => by
    cases a <;> simp [ValTy] at h1
    case list a =>
      cases b <;> simp [Ty.compat, Ty.unify] at hc
      case list b =>
        cases w <;> simp [ValTy] at h2
        case list ys =>
          simp only [Value.equal]
          split
          · rfl
          · exact items_isSome xs ys h1 h2 (by simpa [Ty.compat] using hc)
      case unknown => exact absurd h2 ValTy.unknown
  | .record s vs, w, a, b, h1, h2, hc => by
    cases a <;> simp [ValTy] at h1
    case record s1 =>
      obtain ⟨rfl, sh, hsh, hf⟩ := h1
      cases b <;> simp [Ty.compat, Ty.unify] at hc
      case record s2 =>
        subst hc
        cases w <;> simp [ValTy] at h2
        case record s' ws =>
          obtain ⟨rfl, sh', hsh', hf'⟩ := h2
          rw [hsh] at hsh'; cases hsh'
          simp only [Value.equal]
          split
          · rfl
          · exact fields_isSome vs ws sh.fields hf hf'
      case unknown => exact absurd h2 ValTy.unknown
where
  items_isSome : ∀ (xs ys : List Value) {a b : Ty}, ValTys p xs a → ValTys p ys b → Ty.compat a b = true →
      (Value.equalItems xs ys).isSome = true
    | [], _, _, _, _, _, _ => by simp [Value.equalItems]
    | _ :: _, [], _, _, _, _, _ => by simp [Value.equalItems]
    | x :: xs, y :: ys, a, b, h1, h2, hc => by
      simp only [ValTys] at h1 h2
      have hx := equal_isSome x y h1.1 h2.1 hc
      simp only [Value.equalItems]
      split
      · next he => rw [he] at hx; simp at hx
      · rfl
      · exact items_isSome xs ys h1.2 h2.2 hc
  fields_isSome : ∀ (xs ys : List Value) (fs : List Field), FieldsTy p xs fs → FieldsTy p ys fs →
      (Value.equalItems xs ys).isSome = true
    | [], _, _, _, _ => by simp [Value.equalItems]
    | _ :: _, [], _, _, _ => by simp [Value.equalItems]
    | x :: xs, y :: ys, [], h1, _ => by simp [FieldsTy] at h1
    | x :: xs, y :: ys, f :: fs, h1, h2 => by
      simp only [FieldsTy] at h1 h2
      have hx := equal_isSome x y h1.1 h2.1 (Ty.compat_refl f.ty)
      simp only [Value.equalItems]
      split
      · next he => rw [he] at hx; simp at hx
      · rfl
      · exact fields_isSome xs ys fs h1.2 h2.2

/-! ## `conforms` is `ValTy` and finiteness -/

/-- What crosses a boundary has its type: a value the runtime check admits
at a complete type is a value of that type (and its numbers are finite,
which `ValTy` does not ask). -/
theorem conforms_valTy {p : Program} (hs : ShapesComplete p) : ∀ (v : Value) {t : Ty},
    conforms p v t = true → t.complete = true → ValTy p v t
  | .num _, t, h, hc | .bool _, t, h, hc | .str _, t, h, hc | .unit, t, h, hc | .none, t, h, hc => by
    cases t <;> simp_all [conforms, Ty.complete, ValTy]
  | .some v, t, h, hc => by
    cases t <;> simp [conforms, Ty.complete] at h hc
    case option t => exact conforms_valTy hs v h hc
  | .list xs, t, h, hc => by
    cases t <;> simp [conforms, Ty.complete] at h hc
    case list t => exact all xs h hc
  | .record s' vs, t, h, hc => by
    cases t <;> simp [conforms] at h
    case unknown => simp [Ty.complete] at hc
    case record s =>
      split at h
      next sh hsh =>
        simp only [Bool.and_eq_true, beq_iff_eq] at h
        obtain ⟨⟨rfl, hl⟩, hf⟩ := h
        refine ⟨rfl, sh, hsh, ?_⟩
        exact fields vs sh.fields (by simpa using hl) hf
          (hs sh (List.mem_of_find?_eq_some hsh))
      next => simp at h
where
  all : ∀ (xs : List Value) {t : Ty}, conformsAll p xs t = true → t.complete = true → ValTys p xs t
    | [], _, _, _ => trivial
    | x :: xs, t, h, hc => by
      simp only [conformsAll, Bool.and_eq_true] at h
      exact ⟨conforms_valTy hs x h.1 hc, all xs h.2 hc⟩
  fields : ∀ (vs : List Value) (fs : List Field), vs.length = fs.length →
      conformsFields p vs fs = true → (∀ f ∈ fs, f.ty.complete = true) → FieldsTy p vs fs
    | [], [], _, _, _ => trivial
    | v :: vs, f :: fs, hl, h, hc => by
      simp only [conformsFields, Bool.and_eq_true] at h
      exact ⟨conforms_valTy hs v h.1 (hc f (by simp)),
        fields vs fs (by simpa using hl) h.2 fun g hg => hc g (by simp [hg])⟩
    | [], _ :: _, hl, _, _ | _ :: _, [], hl, _, _ => by simp at hl

/-- The same of `typed`, which leaves out only finiteness. -/
theorem typed_valTy {p : Program} (hs : ShapesComplete p) : ∀ (v : Value) {t : Ty},
    typed p v t = true → t.complete = true → ValTy p v t
  | .num _, t, h, hc | .bool _, t, h, hc | .str _, t, h, hc | .unit, t, h, hc | .none, t, h, hc => by
    cases t <;> simp_all [typed, Ty.complete, ValTy]
  | .some v, t, h, hc => by
    cases t <;> simp [typed, Ty.complete] at h hc
    case option t => exact typed_valTy hs v h hc
  | .list xs, t, h, hc => by
    cases t <;> simp [typed, Ty.complete] at h hc
    case list t => exact all xs h hc
  | .record s' vs, t, h, hc => by
    cases t <;> simp [typed] at h
    case unknown => simp [Ty.complete] at hc
    case record s =>
      split at h
      next sh hsh =>
        simp only [Bool.and_eq_true, beq_iff_eq] at h
        obtain ⟨⟨rfl, hl⟩, hf⟩ := h
        refine ⟨rfl, sh, hsh, ?_⟩
        exact fields vs sh.fields (by simpa using hl) hf
          (hs sh (List.mem_of_find?_eq_some hsh))
      next => simp at h
where
  all : ∀ (xs : List Value) {t : Ty}, typedAll p xs t = true → t.complete = true → ValTys p xs t
    | [], _, _, _ => trivial
    | x :: xs, t, h, hc => by
      simp only [typedAll, Bool.and_eq_true] at h
      exact ⟨typed_valTy hs x h.1 hc, all xs h.2 hc⟩
  fields : ∀ (vs : List Value) (fs : List Field), vs.length = fs.length →
      typedFields p vs fs = true → (∀ f ∈ fs, f.ty.complete = true) → FieldsTy p vs fs
    | [], [], _, _, _ => trivial
    | v :: vs, f :: fs, hl, h, hc => by
      simp only [typedFields, Bool.and_eq_true] at h
      exact ⟨typed_valTy hs v h.1 (hc f (by simp)),
        fields vs fs (by simpa using hl) h.2 fun g hg => hc g (by simp [hg])⟩
    | [], _ :: _, hl, _, _ | _ :: _, [], hl, _, _ => by simp at hl

/-- A value of a type passes `typed`, finite or not. -/
theorem typed_of_valTy {p : Program} : ∀ (v : Value) {t : Ty}, ValTy p v t → typed p v t = true
  | .num _, t, h | .bool _, t, h | .str _, t, h | .unit, t, h | .none, t, h => by
    cases t <;> simp_all [ValTy, typed]
  | .some v, t, h => by
    cases t <;> simp [ValTy] at h
    case option t => simp only [typed]; exact typed_of_valTy v h
  | .list xs, t, h => by
    cases t <;> simp [ValTy] at h
    case list t => simp only [typed]; exact all xs h
  | .record s vs, t, h => by
    cases t <;> simp [ValTy] at h
    case record s' =>
      obtain ⟨rfl, sh, hsh, hfs⟩ := h
      simp only [typed, hsh, beq_self_eq_true, Bool.true_and, Bool.and_eq_true, beq_iff_eq]
      exact ⟨hfs.length, fields vs sh.fields hfs⟩
where
  all : ∀ (xs : List Value) {t : Ty}, ValTys p xs t → typedAll p xs t = true
    | [], _, _ => rfl
    | x :: xs, t, h => by
      simp only [typedAll, Bool.and_eq_true]
      exact ⟨typed_of_valTy x h.1, all xs h.2⟩
  fields : ∀ (vs : List Value) (fs : List Field), FieldsTy p vs fs → typedFields p vs fs = true
    | [], [], _ => rfl
    | v :: vs, f :: fs, h => by
      simp only [typedFields, Bool.and_eq_true]
      exact ⟨typed_of_valTy v h.1, fields vs fs h.2⟩
    | [], _ :: _, _ => rfl
    | _ :: _, [], h => by simp [FieldsTy] at h

/-- An argument the action's check admits at a complete type is of it. -/
theorem argOk_valTy {p : Program} (hs : ShapesComplete p) {x : String} {t : Ty} {v : Value}
    (h : argOk p (x, t) v = true) (hc : t.complete = true) : ValTy p v t := by
  simp only [argOk] at h
  by_cases hx : hiddenParam x = true
  · simp only [hx, ↓reduceIte] at h; exact typed_valTy hs _ h hc
  · simp only [hx, Bool.false_eq_true, ↓reduceIte] at h; exact conforms_valTy hs _ h hc

end Contract
