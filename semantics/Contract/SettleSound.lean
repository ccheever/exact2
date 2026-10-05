/-
Settlement preserves typing: `settle` returns derives and resources of
their declared types, or refuses — never a type error or an unbound name.
-/
import Contract.EnvSound

namespace Contract

/-- An oracle answers or refuses. -/
theorem ask_legit (o : Oracle) (src : String) (args : List Value) (r : String) :
    GoodW Strict (fun _ => True) (o.ask src args r) := by
  unfold Oracle.ask
  split
  · trivial
  · split
    · trivial
    · exact ⟨trivial, by simp⟩

/-- What settlement's evaluations need: the derives and resources are
typed in a scope `G`, and an environment of the slots and any well-typed
settled values satisfies `EnvOK` for `G`. -/
structure SettleHyps (p : Program) (G : Scope) (slots : List (String × Value)) (now : F64) : Prop where
  prog : ProgOK p
  derives : ∀ d ∈ p.derives, ∃ t, HasTy p G [] d.body t
  resources : ∀ r ∈ p.resources, ∃ ts, ListTy p G [] r.args ts
  env : ∀ st : Settled, SettledOK p st →
    EnvOK p G { prog := p, slots, derives := st.derives, resources := st.resources, now }

theorem SettledOK.addDerive {p : Program} {st : Settled} {d : DeriveDecl} {v : Value} (h : SettledOK p st)
    (hd : d ∈ p.derives) (hv : conforms p v d.ty = true) :
    SettledOK p { st with derives := st.derives ++ [(d.name, v)] } := by
  refine ⟨fun x w hx => ?_, h.2⟩
  rcases List.mem_append.mp hx with hx | hx
  · exact h.1 x w hx
  · simp only [List.mem_singleton, Prod.mk.injEq] at hx
    obtain ⟨rfl, rfl⟩ := hx
    exact ⟨d, hd, rfl, hv⟩

theorem SettledOK.addResource {p : Program} {st : Settled} {r : ResourceDecl} {v : Value} {a}
    (h : SettledOK p st) (hr : r ∈ p.resources) (hv : conforms p v r.ty = true) :
    SettledOK p { st with resources := st.resources ++ [(r.name, v)], args := a } := by
  refine ⟨h.1, fun x w hx => ?_⟩
  rcases List.mem_append.mp hx with hx | hx
  · exact h.2 x w hx
  · simp only [List.mem_singleton, Prod.mk.injEq] at hx
    obtain ⟨rfl, rfl⟩ := hx
    exact ⟨r, hr, rfl, hv⟩

theorem settle_pass_good {p : Program} {o : Oracle} {slots now prev force} {G : Scope} {H : Prop}
    (hN : distinct (p.resources.map (·.name)) = true)
    (hH : H → SettleHyps p G slots now) (hprev : SettledOK p prev) :
    ∀ n st, SettledOK p st →
      GoodW (fun e => (H → Legit e) ∧ e ≠ .pending) (SettledOK p) (settle.pass p o slots now prev force n st)
  | 0, _, _ => by simp [settle.pass, GoodW, Legit]
  | n + 1, st, hst => by
    rw [settle.pass]
    refine GoodW.bind (forIn_goodW (fun s : Settled × Bool × Bool => SettledOK p s.1) _ _ _ hst ?_)
      fun s hs => ?_
    · intro d hd s hs
      dsimp only
      split
      · exact hs
      split
      · next v hv =>
        split
        · simp [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW, Legit]
        · next hc => exact hs.addDerive hd (by simpa using hc)
      · exact hs
      · next e hne he =>
        simp only [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW]
        refine ⟨fun hHy => ?_, hne⟩
        have HH := hH hHy
        obtain ⟨t, ht⟩ := HH.derives d hd
        have := eval_sound_ty (n := fuel) HH.prog (HH.env s.1 hs) LocalsOK.nil ht
        rw [he] at this
        exact this
    refine GoodW.bind (forIn_goodW (fun s : Settled × Bool × Bool => SettledOK p s.1) _ _ _ hs ?_)
      fun s hs => ?_
    · intro r hr s hs
      dsimp only
      split
      · exact hs
      split
      · next args hargs =>
        split
        · next k hk =>
          obtain ⟨v, held⟩ := k
          have hv : lookup r.name prev.resources = .some v := by
            split at hk
            · split at hk
              · cases hk; assumption
              · cases hk
            · cases hk
          obtain ⟨r', hr', hn, hc⟩ := hprev.2 _ _ (lookup_mem hv)
          have := distinct_eq hN hr' hr hn; subst this
          exact hs.addResource hr' hc
        · refine GoodW.bind ((ask_legit o r.source args r.name).mono
            (fun e he => ⟨fun _ => he.1, he.2⟩) (fun _ h => h)) fun v _ => ?_
          split
          · simp [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW, Legit]
          · next hc => exact hs.addResource hr (by simpa using hc)
      · exact hs
      · next e hne he =>
        simp only [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW]
        refine ⟨fun hHy => ?_, hne⟩
        have HH := hH hHy
        obtain ⟨ts, ht⟩ := HH.resources r hr
        have := evalList_sound_ty (n := fuel) HH.prog (HH.env s.1 hs) LocalsOK.nil ht
        rw [he] at this
        exact this
    dsimp only
    split
    · exact hs
    split
    · simp [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW, Legit]
    · exact settle_pass_good hN hH hprev n _ hs

/-- **Settlement preserves typing.** From well-typed previous values,
`settle` returns derives and resources that are declared ones holding
values of their declared types (`SettledOK`, which needs no hypothesis:
the runtime checks every value it settles); and when the slots are well
typed for the scope the derives and resources are typed in (`SettleHyps`),
it fails only legitimately: never a type error, never an unbound name. -/
theorem settle_good {p : Program} {o : Oracle} {slots now prev force} {G : Scope} {H : Prop}
    (hN : distinct (p.resources.map (·.name)) = true)
    (hH : H → SettleHyps p G slots now) (hprev : SettledOK p prev) :
    GoodW (fun e => (H → Legit e) ∧ e ≠ .pending) (SettledOK p) (settle p o slots now prev force) :=
  settle_pass_good hN hH hprev _ _ SettledOK.empty

theorem settle_settledOK {p : Program} {o : Oracle} {slots now prev force st}
    (hN : distinct (p.resources.map (·.name)) = true) (hprev : SettledOK p prev)
    (h : settle p o slots now prev force = .ok st) : SettledOK p st := by
  have := settle_good (o := o) (slots := slots) (now := now) (force := force) (G := []) (H := False) hN
    (fun h => h.elim) hprev
  rw [h] at this; exact this

theorem settle_legit {p : Program} {o : Oracle} {slots now prev force} {G : Scope}
    (hN : distinct (p.resources.map (·.name)) = true) (hH : SettleHyps p G slots now) (hprev : SettledOK p prev) :
    GoodR (SettledOK p) (settle p o slots now prev force) :=
  GoodR.of_goodW ((settle_good (H := True) hN (fun _ => hH) hprev).mono (fun _ h => h.1 trivial) (fun _ h => h))

/-- Settlement fails only legitimately and never `pending` (it waits on
those). -/
theorem settle_strict {p : Program} {o : Oracle} {slots now prev force} {G : Scope}
    (hN : distinct (p.resources.map (·.name)) = true) (hH : SettleHyps p G slots now) (hprev : SettledOK p prev) :
    GoodW Strict (SettledOK p) (settle p o slots now prev force) :=
  (settle_good (H := True) hN (fun _ => hH) hprev).mono (fun _ h => ⟨h.1 trivial, h.2⟩) (fun _ h => h)

end Contract
