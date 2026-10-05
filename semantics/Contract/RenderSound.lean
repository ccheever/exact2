/-
Rendering is safe: `render` of a well-typed view (`NodesTy`), in a
well-typed environment, never fails with a type error or an unbound name,
keeps every row's slots of their types, and records only handlers that
name an existing action with curried arguments typed (in the scope the
element stands in) at most the action's leading parameters.
-/
import Contract.SettleSound

namespace Contract

/-! ## Row stores -/

/-- Every live row's slots are of their types. -/
def StoreOK (p : Program) (store : RowStore) : Prop := ∀ id s, (id, s) ∈ store → RowsOK p s

theorem StoreOK.nil {p : Program} : StoreOK p [] := fun _ _ h => nomatch h

theorem StoreOK.append {p : Program} {a b} (ha : StoreOK p a) (hb : StoreOK p b) : StoreOK p (a ++ b) :=
  fun id s h => (List.mem_append.mp h).elim (ha id s) (hb id s)

theorem StoreOK.single {p : Program} {id s} (h : RowsOK p s) : StoreOK p [(id, s)] := by
  intro i t hm; simp only [List.mem_singleton, Prod.mk.injEq] at hm; obtain ⟨-, rfl⟩ := hm; exact h

theorem StoreOK.find {p : Program} {store : RowStore} {id s} (h : StoreOK p store)
    (hf : store.find id = .some s) : RowsOK p s := by
  simp only [RowStore.find, Option.map_eq_some_iff] at hf
  obtain ⟨⟨i, t⟩, hm, rfl⟩ := hf
  exact h i t (List.mem_of_find?_eq_some hm)

theorem StoreOK.rowSlots {p : Program} {store : RowStore} (h : StoreOK p store) :
    ∀ rows : List RowId, RowsOK p (Contract.rowSlots store rows)
  | [] => RowsOK.nil
  | id :: rows => by
    simp only [Contract.rowSlots, List.foldr_cons]
    refine RowsOK.append ?_ (StoreOK.rowSlots h rows)
    cases hf : store.find id with
    | none => exact RowsOK.nil
    | some s => exact h.find hf

/-! ## Rendered elements -/

/-- A rendered element's handlers name existing actions, their curried
arguments typed in a scope the element's bound names satisfy, at most the
action's leading parameters. -/
def VNodeOK (p : Program) (n : VNode) : Prop :=
  ∀ h ∈ n.handlers, ∃ a, p.actions.find? (·.name == h.2.1) = .some a ∧
    ∃ Γ ts, LocalsOK p Γ n.locals ∧ ListTy p (compScope p) Γ h.2.2 ts ∧
      Ty.lePrefix ts (a.params.map (·.2)) = true

/-- Every element of a tree, at any depth, is `VNodeOK`. -/
def ViewOK (p : Program) (vs : List VNode) : Prop := ∀ n, VNode.In n vs → VNodeOK p n

theorem ViewOK.nil {p : Program} : ViewOK p [] := fun _ h => nomatch h

theorem ViewOK.append {p : Program} {a b} (ha : ViewOK p a) (hb : ViewOK p b) : ViewOK p (a ++ b) :=
  fun n hn => (VNode.In.append hn).elim (ha n) (hb n)

theorem ViewOK.single {p : Program} {v : VNode} (hv : VNodeOK p v) (hc : ViewOK p v.children) :
    ViewOK p [v] := fun n hn => by
  cases hn with
  | head => exact hv
  | child h => exact hc n h
  | tail h => exact nomatch h

/-! ## Environments with rows -/

theorem EnvGood.withRows {p : Program} {env : Env} {s} (h : EnvGood p env) (hs : RowsOK p s) :
    EnvGood p { env with rows := s ++ env.rows } :=
  ⟨h.prog, h.slots, h.present, hs.append h.rows, h.settled⟩

theorem LocalsOK.each {p : Program} {Γ ls a x ix} {item : Value} (h : LocalsOK p Γ ls) (hv : ValTy p item a) (i : Nat) :
    LocalsOK p (eachScope x ix a Γ)
      (match ix with
       | .some n => (n, Value.num (F64.ofNat i)) :: (x, item) :: ls
       | .none => (x, item) :: ls) := by
  cases ix with
  | none => exact h.cons hv
  | some n => exact (h.cons hv).cons (by simp [ValTy])

/-! ## The pieces of an element -/

/-- A well-typed environment satisfies `EnvOKE` for any `E` admitting the
legitimate failures but `pending`, given that everything has settled or
`E` admits `pending`. -/
theorem EnvGood.envOKE' {p : Program} (hp : WellTyped p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) {env : Env} (h : EnvGood p env)
    (hc : Complete p { derives := env.derives, resources := env.resources } ∨ E .pending) :
    EnvOKE p E (compScope p) env := by
  have := envOKE_gen (L := fun _ => false) hp hE h.prog h.present'
    (fun _ _ hx _ => h.slots.valTy hp hx) h.rows h.settled (SettledFor.of hc)
  rwa [scopeWithout_none] at this

theorem elementText_good {p : Program} (hfn : ProgOK p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) {fuel G env Γ ls tag pos}
    (henv : EnvOKE p E G env) (hl : LocalsOK p Γ ls)
    (h : (tag == "text" || tag == "tspan" || tag == "option") = true →
      ∀ e ∈ pos.head?, ∃ t, HasTy p G Γ e t ∧ t.displayable = true) :
    GoodW E (fun _ => True) (elementText fuel env ls tag pos) := by
  unfold elementText
  split
  · next e rest =>
    split
    · next ht =>
      obtain ⟨t, he, hd⟩ := h ht e (by simp)
      refine GoodW.bind (eval_sound_E hfn hE henv hl he) fun v hv => ?_
      obtain ⟨s, hs⟩ := display_ok hv hd
      simp [hs, bind, Except.bind, pure, Except.pure, GoodW]
    · trivial
  · trivial

theorem elementTestId_good {p : Program} (hfn : ProgOK p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) {fuel G env Γ ls props}
    (henv : EnvOKE p E G env) (hl : LocalsOK p Γ ls)
    (h : ∀ e ∈ lookupField "testId" props, ∃ t, HasTy p G Γ e t ∧ t.displayable = true) :
    GoodW E (fun _ => True) (elementTestId fuel env ls props) := by
  unfold elementTestId
  split
  · next e he =>
    obtain ⟨t, ht, hd⟩ := h e (by simp [he])
    refine GoodW.bind (eval_sound_E hfn hE henv hl ht) fun v hv => ?_
    obtain ⟨s, hs⟩ := display_ok hv hd
    simp [hs, bind, Except.bind, pure, Except.pure, GoodW]
  · trivial

theorem rowKey_good {E : Err → Prop} (hE : ∀ e, Legit e → e ≠ .pending → E e) (v : Value) :
    GoodW E (fun _ => True) (rowKey v) := by
  unfold rowKey
  split
  · trivial
  · trivial
  · split
    · trivial
    · exact hE _ trivial (by simp)
  · exact hE _ trivial (by simp)

/-- An arm's slots: kept ones are of their types, and fresh ones are
initialized in the arm's scope (`ArmInits`) and checked. -/
theorem armSlots_good {p : Program} (hp : WellTyped p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) {fuel cx Γ ls id owner}
    (henv : EnvGood p cx.env)
    (hc : Complete p { derives := cx.env.derives, resources := cx.env.resources } ∨ E .pending) (hst : StoreOK p cx.store) (hl : LocalsOK p Γ ls)
    (hinit : ArmInits p (compScope p) Γ owner) :
    GoodW E (RowsOK p) (render.armSlots fuel cx ls id owner) := by
  have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
  rw [render.armSlots]
  split
  · next s hs => exact hst.find hs
  refine (GoodW.bind (forIn_goodW (RowsOK p) _ _ _ RowsOK.nil ?_) fun s hs => hs)
  intro st hmem s hs
  rw [henv.prog] at hmem
  dsimp only
  split
  · next ho =>
    have ho' : st.owner = .some owner := by simpa using ho
    obtain ⟨t, ht, -⟩ := hinit st hmem ho'
    refine GoodW.bind (eval_sound_E hfn hE ((henv.withRows hs).envOKE' hp hE hc) hl ht) fun v _ => ?_
    split
    · simp [bind, Except.bind, throw, throwThe, MonadExceptOf.throw, GoodW]; exact hE _ trivial (by simp)
    · next hc =>
      simp only [pure, Except.pure, GoodW, ForInStep.value]
      refine hs.append fun x w hx => ?_
      simp only [List.mem_singleton, Prod.mk.injEq] at hx
      obtain ⟨rfl, rfl⟩ := hx
      rw [slotTy_state hp.names hmem]
      rw [henv.prog] at hc
      simpa using hc
  · exact hs

/-! ## Rendering -/

/-- **Rendering is safe.** A well-typed view rendered in a well-typed
environment, from a well-typed row store, renders elements whose handlers
are `VNodeOK` and a row store of well-typed slots, or fails legitimately:
a row key that is not finite, a slot initialized with a value that is not
finite, out of fuel, a refusal or unsupported entry inside an expression, or
a `pending` read. Never a type error, never an unbound name. -/
theorem render_good {p : Program} (hp : WellTyped p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) : ∀ fuel,
    (∀ {cx Γ ls nodes live}, EnvGood p cx.env →
      (Complete p { derives := cx.env.derives, resources := cx.env.resources } ∨ E .pending) → StoreOK p cx.store → StoreOK p live →
      LocalsOK p Γ ls → NodesTy p (compScope p) Γ nodes →
      GoodW E (fun r => ViewOK p r.1 ∧ StoreOK p r.2) (render fuel cx ls nodes live)) ∧
    (∀ {cx Γ ls tag arm body live}, EnvGood p cx.env →
      (Complete p { derives := cx.env.derives, resources := cx.env.resources } ∨ E .pending) → StoreOK p cx.store → StoreOK p live →
      LocalsOK p Γ ls → ArmInits p (compScope p) Γ (tag, arm) → NodesTy p (compScope p) Γ body →
      GoodW E (fun r => ViewOK p r.1 ∧ StoreOK p r.2) (render.renderArm fuel cx ls tag arm body live)) ∧
    (∀ {cx Γ ls tag x ix key body items i seen live a k}, EnvGood p cx.env →
      (Complete p { derives := cx.env.derives, resources := cx.env.resources } ∨ E .pending) → StoreOK p cx.store →
      StoreOK p live → LocalsOK p Γ ls → ValTys p items a →
      HasTy p (compScope p) (eachScope x ix a Γ) key k →
      ArmInits p (compScope p) (eachScope x ix a Γ) (tag, 0) → NodesTy p (compScope p) (eachScope x ix a Γ) body →
      GoodW E (fun r => ViewOK p r.1 ∧ StoreOK p r.2)
        (render.renderRows fuel cx ls tag x ix key body items i seen live))
  | 0 => by
    refine ⟨?_, ?_, ?_⟩ <;> intros <;> simp [render, render.renderArm, render.renderRows, GoodW, outOfFuel] <;> exact hE _ trivial (by simp)
  | n + 1 => by
    have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
    obtain ⟨ihR, ihA, ihW⟩ := render_good hp hE n
    refine ⟨?_, ?_, ?_⟩
    · intro cx Γ ls nodes live henv hcm hst hlive hl hn
      have hok := henv.envOKE' hp hE hcm
      cases nodes with
      | nil => simp only [render, GoodW]; exact ⟨ViewOK.nil, hlive⟩
      | cons node rest =>
        unfold render
        have hrest : NodesTy p (compScope p) Γ rest := by
          cases node <;> simp only [NodesTy] at hn
          · exact hn.2.2
          · exact hn.2.2.2.2.2
          · exact hn.2
          · exact hn.2.2.2
        refine GoodW.bind (P := fun r => ViewOK p r.1 ∧ StoreOK p r.2) ?_ fun ⟨here, l1⟩ ⟨hv1, hs1⟩ => ?_
        · cases node with
          | element tag pos props hs children =>
            simp only [NodesTy] at hn
            obtain ⟨⟨htext, htid, hhs⟩, hkids, -⟩ := hn
            refine GoodW.bind (elementText_good hfn hE hok hl htext) fun _ _ => ?_
            refine GoodW.bind (elementTestId_good hfn hE hok hl htid) fun _ _ => ?_
            refine GoodW.bind (ihR henv hcm hst hlive hl hkids) fun ⟨kids, l2⟩ ⟨hv2, hs2⟩ => ?_
            refine ⟨ViewOK.single ?_ hv2, hs2⟩
            intro h hh
            obtain ⟨a, ha, ts, hts, hle⟩ := hhs h hh
            exact ⟨a, ha, Γ, ts, hl, hts, hle⟩
          | when tag c thn els =>
            simp only [NodesTy] at hn
            obtain ⟨⟨t, hc, hle⟩, ha0, ha1, hthn, hels, -⟩ := hn
            refine GoodW.bind (eval_sound_E hfn hE hok hl hc) fun v hv => ?_
            obtain ⟨b, rfl⟩ := (hv.mono hle).bool_inv
            cases b
            · exact ihA henv hcm hst hlive hl ha1 hels
            · exact ihA henv hcm hst hlive hl ha0 hthn
          | matchN tag s x sm nn =>
            simp only [NodesTy] at hn
            obtain ⟨⟨a, hs, ha0, hsm⟩, ha1, hnn, -⟩ := hn
            refine GoodW.bind (eval_sound_E hfn hE hok hl hs) fun v hv => ?_
            rcases hv.option_inv with rfl | ⟨w, rfl, hw⟩
            · exact ihA henv hcm hst hlive hl ha1 hnn
            · exact ihA henv hcm hst hlive (hl.cons hw) ha0 hsm
          | each tag x ix list key body =>
            simp only [NodesTy] at hn
            obtain ⟨⟨a, hlist, -, ⟨k, hk, -⟩, hinit, hbody⟩, -⟩ := hn
            refine GoodW.bind (eval_sound_E hfn hE hok hl hlist) fun v hv => ?_
            obtain ⟨xs, rfl, hxs⟩ := hv.list_inv
            exact ihW henv hcm hst hlive hl hxs hk hinit hbody
        · refine GoodW.bind (ihR henv hcm hst hs1 hl hrest) fun ⟨more, l2⟩ ⟨hv2, hs2⟩ => ?_
          exact ⟨hv1.append hv2, hs2⟩
    · intro cx Γ ls tag arm body live henv hcm hst hlive hl hinit hbody
      unfold render.renderArm
      refine GoodW.bind (armSlots_good hp hE henv hcm hst hl hinit) fun slots hslots => ?_
      exact ihR (henv.withRows hslots) hcm hst (hlive.append (StoreOK.single hslots)) hl hbody
    · intro cx Γ ls tag x ix key body items i seen live a k henv hcm hst hlive hl hitems hk hinit hbody
      cases items with
      | nil => simp only [render.renderRows, GoodW]; exact ⟨ViewOK.nil, hlive⟩
      | cons item items =>
        unfold render.renderRows
        have hitem : ValTy p item a := hitems.1
        have hl' := hl.each (x := x) (ix := ix) hitem i
        refine GoodW.bind (eval_sound_E hfn hE (henv.envOKE' hp hE hcm) hl' hk) fun kv _ => ?_
        refine GoodW.bind (rowKey_good hE kv) fun key' _ => ?_
        refine GoodW.bind (armSlots_good hp hE henv hcm hst hl' hinit) fun slots hslots => ?_
        refine GoodW.bind (ihR (henv.withRows hslots) hcm hst (hlive.append (StoreOK.single hslots)) hl' hbody)
          fun ⟨vs, l1⟩ ⟨hv1, hs1⟩ => ?_
        refine GoodW.bind (ihW henv hcm hst hs1 hl hitems.2 hk hinit hbody) fun ⟨more, l2⟩ ⟨hv2, hs2⟩ => ?_
        exact ⟨hv1.append hv2, hs2⟩

end Contract
