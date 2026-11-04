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
    · split
      · exact ⟨trivial, by simp⟩
      · exact ⟨trivial, by simp⟩

/-- What settlement's evaluations need: the derives and resources are
typed in a scope `G`, and an environment of the slots and any well-typed
settled values satisfies `EnvOK` for `G`. -/
structure SettleHyps (p : Program) (G : Scope) (slots : List (String × Value)) (now : F64) : Prop where
  prog : ProgOK p
  derives : ∀ d ∈ p.derives, ∃ t, HasTy p G [] d.body t
  resources : ∀ r ∈ p.resources, ∃ ts, ResTy p G r ts
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

/-- A row key is a key or a refusal. -/
theorem rowKey_legit (v : Value) : GoodR (fun _ => True) (rowKey v) := by
  cases v <;> simp only [rowKey] <;> (try split) <;> simp [GoodR, Legit]

/-- A resource over a list's elements (LLP 1017.000 P4c, amended
2026-11-04), its key and arguments typed with the element bound at `te`:
each step fails only legitimately. -/
theorem settleEachGo_good {p : Program} {o : Oracle} {env : Env} {r : ResourceDecl}
    {e : ResourceEach} {prev : List Value} {forced : Bool} {G : Scope} {te : Ty} {ts : List Ty}
    (hp : ProgOK p) (henv : EnvOK p G env) (hargs : ListTy p G [(e.item, te)] r.args ts)
    (hkey : ∀ k, e.key = .some k → ∃ u, HasTy p G [(e.item, te)] k u) :
    ∀ (xs out held : List Value), ValTys p xs te →
      GoodR (fun _ => True) (settleEachGo p o env r e prev forced xs out held)
  | [], out, held, _ => by simp [settleEachGo, GoodR]
  | x :: xs, out, held, hxs => by
    simp only [ValTys] at hxs
    have hl : LocalsOK p [(e.item, te)] [(e.item, x)] := LocalsOK.nil.cons hxs.1
    rw [settleEachGo]
    have hk : GoodR (fun _ => True) (eachKey env e x) := by
      unfold eachKey
      split
      · next k hk =>
        obtain ⟨u, hu⟩ := hkey k hk
        exact (eval_sound_ty (n := fuel) hp henv hl hu).mono fun _ _ => trivial
      · trivial
    refine GoodR.bind hk fun v _ => GoodR.bind (rowKey_legit v) fun k _ => ?_
    split
    · exact settleEachGo_good hp henv hargs hkey xs _ _ hxs.2
    · refine GoodR.bind ((evalList_sound_ty (n := fuel) hp henv hl hargs).mono fun _ _ => trivial)
        fun args _ => GoodR.bind (P := fun _ => True) ?_ fun w _ => settleEachGo_good hp henv hargs hkey xs _ _ hxs.2
      unfold eachAnswer
      split
      · trivial
      · exact GoodR.of_goodW ((ask_legit o r.source args r.name).mono (fun _ h => h.1) fun _ h => h)

theorem settleEach_good {p : Program} {o : Oracle} {env : Env} {r : ResourceDecl}
    {e : ResourceEach} {prev : List Value} {forced : Bool} {G : Scope}
    (hp : ProgOK p) (henv : EnvOK p G env) (hty : ResTy p G r ts) (he : r.each = .some e) :
    GoodR (fun _ => True) (settleEach p o env r e prev forced) := by
  simp only [ResTy, he] at hty
  obtain ⟨te, hlist, hargs, hkey⟩ := hty
  unfold settleEach
  refine GoodR.bind (eval_sound_ty (n := fuel) hp henv LocalsOK.nil hlist) fun v hv => ?_
  split
  · next xs => exact settleEachGo_good hp henv hargs hkey xs [] [] hv
  · trivial

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
      · next e heach =>
        split
        · next vs held hok =>
          split
          · simp [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW, Legit]
          · next hc => exact hs.addResource hr (by simpa using hc)
        · exact hs
        · next err hne herr =>
          simp only [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW]
          refine ⟨fun hHy => ?_, hne⟩
          have HH := hH hHy
          obtain ⟨ts, ht⟩ := HH.resources r hr
          have := settleEach_good (o := o) (prev := ((prev.args.find? (·.1 == r.name)).map (·.2)).getD [])
            (forced := force.contains r.name) HH.prog (HH.env s.1 hs) ht heach
          rw [herr] at this
          exact this
      rename_i heach
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
        simp only [ResTy, heach] at ht
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
