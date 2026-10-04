/-
The static types of `Contract.Lower` are sound: what a value of each type
is, and the facts the compiler's choices rest on (a joined type holds both
sides' values, a roster entry's result is of `rosterTy`, the opcode a
strict operator compiles to means what `binop` means on operands of the
type the compiler saw).
-/
import Contract.Lower
import Contract.VmFacts

namespace Contract.Lower

open Vm

mutual
/-- `v` is a value of type `t`: a record names a declared shape and holds
values of its fields' types. -/
def VTy (sh : List Shape) : Value → STy → Prop
  | _, .top => True
  | .num _, .number => True
  | .bool _, .bool => True
  | .str _, .string => True
  | .unit, .unit => True
  | .none, .option _ => True
  | .some v, .option t => VTy sh v t
  | .list xs, .list t => VTyAll sh xs t
  | .record s fs, .record s' => s = s' ∧ ∃ d, sh.find? (·.name == s) = some d ∧
      VTys sh fs (d.fields.map fun f => STy.ofTy f.ty)
  | _, _ => False
def VTyAll (sh : List Shape) : List Value → STy → Prop
  | [], _ => True
  | v :: vs, t => VTy sh v t ∧ VTyAll sh vs t
def VTys (sh : List Shape) : List Value → List STy → Prop
  | [], [] => True
  | v :: vs, t :: ts => VTy sh v t ∧ VTys sh vs ts
  | _, _ => False
end

theorem VTy.top' {sh v} : VTy sh v .top := by cases v <;> simp [VTy]

theorem VTyAll.get {sh t v} : ∀ {xs : List Value} {i : Nat}, VTyAll sh xs t → xs[i]? = some v → VTy sh v t
  | [], _, _, hi => by simp at hi
  | x :: xs, 0, h, hi => by simp [VTyAll] at h hi; subst hi; exact h.1
  | x :: xs, i + 1, h, hi => VTyAll.get (xs := xs) h.2 (by simpa using hi)

theorem map_str_ok {d : Except Err String} {v} (h : Value.str <$> d = .ok v) : ∃ s, v = .str s := by
  cases d <;> simp_all [Functor.map, Except.map]
  exact ⟨_, h.symm⟩

theorem bind_str_ok {α} {m : Except Err α} {g : α → String} {v}
    (h : (m >>= fun a => Except.ok (Value.str (g a))) = .ok v) : ∃ s, v = .str s := by
  cases m <;> simp_all [bind, Except.bind]
  exact ⟨_, h.symm⟩

theorem at_ok {xs : List Value} {c : Prop} [Decidable c] {k : Nat} {v}
    (h : (if c then Except.ok (match xs[k]? with | some v => Value.some v | none => .none)
      else .ok .none) = (Except.ok v : Result Value)) : v = .none ∨ ∃ w, xs[k]? = some w ∧ v = .some w := by
  split at h
  · split at h <;> simp at h <;> subst h
    · exact .inr ⟨_, by assumption, rfl⟩
    · exact .inl rfl
  · simp at h; exact .inl h.symm

set_option maxHeartbeats 1000000 in
theorem stdlib_ty {env : Env} {sh f vs ts v} (h : stdlib env f vs = .ok v) (hts : VTys sh vs ts) :
    VTy sh v (rosterTy f ts) := by
  unfold stdlib at h
  split at h <;> simp only [rosterTy] <;> (try simp at h) <;> (try subst h) <;> (try simp [VTy])
  case h_6 =>
    obtain ⟨s, rfl⟩ := map_str_ok h; simp [VTy]
  case h_10 _ _ xs =>
    rcases ts with _ | ⟨t0, ts⟩
    · simp [VTys] at hts
    · simp [VTys] at hts
      cases t0 <;> simp [VTy.top']
      have := hts.1
      cases xs <;> simp_all [VTy, VTyAll]
  case h_11 _ _ xs _ =>
    rcases ts with _ | ⟨t0, ts⟩
    · simp [VTys] at hts
    · simp [VTys] at hts
      cases t0 <;> simp [VTy.top']
      rcases at_ok h with rfl | ⟨w, hw, rfl⟩
      · simp [VTy]
      · simp only [VTy]
        exact VTyAll.get (by simpa [VTy] using hts.1) hw
  case h_17 =>
    split at h
    · obtain rfl := Except.ok.inj h; simp [VTy]
    · obtain ⟨s, rfl⟩ := bind_str_ok h; simp [VTy]

theorem VTy.not_bot {sh} : ∀ {v}, ¬ VTy sh v .bot := by
  intro v; cases v <;> simp [VTy]

theorem VTyAll.append {sh t} : ∀ {xs ys : List Value}, VTyAll sh xs t → VTyAll sh ys t →
    VTyAll sh (xs ++ ys) t
  | [], _, _, h => h
  | _ :: xs, _, h₁, h₂ => ⟨h₁.1, VTyAll.append (xs := xs) h₁.2 h₂⟩

theorem VTys.get {sh v t} : ∀ {vs : List Value} {ts : List STy} {i : Nat}, VTys sh vs ts →
    vs[i]? = some v → ts[i]? = some t → VTy sh v t
  | [], [], _, _, hv, _ => by simp at hv
  | [], _ :: _, _, h, _, _ => by simp [VTys] at h
  | _ :: _, [], _, h, _, _ => by simp [VTys] at h
  | _ :: _, _ :: _, 0, h, hv, ht => by
    simp at hv ht; subst hv; subst ht; exact h.1
  | _ :: vs, _ :: ts, i + 1, h, hv, ht => VTys.get (vs := vs) (ts := ts) h.2 (by simpa using hv) (by simpa using ht)

theorem VTys.length {sh} : ∀ {vs : List Value} {ts : List STy}, VTys sh vs ts → vs.length = ts.length
  | [], [], _ => rfl
  | [], _ :: _, h => by simp [VTys] at h
  | _ :: _, [], h => by simp [VTys] at h
  | _ :: vs, _ :: ts, h => by simp [VTys.length (vs := vs) (ts := ts) h.2]

theorem VTys.append {sh} : ∀ {vs ws : List Value} {ts us : List STy}, VTys sh vs ts → VTys sh ws us →
    VTys sh (vs ++ ws) (ts ++ us)
  | [], _, [], _, _, h => h
  | [], _, _ :: _, _, h, _ => by simp [VTys] at h
  | _ :: _, _, [], _, h, _ => by simp [VTys] at h
  | _ :: vs, _, _ :: ts, _, h₁, h₂ => ⟨h₁.1, VTys.append (vs := vs) (ts := ts) h₁.2 h₂⟩

theorem VTys.cons_iff {sh v vs t ts} : VTys sh (v :: vs) (t :: ts) ↔ VTy sh v t ∧ VTys sh vs ts := by
  simp [VTys]

/-! ## Subtypes and joins -/

theorem vty_sub {a : STy} : ∀ {b : STy} {sh v}, a.sub b = true → VTy sh v a → VTy sh v b := by
  induction a
  case bot => intro _ _ _ _ h; exact absurd h VTy.not_bot
  case option a ih =>
    intro b sh v hs h
    cases b
    case top => exact VTy.top'
    case option b =>
      simp [STy.sub] at hs
      cases v <;> simp [VTy] at h ⊢
      exact ih hs h
    all_goals simp [STy.sub] at hs
  case list a ih =>
    intro b sh v hs h
    cases b
    case top => exact VTy.top'
    case list b =>
      simp [STy.sub] at hs
      cases v <;> simp [VTy] at h ⊢
      rename_i xs
      induction xs with
      | nil => simp [VTyAll]
      | cons x xs ihx => simp [VTyAll] at h ⊢; exact ⟨ih hs h.1, ihx h.2⟩
    all_goals simp [STy.sub] at hs
  all_goals
    intro b sh v hs h
    cases b <;> simp [STy.sub] at hs <;> (try subst hs) <;> first | exact VTy.top' | exact h

theorem vty_refine {a d : STy} {sh v} (h : VTy sh v a) : VTy sh v (a.refine d) := by
  unfold STy.refine
  split
  · exact vty_sub (by assumption) h
  · exact h

theorem vty_join_left {a : STy} : ∀ {b : STy} {sh v}, VTy sh v a → VTy sh v (a.join b) := by
  induction a
  case option a ih =>
    intro b sh v h
    cases b
    case option b =>
      simp only [STy.join]
      cases v <;> simp [VTy] at h ⊢
      exact ih h
    all_goals (simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot)
  case list a ih =>
    intro b sh v h
    cases b
    case list b =>
      simp only [STy.join]
      cases v <;> simp [VTy] at h ⊢
      rename_i xs
      induction xs with
      | nil => simp [VTyAll]
      | cons x xs ihx => simp [VTyAll] at h ⊢; exact ⟨ih h.1, ihx h.2⟩
    all_goals (simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot)
  all_goals
    intro b sh v h
    cases b <;> simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot

theorem vty_join_right {a : STy} : ∀ {b : STy} {sh v}, VTy sh v b → VTy sh v (a.join b) := by
  induction a
  case option a ih =>
    intro b sh v h
    cases b
    case option b =>
      simp only [STy.join]
      cases v <;> simp [VTy] at h ⊢
      exact ih h
    all_goals (simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot)
  case list a ih =>
    intro b sh v h
    cases b
    case list b =>
      simp only [STy.join]
      cases v <;> simp [VTy] at h ⊢
      rename_i xs
      induction xs with
      | nil => simp [VTyAll]
      | cons x xs ihx => simp [VTyAll] at h ⊢; exact ⟨ih h.1, ihx h.2⟩
    all_goals (simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot)
  all_goals
    intro b sh v h
    cases b <;> simp [STy.join] <;> (try split) <;> (try subst_vars) <;>
      first | exact h | exact VTy.top' | exact absurd h VTy.not_bot

/-! ## Strict operators -/

/-- The opcode a strict operator compiles to answers what `binop` answers,
on a left operand of the type the compiler saw. -/
theorem binary_ok {op ta i t sh va vb} (hb : binInstr op ta = .ok (i, t)) (hva : VTy sh va ta) :
    (∀ v, binary i va vb = .ok v → binop op va vb = .ok v ∧ VTy sh v t) ∧
    (∀ v, binop op va vb = .ok v → binary i va vb = .ok v) := by
  cases op <;> simp [binInstr] at hb
  case add =>
    split at hb
    · simp at hb; obtain ⟨rfl, rfl⟩ := hb; subst_vars
      cases va <;> simp [VTy] at hva
      cases vb <;> simp [binary, binop, VTy]
    · split at hb
      · simp at hb; obtain ⟨rfl, rfl⟩ := hb; subst_vars
        cases va <;> simp [VTy] at hva
        cases vb <;> simp [binary, binop, num2, VTy]
      · cases hb
  all_goals (obtain ⟨rfl, rfl⟩ := hb)
  case eq | ne =>
    cases h : Value.equal va vb <;> simp [binary, binop, h, VTy]
  all_goals (cases va <;> cases vb <;> simp [binary, binop, num2, VTy])

end Contract.Lower
