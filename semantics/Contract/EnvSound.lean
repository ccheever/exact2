/-
Well-typed environments: what a configuration must hold for `EnvOK` —
the hypothesis of `eval_sound_ty` and `exec_sound_ty` — to be a theorem
about it rather than an assumption.

`EnvGood p env` is concrete: every root slot is present and of its type
(`SlotsOK`, `SlotsPresent`), the row slots in force are of their types
(`RowsOK`), and every settled derive and resource is a declared one
holding a value of its declared type (`SettledOK`). `EnvGood.envOK` turns
it into `EnvOK p (compScope p) env`, using that component names are
distinct.
-/
import Contract.TypeInvariant

namespace Contract

/-! ## Results with a chosen failure property -/

/-- A value with property `P`, or a failure with property `E`. `GoodR P`
is `GoodW Legit P`. -/
def GoodW {α} (E : Err → Prop) (P : α → Prop) : Result α → Prop
  | .ok a => P a
  | .error e => E e

theorem GoodW.bind {α β} {E : Err → Prop} {P : α → Prop} {Q : β → Prop} {r : Result α}
    {f : α → Result β} (h : GoodW E P r) (hf : ∀ a, P a → GoodW E Q (f a)) : GoodW E Q (r >>= f) := by
  cases r with
  | ok a => exact hf a h
  | error e => exact h

theorem GoodW.mono {α} {E E' : Err → Prop} {P Q : α → Prop} {r : Result α} (h : GoodW E P r)
    (he : ∀ e, E e → E' e) (hq : ∀ a, P a → Q a) : GoodW E' Q r := by
  cases r with
  | ok a => exact hq a h
  | error e => exact he e h

theorem GoodW.of_goodR {α} {P : α → Prop} {r : Result α} (h : GoodR P r) : GoodW Legit P r := by
  cases r <;> exact h

theorem GoodR.of_goodW {α} {P : α → Prop} {r : Result α} (h : GoodW Legit P r) : GoodR P r := by
  cases r <;> exact h

/-- A `for` loop in `Except`: an invariant every iteration keeps, with
every failure of the body's kind. -/
theorem forIn_goodW {α σ} {E : Err → Prop} (I : σ → Prop) :
    ∀ (l : List α) (init : σ) (f : α → σ → Except Err (ForInStep σ)),
    I init → (∀ a ∈ l, ∀ s, I s → GoodW E (fun r => I r.value) (f a s)) →
    GoodW E I (forIn l init f)
  | [], init, f, h0, _ => by rw [List.forIn_nil]; exact h0
  | a :: l, init, f, h0, hf => by
    rw [List.forIn_cons]
    have h1 := hf a (by simp) init h0
    cases h : f a init with
    | error e => rw [h] at h1; exact h1
    | ok r =>
      rw [h] at h1
      cases r with
      | done b => exact h1
      | yield b =>
        exact forIn_goodW I l b f h1 fun a' ha' s hs => hf a' (by simp [ha']) s hs

/-! ## Distinct component names -/

theorem distinct_eq {α} {name : α → String} {l : List α} {a b : α} (hd : distinct (l.map name) = true)
    (ha : a ∈ l) (hb : b ∈ l) (he : name a = name b) : a = b := by
  have h1 := distinct_find hd ha
  have h2 := distinct_find hd hb
  rw [he] at h1
  rw [h1] at h2; cases h2; rfl

/-- What distinct component names give: each kind's names are distinct,
and no name is of two kinds. -/
structure Names (p : Program) : Prop where
  states : distinct (p.states.map (·.name)) = true
  derives : distinct (p.derives.map (·.name)) = true
  resources : distinct (p.resources.map (·.name)) = true
  mutations : distinct (p.mutations.map (·.name)) = true
  derive_state : ∀ d ∈ p.derives, ∀ s ∈ p.states, s.name ≠ d.name
  resource_state : ∀ r ∈ p.resources, ∀ s ∈ p.states, s.name ≠ r.name
  resource_derive : ∀ r ∈ p.resources, ∀ d ∈ p.derives, d.name ≠ r.name
  mutation_state : ∀ m ∈ p.mutations, ∀ s ∈ p.states, s.name ≠ m.name
  mutation_derive : ∀ m ∈ p.mutations, ∀ d ∈ p.derives, d.name ≠ m.name
  mutation_resource : ∀ m ∈ p.mutations, ∀ r ∈ p.resources, r.name ≠ m.name

theorem Names.of {p : Program} (hd : distinct (compNames p) = true) : Names p := by
  have h := distinct_append (l₁ := p.states.map (·.name) ++ p.derives.map (·.name) ++
    p.resources.map (·.name)) (l₂ := p.mutations.map (·.name))
    (by simpa [compNames, List.append_assoc] using hd)
  have h1 := distinct_append (l₁ := p.states.map (·.name) ++ p.derives.map (·.name)) h.1
  have h2 := distinct_append (l₁ := p.states.map (·.name)) h1.1
  refine ⟨h2.1, h2.2.1, h1.2.1, h.2.1, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro d hd s hs he
    exact h2.2.2 d.name (List.mem_map_of_mem hd) (he ▸ List.mem_map_of_mem hs)
  · intro r hr s hs he
    exact h1.2.2 r.name (List.mem_map_of_mem hr) (by simp only [List.mem_append]; exact .inl (he ▸ List.mem_map_of_mem hs))
  · intro r hr d hd he
    exact h1.2.2 r.name (List.mem_map_of_mem hr) (by simp only [List.mem_append]; exact .inr (he ▸ List.mem_map_of_mem hd))
  · intro m hm s hs he
    exact h.2.2 m.name (List.mem_map_of_mem hm)
      (by simp only [List.mem_append]; exact .inl (.inl (he ▸ List.mem_map_of_mem hs)))
  · intro m hm d hd he
    exact h.2.2 m.name (List.mem_map_of_mem hm)
      (by simp only [List.mem_append]; exact .inl (.inr (he ▸ List.mem_map_of_mem hd)))
  · intro m hm r hr he
    exact h.2.2 m.name (List.mem_map_of_mem hm)
      (by simp only [List.mem_append]; exact .inr (he ▸ List.mem_map_of_mem hr))

/-! ## Scopes -/

theorem lookupTy_append (x : String) : ∀ (A B : Scope), lookupTy x (A ++ B) = (lookupTy x A).or (lookupTy x B)
  | [], B => by simp [lookupTy]
  | (y, t) :: A, B => by
    simp only [List.cons_append, lookupTy]
    split
    · rfl
    · exact lookupTy_append x A B

theorem lookupTy_map {α} {name : α → String} {ty : α → Ty} {x : String} {t : Ty} :
    ∀ {l : List α}, lookupTy x (l.map fun a => (name a, ty a)) = .some t → ∃ a ∈ l, name a = x ∧ ty a = t
  | [], h => by simp [lookupTy] at h
  | a :: l, h => by
    simp only [List.map_cons, lookupTy] at h
    split at h
    · next he => cases h; exact ⟨a, by simp, (by simpa using he : x = name a).symm, rfl⟩
    · obtain ⟨b, hb, h⟩ := lookupTy_map h
      exact ⟨b, by simp [hb], h⟩

theorem lookupTy_map_none {α} {name : α → String} {ty : α → Ty} {x : String} :
    ∀ {l : List α}, lookupTy x (l.map fun a => (name a, ty a)) = .none → ∀ a ∈ l, name a ≠ x
  | [], _, _, ha => by simp at ha
  | b :: l, h, a, ha => by
    simp only [List.map_cons, lookupTy] at h
    split at h
    · cases h
    · next hne =>
      simp only [List.mem_cons] at ha
      rcases ha with rfl | ha
      · intro he; simp [he] at hne
      · exact lookupTy_map_none h a ha

theorem lookup_isSome_of_key {x : String} : ∀ {l : List (String × Value)}, x ∈ l.map (·.1) → ∃ v, lookup x l = .some v
  | [], h => by simp at h
  | (y, w) :: l, h => by
    simp only [lookup]
    split
    · exact ⟨w, rfl⟩
    · next he =>
      simp only [List.map_cons, List.mem_cons] at h
      rcases h with h | h
      · subst h; simp at he
      · exact lookup_isSome_of_key h

/-! ## Well-typed environments -/

/-- Every root state and every mutation has a slot. -/
def SlotsPresent (p : Program) (slots : List (String × Value)) : Prop :=
  (∀ st ∈ p.states, st.owner = .none → st.name ∈ slots.map (·.1)) ∧
    ∀ m ∈ p.mutations, m.name ∈ slots.map (·.1)

theorem setSlot_keys (slots : List (String × Value)) (x : String) (v : Value) :
    (setSlot slots x v).map (·.1) = slots.map (·.1) := by
  simp only [setSlot, List.map_map]
  congr 1; funext ⟨y, w⟩; simp only [Function.comp]; split <;> rfl

theorem applyWrites_keys : ∀ (slots ws : List (String × Value)),
    (applyWrites slots ws).map (·.1) = slots.map (·.1)
  | _, [] => rfl
  | slots, (x, v) :: ws => by
    simp only [applyWrites, List.foldl_cons]
    rw [show (ws.foldl (fun s (x, v) => setSlot s x v) (setSlot slots x v)) =
      applyWrites (setSlot slots x v) ws from rfl, applyWrites_keys, setSlot_keys]

theorem SlotsPresent.of_keys {p : Program} {a b : List (String × Value)} (h : SlotsPresent p a)
    (hk : b.map (·.1) = a.map (·.1)) : SlotsPresent p b := by
  rw [SlotsPresent, hk]; exact h

/-- Row slots of their types. -/
def RowsOK (p : Program) (rows : List (String × Value)) : Prop :=
  ∀ x v, (x, v) ∈ rows → conforms p v (slotTy p x) = true

theorem RowsOK.append {p : Program} {a b} (ha : RowsOK p a) (hb : RowsOK p b) : RowsOK p (a ++ b) :=
  fun x v h => (List.mem_append.mp h).elim (ha x v) (hb x v)

theorem RowsOK.nil {p : Program} : RowsOK p [] := fun _ _ h => nomatch h

theorem RowsOK.setSlot {p : Program} {s x v} (h : RowsOK p s) (hv : conforms p v (slotTy p x) = true) :
    RowsOK p (Contract.setSlot s x v) := by
  intro y w hy
  rcases mem_setSlot hy with hy | ⟨rfl, rfl⟩
  · exact h y w hy
  · exact hv

/-- Every settled derive and resource is a declared one, holding a value
the runtime check admits at its declared type. -/
def SettledOK (p : Program) (st : Settled) : Prop :=
  (∀ x v, (x, v) ∈ st.derives → ∃ d ∈ p.derives, d.name = x ∧ conforms p v d.ty = true) ∧
    (∀ x v, (x, v) ∈ st.resources → ∃ r ∈ p.resources, r.name = x ∧ conforms p v r.ty = true)

theorem SettledOK.empty {p : Program} : SettledOK p {} := ⟨by simp, by simp⟩

/-- An environment a well-typed program evaluates in. -/
structure EnvGood (p : Program) (env : Env) : Prop where
  prog : env.prog = p
  slots : SlotsOK p env.slots
  present : SlotsPresent p env.slots
  rows : RowsOK p env.rows
  settled : SettledOK p { derives := env.derives, resources := env.resources }

/-- A root slot holds a value of its slot type (the router slot's boot
value included). -/
theorem SlotsOK.valTy {p : Program} (hp : WellTyped p) {slots : List (String × Value)} (h : SlotsOK p slots)
    {x v} (hx : (x, v) ∈ slots) : ValTy p v (slotTy p x) := by
  obtain ⟨hs, hc⟩ := h x v hx
  rcases hc with hc | ⟨hrx, r, rfl⟩
  · refine conforms_valTy hp.shapes v hc ?_
    simp only [isSlot, Bool.or_eq_true, List.any_eq_true, beq_iff_eq] at hs
    rcases hs with ⟨st, hst, rfl⟩ | hm
    · rw [slotTy_state hp.names hst]; exact hp.states st hst
    · have hm' : isMutation p x = true := by simpa [isMutation] using hm
      obtain ⟨hty, md, hmd⟩ := slotTy_mutation hp.names hm'
      rw [hty, hmd]
      exact hp.mutations md (List.mem_of_find?_eq_some hmd)
  · have hslot := hp.routerSlot
    simp only [routerSlotOK, hrx, Bool.and_eq_true, List.any_eq_true, decide_eq_true_eq,
      beq_iff_eq] at hslot
    obtain ⟨⟨st, hst, ⟨⟨⟨rfl, hty⟩, -⟩, -⟩⟩, hshape⟩ := hslot
    rw [slotTy_state hp.names hst, hty]
    exact routerValue_ty (RouteShapes.of hp.routeShapes hshape) r

theorem find_state {p : Program} (N : Names p) {st : StateDecl} (hst : st ∈ p.states) :
    p.states.find? (·.name == st.name) = .some st :=
  distinct_find (name := StateDecl.name) N.states hst

theorem find_state_none {p : Program} {x : String} (h : ∀ s ∈ p.states, s.name ≠ x) :
    p.states.find? (·.name == x) = .none := by
  rw [List.find?_eq_none]; intro s hs; simpa using h s hs

theorem any_name {α} {name : α → String} {l : List α} {x : String} :
    (l.any (fun a => name a == x)) = true ↔ ∃ a ∈ l, name a = x := by
  simp

/-- **Well-typed environments satisfy `EnvOK`.** Every component name
reads a value of its declared type, or a derive or resource not yet
settled (`pending`), or a row slot outside its row (refused). -/
theorem EnvGood.envOK {p : Program} (hp : WellTyped p) {env : Env} (h : EnvGood p env) :
    EnvOK p (compScope p) env := by
  have N := Names.of hp.names
  refine ⟨h.prog, fun x t hx => ?_⟩
  have hprog := h.prog
  simp only [compScope, lookupTy_append] at hx
  unfold Env.global
  rw [hprog]
  -- states
  cases hs : lookupTy x (p.states.map fun s => (s.name, s.ty)) with
  | some t' =>
    rw [hs] at hx; simp only [Option.some_or] at hx; cases hx
    obtain ⟨st, hst, rfl, rfl⟩ := lookupTy_map hs
    rw [find_state N hst]
    have hty : slotTy p st.name = st.ty := slotTy_state hp.names hst
    cases ho : st.owner with
    | none =>
      obtain ⟨v, hv⟩ := lookup_isSome_of_key (h.present.1 st hst ho)
      simp only [ho, hv, Option.elim]
      have := h.slots.valTy hp (lookup_mem hv)
      rwa [hty] at this
    | some o =>
      cases hv : lookup st.name env.rows with
      | none => simp [ho, Option.elim, GoodR, Legit]
      | some v =>
        simp only [ho, Option.elim, GoodR]
        have := h.rows _ _ (lookup_mem hv)
        rw [hty] at this
        exact conforms_valTy hp.shapes v this (hp.states st hst)
  | none =>
  rw [hs] at hx; simp only [Option.none_or] at hx
  have hns : ∀ s ∈ p.states, s.name ≠ x := lookupTy_map_none hs
  rw [find_state_none hns]
  simp only
  cases hd : lookupTy x (p.derives.map fun d => (d.name, d.ty)) with
  | some t' =>
    rw [hd] at hx; simp only [Option.some_or] at hx; cases hx
    obtain ⟨d, hdm, rfl, rfl⟩ := lookupTy_map hd
    have : p.derives.any (·.name == d.name) = true := any_name.mpr ⟨d, hdm, rfl⟩
    simp only [this, ite_true]
    cases hv : lookup d.name env.derives with
    | none => simp [Option.elim, GoodR, Legit]
    | some v =>
      simp only [Option.elim, GoodR]
      obtain ⟨d', hd', hn, hc⟩ := h.settled.1 _ _ (lookup_mem hv)
      have := distinct_eq N.derives hd' hdm hn; subst this
      exact conforms_valTy hp.shapes v hc (hp.derivesComplete d' hd')
  | none =>
  rw [hd] at hx; simp only [Option.none_or] at hx
  cases hr : lookupTy x (p.resources.map fun r => (r.name, r.ty)) with
  | some t' =>
    rw [hr] at hx; simp only [Option.some_or] at hx; cases hx
    obtain ⟨r, hrm, rfl, rfl⟩ := lookupTy_map hr
    have hnd : p.derives.any (·.name == r.name) = false := by
      simpa using fun d hd => N.resource_derive r hrm d hd
    have : p.resources.any (·.name == r.name) = true := any_name.mpr ⟨r, hrm, rfl⟩
    simp only [hnd, this, ite_true, Bool.false_eq_true, ite_false]
    cases hv : lookup r.name env.resources with
    | none => simp [Option.elim, GoodR, Legit]
    | some v =>
      simp only [Option.elim, GoodR]
      obtain ⟨r', hr', hn, hc⟩ := h.settled.2 _ _ (lookup_mem hv)
      have := distinct_eq N.resources hr' hrm hn; subst this
      exact conforms_valTy hp.shapes v hc (hp.resourcesComplete r' hr')
  | none =>
  rw [hr] at hx
  simp only [Option.none_or] at hx
  obtain ⟨m, hm, rfl, rfl⟩ := lookupTy_map hx
  have hnd : p.derives.any (·.name == m.name) = false := by
    simpa using fun d hd => N.mutation_derive m hm d hd
  have hnr : p.resources.any (·.name == m.name) = false := by
    simpa using fun r hr => N.mutation_resource m hm r hr
  have : p.mutations.any (·.name == m.name) = true := any_name.mpr ⟨m, hm, rfl⟩
  simp only [hnd, hnr, this, ite_true, Bool.false_eq_true, ite_false]
  obtain ⟨v, hv⟩ := lookup_isSome_of_key (h.present.2 m hm)
  simp only [hv, Option.elim, GoodR]
  have hval := h.slots.valTy hp (lookup_mem hv)
  have hmut : isMutation p m.name = true := by simpa [isMutation] using ⟨m, hm, rfl⟩
  obtain ⟨hty, -⟩ := slotTy_mutation hp.names hmut
  rw [hty, distinct_find (name := MutationDecl.name) N.mutations hm] at hval
  exact hval

end Contract
