/-
Settlement is complete: a settlement that succeeds has settled every
derive and every resource. So an environment built from it never reads a
name `pending`.
-/
import Contract.SettleSound

namespace Contract

theorem forIn_preserve {α σ ε} (R : σ → Prop) :
    ∀ (l : List α) (init : σ) (f : α → σ → Except ε (ForInStep σ)) (r : σ),
    R init → (∀ a ∈ l, ∀ s s', R s → f a s = .ok (.yield s') → R s') →
    (∀ a s s', f a s ≠ .ok (.done s')) → forIn l init f = .ok r → R r
  | [], init, f, r, h0, _, _, h => by rw [List.forIn_nil] at h; cases h; exact h0
  | a :: l, init, f, r, h0, hs, hd, h => by
    rw [List.forIn_cons] at h
    cases hf : f a init with
    | error e => rw [hf] at h; cases h
    | ok st =>
      rw [hf] at h
      cases st with
      | done b => exact absurd hf (hd _ _ _)
      | yield b =>
        exact forIn_preserve R l b f r (hs a (by simp) _ _ h0 hf)
          (fun a' ha' => hs a' (by simp [ha'])) hd h

/-- A loop that leaves `Q a` after visiting `a`, which later visits keep:
at its end, `Q a` for every `a` it visited. -/
theorem forIn_all {α σ ε} (Q : α → σ → Prop) :
    ∀ (l : List α) (init : σ) (f : α → σ → Except ε (ForInStep σ)) (r : σ),
    (∀ a ∈ l, ∀ s s', f a s = .ok (.yield s') → Q a s') →
    (∀ c, ∀ a ∈ l, ∀ s s', Q c s → f a s = .ok (.yield s') → Q c s') →
    (∀ a s s', f a s ≠ .ok (.done s')) → forIn l init f = .ok r → ∀ a ∈ l, Q a r
  | [], _, _, _, _, _, _, _ => by simp
  | a :: l, init, f, r, hq, hm, hd, h => by
    rw [List.forIn_cons] at h
    cases hf : f a init with
    | error e => rw [hf] at h; cases h
    | ok st =>
      rw [hf] at h
      cases st with
      | done b => exact absurd hf (hd _ _ _)
      | yield b =>
        intro c hc
        simp only [List.mem_cons] at hc
        rcases hc with rfl | hc
        · exact forIn_preserve (Q c) l b f r (hq c (by simp) _ _ hf)
            (fun a' ha' s s' hs h' => hm c a' (by simp [ha']) s s' hs h') hd h
        · exact forIn_all Q l b f r (fun a' ha' => hq a' (by simp [ha']))
            (fun c' a' ha' => hm c' a' (by simp [ha'])) hd h c hc

theorem lookup_isSome_append {x : String} {l l' : List (String × Value)} (h : (lookup x l).isSome = true) :
    (lookup x (l ++ l')).isSome = true := by
  rw [lookup_append]; cases hl : lookup x l <;> simp_all

theorem lookup_isSome_append_self {x : String} {l : List (String × Value)} {v : Value}
    (h : (lookup x l).isSome = false) : (lookup x (l ++ [(x, v)])).isSome = true := by
  rw [lookup_append]; cases hl : lookup x l <;> simp_all [lookup]

theorem settle_pass_complete {p : Program} {o : Oracle} {slots now prev force} :
    ∀ n st r, settle.pass p o slots now prev force n st = .ok r →
      (∀ d ∈ p.derives, (lookup d.name r.derives).isSome = true) ∧
      (∀ x ∈ p.resources, (lookup x.name r.resources).isSome = true)
  | 0, _, _, h => by simp [settle.pass] at h
  | n + 1, st, r, h => by
    rw [settle.pass] at h
    simp only [Except.bind_ok_iff] at h
    obtain ⟨s1, h1, s2, h2, h3⟩ := h
    have K : ∀ {β} {e : Err} {k : Unit → Result β} {b : β}, (throw e >>= k : Result β) ≠ .ok b := by
      intro β e k b h; cases h
    have hD := forIn_all (fun (d : DeriveDecl) (s : Settled × Bool × Bool) =>
      (lookup d.name s.1.derives).isSome = true ∨ s.2.2 = true) _ _ _ _ ?q1 ?m1 ?n1 h1
    case q1 =>
      intro d _ s s' hf
      try dsimp only at hf
      split at hf
      · cases hf; exact .inl ‹_›
      next hns =>
      split at hf
      · split at hf
        · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
        · cases hf; exact .inl (lookup_isSome_append_self (by simpa using hns))
      · cases hf; exact .inr rfl
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    case m1 =>
      intro c d _ s s' hc hf
      try dsimp only at hf
      split at hf
      · cases hf; exact hc
      split at hf
      · split at hf
        · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
        · cases hf
          rcases hc with hc | hc
          · exact .inl (lookup_isSome_append hc)
          · exact .inr hc
      · cases hf; exact .inr rfl
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    case n1 =>
      intro d s s' hf
      try dsimp only at hf
      split at hf
      · cases hf
      split at hf
      · split at hf
        · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
        · cases hf
      · cases hf
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    have hD2 := forIn_preserve (fun (s : Settled × Bool × Bool) => ∀ d ∈ p.derives,
      (lookup d.name s.1.derives).isSome = true ∨ s.2.2 = true) _ _ _ _ hD ?pr ?nd2 h2
    case pr =>
      intro x _ s s' hs hf d hd
      try dsimp only at hf
      split at hf
      · cases hf; exact hs d hd
      split at hf
      · split at hf
        · cases hf; exact hs d hd
        · simp only [bind, Except.bind] at hf
          split at hf
          · cases hf
          · split at hf
            · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
            · cases hf; exact hs d hd
      · cases hf
        rcases hs d hd with h | h
        · exact .inl h
        · exact .inr rfl
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    case nd2 =>
      intro x s s' hf
      try dsimp only at hf
      split at hf
      · cases hf
      split at hf
      · split at hf
        · cases hf
        · simp only [bind, Except.bind] at hf
          split at hf
          · cases hf
          · split at hf
            · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
            · cases hf
      · cases hf
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    have hR := forIn_all (fun (x : ResourceDecl) (s : Settled × Bool × Bool) =>
      (lookup x.name s.1.resources).isSome = true ∨ s.2.2 = true) _ _ _ _ ?q2 ?m2 ?nd3 h2
    case q2 =>
      intro x _ s s' hf
      try dsimp only at hf
      split at hf
      · cases hf; exact .inl ‹_›
      next hns =>
      split at hf
      · split at hf
        · cases hf; exact .inl (lookup_isSome_append_self (by simpa using hns))
        · simp only [bind, Except.bind] at hf
          split at hf
          · cases hf
          · split at hf
            · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
            · cases hf; exact .inl (lookup_isSome_append_self (by simpa using hns))
      · cases hf; exact .inr rfl
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    case m2 =>
      intro c x _ s s' hc hf
      try dsimp only at hf
      have keep : ∀ {v w}, s' = ({ s.1 with resources := s.1.resources ++ [(x.name, v)], args := s.1.args ++ [(x.name, w)] }, true, s.2.2) →
          (lookup c.name s'.1.resources).isSome = true ∨ s'.2.2 = true := by
        intro v w h; subst h
        rcases hc with hc | hc
        · exact .inl (lookup_isSome_append hc)
        · exact .inr hc
      split at hf
      · cases hf; exact hc
      split at hf
      · split at hf
        · cases hf; exact keep rfl
        · simp only [bind, Except.bind] at hf
          split at hf
          · cases hf
          · split at hf
            · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
            · cases hf; exact keep rfl
      · cases hf; exact .inr rfl
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    case nd3 =>
      intro x s s' hf
      try dsimp only at hf
      split at hf
      · cases hf
      split at hf
      · split at hf
        · cases hf
        · simp only [bind, Except.bind] at hf
          split at hf
          · cases hf
          · split at hf
            · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
            · cases hf
      · cases hf
      · first | exact absurd hf K | simp [throw, throwThe, MonadExceptOf.throw] at hf
    split at h3
    · next hw =>
      cases h3
      simp only [Bool.not_eq_true'] at hw
      refine ⟨fun d hd => ?_, fun x hx => ?_⟩
      · rcases hD2 d hd with h | h
        · exact h
        · simp [hw] at h
      · rcases hR x hx with h | h
        · exact h
        · simp [hw] at h
    · split at h3
      · exact absurd h3 K
      · exact settle_pass_complete n _ _ h3

/-- **Settlement is complete**: a settlement that succeeds has settled
every derive and every resource. -/
theorem settle_complete {p : Program} {o : Oracle} {slots now prev force st}
    (h : settle p o slots now prev force = .ok st) :
    (∀ d ∈ p.derives, (lookup d.name st.derives).isSome = true) ∧
      (∀ x ∈ p.resources, (lookup x.name st.resources).isSome = true) :=
  settle_pass_complete _ _ _ h

end Contract
