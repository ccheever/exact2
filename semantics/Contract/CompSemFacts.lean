/-
Facts about the component-level evaluator (`Contract.CompSem.ceval`):
more fuel never changes an answer (`ceval_mono`), so an expression has at
most one value in a frame (`ceval_det`).
-/
import Contract.CompSem
import Contract.Big

namespace Contract.CompSemFacts

open Contract Components CompSem

theorem ceval_binary_strict {n ce c id binds ls a b} {op : BinOp} (hand : op ≠ .and) (hor : op ≠ .or) :
    ceval (n + 1) ce (.inst c id binds) ls (.binary op a b) =
      (do let va ← ceval n ce (.inst c id binds) ls a; let vb ← ceval n ce (.inst c id binds) ls b
          binop op va vb) := by
  cases op <;> first | rfl | (exact absurd rfl hand) | (exact absurd rfl hor)

theorem cmono_aux : ∀ n,
    (∀ {m ce f ls e v}, n ≤ m → ceval n ce f ls e = .ok v → ceval m ce f ls e = .ok v) ∧
    (∀ {m ce c id binds x v}, n ≤ m → cvar n ce c id binds x = .ok v → cvar m ce c id binds x = .ok v) ∧
    (∀ {m ce f ls w x v}, n ≤ m → cpending n ce f ls w x = .ok v → cpending m ce f ls w x = .ok v) ∧
    (∀ {m ce f ls es vs}, n ≤ m → cevalList n ce f ls es = .ok vs → cevalList m ce f ls es = .ok vs) ∧
    (∀ {m ce f ls es ss}, n ≤ m → cevalDisplays n ce f ls es = .ok ss →
      cevalDisplays m ce f ls es = .ok ss) ∧
    (∀ {m ce f ls ps body xs i ys}, n ≤ m → cevalMap n ce f ls ps body xs i = .ok ys →
      cevalMap m ce f ls ps body xs i = .ok ys) ∧
    (∀ {m ce f ls ps body xs i ys}, n ≤ m → cevalFilter n ce f ls ps body xs i = .ok ys →
      cevalFilter m ce f ls ps body xs i = .ok ys) ∧
    (∀ {m ce f ls w b fs i vs}, n ≤ m → cevalFields n ce f ls w b fs i = .ok vs →
      cevalFields m ce f ls w b fs i = .ok vs)
  | 0 => by
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩ <;> intros <;>
      simp_all [ceval, cvar, cpending, cevalList, cevalDisplays, cevalMap, cevalFilter, cevalFields]
  | n + 1 => by
    obtain ⟨ihE, ihV, ihP, ihL, ihD, ihM, ihF, ihR⟩ := cmono_aux n
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · intro m ce f ls e v hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases f with
      | root => simp only [ceval] at h ⊢; exact eval_mono hm h
      | inst c id binds =>
      cases e with
      | num | str | bool | none | arrow | named => exact h
      | var x =>
        simp only [ceval] at h ⊢
        split at h
        · exact h
        · exact ihV hm h
      | some e | template e | list e =>
        simp only [ceval, Except.bind_ok_iff] at h ⊢
        obtain ⟨a, h1, h2⟩ := h
        first | exact ⟨a, ihE hm h1, h2⟩ | exact ⟨a, ihD hm h1, h2⟩ | exact ⟨a, ihL hm h1, h2⟩
      | member e f =>
        simp only [ceval, Except.bind_ok_iff] at h ⊢
        obtain ⟨a, h1, h2⟩ := h
        exact ⟨a, ihE hm h1, h2⟩
      | call name args =>
        simp only [ceval] at h ⊢
        split at h
        next fd hfd =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨vs, h1, h2⟩ := h
          exact ⟨vs, ihL hm h1, eval_mono hm h2⟩
        next hfd =>
          split at h
          next l ps body =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨a, h1, xs, h2, ys, h3, h4⟩ := h
            exact ⟨a, ihE hm h1, xs, h2, ys, ihM hm h3, h4⟩
          next l ps body =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨a, h1, xs, h2, ys, h3, h4⟩ := h
            exact ⟨a, ihE hm h1, xs, h2, ys, ihF hm h3, h4⟩
          next => exact ihP hm h
          next => exact ihP hm h
          next =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨vs, h1, h2⟩ := h
            exact ⟨vs, ihL hm h1, h2⟩
      | record shape base fields =>
        simp only [ceval] at h ⊢
        split at h
        next =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨b, h0, decl, hd, vs, h1, h2⟩ := h
          exact ⟨b, h0, decl, hd, vs, ihR hm h1, h2⟩
        next e =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨a, h1, h2⟩ := h
          refine ⟨a, ihE hm h1, ?_⟩
          split at h2
          next =>
            simp only [Except.bind_ok_iff] at h2 ⊢
            obtain ⟨b, h0, decl, hd, vs, h3, h4⟩ := h2
            exact ⟨b, h0, decl, hd, vs, ihR hm h3, h4⟩
          next => simp at h2
      | unary op e =>
        cases op <;> simp only [ceval, Except.bind_ok_iff] at h ⊢ <;> obtain ⟨a, h1, h2⟩ := h <;>
          exact ⟨a, ihE hm h1, h2⟩
      | binary op a b =>
        cases op
        case and | or =>
          simp only [ceval, Except.bind_ok_iff] at h ⊢
          obtain ⟨w, h1, h2⟩ := h
          refine ⟨w, ihE hm h1, ?_⟩
          split at h2
          next => exact h2
          next => exact ihE hm h2
          next => simp at h2
        all_goals
          rw [ceval_binary_strict (by simp) (by simp)] at h ⊢
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨va, h1, vb, h2, h3⟩ := h
          exact ⟨va, ihE hm h1, vb, ihE hm h2, h3⟩
      | ternary c a b | matchOpt c x a b =>
        simp only [ceval, Except.bind_ok_iff] at h ⊢
        obtain ⟨w, h1, h2⟩ := h
        refine ⟨w, ihE hm h1, ?_⟩
        split at h2
        next => exact ihE hm h2
        next => exact ihE hm h2
        next => simp at h2
      | letE x e body =>
        simp only [ceval, Except.bind_ok_iff] at h ⊢
        obtain ⟨w, h1, h2⟩ := h
        exact ⟨w, ihE hm h1, ihE hm h2⟩
      | typed e ty =>
        simp only [ceval] at h ⊢
        exact ihE hm h
    · intro m ce c id binds x v hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      simp only [cvar, Except.bind_ok_iff] at h ⊢
      obtain ⟨C, hC, h⟩ := h
      refine ⟨C, hC, ?_⟩
      revert h
      split
      · exact ihE hm
      · split
        · exact fun h => h
        · split
          · exact ihE hm
          · exact fun h => h
    · intro m ce f ls w x v hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases f with
      | root => simp only [cpending] at h ⊢; exact eval_mono hm h
      | inst c id binds =>
        simp only [cpending] at h ⊢
        revert h
        split
        · exact fun h => h
        · simp only [Except.bind_ok_iff]
          rintro ⟨C, hC, h⟩
          refine ⟨C, hC, ?_⟩
          revert h
          split
          · split
            · exact ihP hm
            · exact fun h => h
          · split
            · exact fun h => h
            · split
              · exact ihP hm
              · exact fun h => h
              · exact eval_mono hm
    · intro m ce f ls es vs hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases es with
      | nil => exact h
      | cons e es =>
        simp only [cevalList, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, vs', h2, h3⟩ := h
        exact ⟨v, ihE hm h1, vs', ihL hm h2, h3⟩
    · intro m ce f ls es ss hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases es with
      | nil => exact h
      | cons e es =>
        simp only [cevalDisplays, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, s, hs, ss', h2, h3⟩ := h
        exact ⟨v, ihE hm h1, s, hs, ss', ihD hm h2, h3⟩
    · intro m ce f ls ps body xs i ys hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases xs with
      | nil => exact h
      | cons x xs =>
        simp only [cevalMap, Except.bind_ok_iff] at h ⊢
        obtain ⟨y, h1, ys', h2, h3⟩ := h
        exact ⟨y, ihE hm h1, ys', ihM hm h2, h3⟩
    · intro m ce f ls ps body xs i ys hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases xs with
      | nil => exact h
      | cons x xs =>
        simp only [cevalFilter, Except.bind_ok_iff] at h ⊢
        obtain ⟨k, h1, ys', h2, h3⟩ := h
        exact ⟨k, ihE hm h1, ys', ihF hm h2, h3⟩
    · intro m ce f ls w b fs i vs hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases fs with
      | nil => exact h
      | cons fd fs =>
        simp only [cevalFields] at h ⊢
        split at h
        next e he =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨v, h1, vs', h2, h3⟩ := h
          exact ⟨v, ihE hm h1, vs', ihR hm h2, h3⟩
        next he =>
          split at h
          next bs =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨v, h1, vs', h2, h3⟩ := h
            exact ⟨v, h1, vs', ihR hm h2, h3⟩
          next => simp at h

theorem ceval_mono {n m ce f ls e v} (hm : n ≤ m) (h : ceval n ce f ls e = .ok v) :
    ceval m ce f ls e = .ok v := (cmono_aux n).1 hm h

theorem cvar_mono {n m ce c id binds x v} (hm : n ≤ m) (h : cvar n ce c id binds x = .ok v) :
    cvar m ce c id binds x = .ok v := (cmono_aux n).2.1 hm h

/-- A name has at most one value in a frame. -/
theorem ceval_det {n m ce f ls e v w} (h₁ : ceval n ce f ls e = .ok v) (h₂ : ceval m ce f ls e = .ok w) :
    v = w := by
  have := (ceval_mono (Nat.le_max_left n m) h₁).symm.trans (ceval_mono (Nat.le_max_right n m) h₂)
  simpa using this

theorem cvar_det {n m ce c id binds x v w} (h₁ : cvar n ce c id binds x = .ok v)
    (h₂ : cvar m ce c id binds x = .ok w) : v = w := by
  have := (cvar_mono (Nat.le_max_left n m) h₁).symm.trans (cvar_mono (Nat.le_max_right n m) h₂)
  simpa using this

end Contract.CompSemFacts
