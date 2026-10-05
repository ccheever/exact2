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

/-- A failure that is legitimate and not `pending`: what a step of a
well-typed program may fail with. -/
def Strict (e : Err) : Prop := Legit e ∧ e ≠ .pending

theorem Strict.of {e : Err} (h : Legit e) (hn : e ≠ .pending) : Strict e := ⟨h, hn⟩

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

/-- Every declared derive and resource has settled. -/
def Complete (p : Program) (st : Settled) : Prop :=
  (∀ d ∈ p.derives, (lookup d.name st.derives).isSome = true) ∧
    ∀ r ∈ p.resources, (lookup r.name st.resources).isSome = true

/-- The component scope without the names `L` marks (late slots, at boot). -/
def scopeWithout (p : Program) (L : String → Bool) : Scope := (compScope p).filter fun q => !L q.1

theorem lookupTy_filter (x : String) (L : String → Bool) : ∀ (l : Scope),
    lookupTy x (l.filter fun q => !L q.1) = if L x then .none else lookupTy x l
  | [] => by simp [lookupTy]
  | (y, t) :: l => by
    simp only [List.filter_cons, lookupTy]
    by_cases hy : L y = true
    · simp only [hy, Bool.not_true, Bool.false_eq_true, ite_false]
      rw [lookupTy_filter x L l]
      by_cases hxy : (x == y) = true
      · simp only [beq_iff_eq] at hxy; subst hxy; simp [hy]
      · simp [hxy]
    · simp only [Bool.not_eq_true] at hy
      simp only [hy, Bool.not_false, ite_true, lookupTy]
      by_cases hxy : (x == y) = true
      · simp only [beq_iff_eq] at hxy; subst hxy; simp [hy]
      · simp only [hxy, Bool.false_eq_true, ite_false]; exact lookupTy_filter x L l

/-- The derives and resources a scope without `L` reads have settled, or
`E` admits `pending`. -/
def SettledFor (p : Program) (L : String → Bool) (env : Env) (E : Err → Prop) : Prop :=
  E .pending ∨ ∀ x, L x = false → (∀ d ∈ p.derives, d.name = x → (lookup x env.derives).isSome = true) ∧
    (∀ r ∈ p.resources, r.name = x → (lookup x env.resources).isSome = true)

theorem SettledFor.of {p : Program} {L : String → Bool} {env : Env} {E : Err → Prop}
    (h : Complete p { derives := env.derives, resources := env.resources } ∨ E .pending) :
    SettledFor p L env E := by
  rcases h with h | h
  · exact .inr fun x _ => ⟨fun d hd he => he ▸ h.1 d hd, fun r hr he => he ▸ h.2 r hr⟩
  · exact .inl h

/-- **Well-typed environments satisfy `EnvOKE`**, in general form: the root
slots the scope reads (all but `L`'s) are present and of their types, the
row slots in force are of theirs, and the settled values are declared ones
of theirs. A derive or resource not settled reads `pending`, which `E`
must admit unless every one has settled (`Complete`). -/
theorem envOKE_gen {p : Program} (hp : WellTyped p) {E : Err → Prop}
    (hE : ∀ e, Legit e → e ≠ .pending → E e) {L : String → Bool} {env : Env}
    (hprog : env.prog = p)
    (hpres : ∀ x, L x = false → ((∃ st ∈ p.states, st.name = x ∧ st.owner = .none) ∨
      ∃ m ∈ p.mutations, m.name = x) → x ∈ env.slots.map (·.1))
    (hslots : ∀ x v, (x, v) ∈ env.slots → L x = false → ValTy p v (slotTy p x))
    (hrows : RowsOK p env.rows) (hset : SettledOK p { derives := env.derives, resources := env.resources })
    (hcomp : SettledFor p L env E) :
    EnvOKE p E (scopeWithout p L) env := by
  have N := Names.of hp.names
  have refusedE : ∀ w, E (.refused w) := fun w => hE _ trivial (by simp)
  refine ⟨hprog, fun x t hx => ?_, fun x hx hr => ?_⟩
  rotate_left
  · rcases hcomp with hc | hc
    · exact .inr hc
    · left
      simp only [isResource, List.any_eq_true, beq_iff_eq] at hr
      obtain ⟨r, hrm, rfl⟩ := hr
      simp only [scopeWithout, lookupTy_filter] at hx
      split at hx
      · simp at hx
      · next hL => exact (hc _ (by simpa using hL)).2 r hrm rfl
  simp only [scopeWithout, lookupTy_filter] at hx
  split at hx
  · cases hx
  next hL =>
  simp only [Bool.not_eq_true] at hL
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
      obtain ⟨v, hv⟩ := lookup_isSome_of_key (hpres _ hL (.inl ⟨st, hst, rfl, ho⟩))
      simp only [ho, hv, Option.elim]
      have := hslots _ _ (lookup_mem hv) hL
      rwa [hty] at this
    | some o =>
      cases hv : lookup st.name env.rows with
      | none => simp only [ho, hv, Option.elim, GoodW]; exact refusedE _
      | some v =>
        simp only [ho, Option.elim, GoodW]
        have := hrows _ _ (lookup_mem hv)
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
    | none =>
      simp only [Option.elim, GoodW]
      rcases hcomp with hc | hc
      · exact hc
      · have := (hc _ hL).1 d hdm rfl; simp [hv] at this
    | some v =>
      simp only [Option.elim, GoodW]
      obtain ⟨d', hd', hn, hc⟩ := hset.1 _ _ (lookup_mem hv)
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
    | none =>
      simp only [Option.elim, GoodW]
      rcases hcomp with hc | hc
      · exact hc
      · have := (hc _ hL).2 r hrm rfl; simp [hv] at this
    | some v =>
      simp only [Option.elim, GoodW]
      obtain ⟨r', hr', hn, hc⟩ := hset.2 _ _ (lookup_mem hv)
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
  obtain ⟨v, hv⟩ := lookup_isSome_of_key (hpres _ hL (.inr ⟨m, hm, rfl⟩))
  simp only [hv, Option.elim, GoodW]
  have hval := hslots _ _ (lookup_mem hv) hL
  have hmut : isMutation p m.name = true := by simpa [isMutation] using ⟨m, hm, rfl⟩
  obtain ⟨hty, -⟩ := slotTy_mutation hp.names hmut
  rw [hty, distinct_find (name := MutationDecl.name) N.mutations hm] at hval
  exact hval

theorem SlotsPresent.of {p : Program} {slots : List (String × Value)} (h : SlotsPresent p slots) (L : String → Bool) :
    ∀ x, L x = false → ((∃ st ∈ p.states, st.name = x ∧ st.owner = .none) ∨
      ∃ m ∈ p.mutations, m.name = x) → x ∈ slots.map (·.1) := by
  intro x _ hx
  rcases hx with ⟨st, hst, rfl, ho⟩ | ⟨m, hm, rfl⟩
  · exact h.1 st hst ho
  · exact h.2 m hm

theorem EnvGood.present' {p : Program} {env : Env} (h : EnvGood p env) :
    ∀ x, (fun _ => false) x = false → ((∃ st ∈ p.states, st.name = x ∧ st.owner = .none) ∨
      ∃ m ∈ p.mutations, m.name = x) → x ∈ env.slots.map (·.1) := h.present.of _

/-- A smaller scope: what `G'` reads, `G` types alike. -/
theorem EnvOKE.sub {p : Program} {E G G' env} (h : EnvOKE p E G env)
    (hs : ∀ x t, lookupTy x G' = .some t → lookupTy x G = .some t) : EnvOKE p E G' env := by
  refine ⟨h.1, fun x t hx => h.2.1 x t (hs x t hx), fun x hx hr => h.2.2 x ?_ hr⟩
  obtain ⟨t, ht⟩ := Option.isSome_iff_exists.mp hx
  simp [hs x t ht]

theorem lookupTy_mem {x : String} {t : Ty} : ∀ {l : Scope}, lookupTy x l = .some t → (x, t) ∈ l
  | [], h => by simp [lookupTy] at h
  | (y, u) :: l, h => by
    simp only [lookupTy] at h
    split at h
    · next he => cases h; simp only [beq_iff_eq] at he; subst he; simp
    · exact List.mem_cons_of_mem _ (lookupTy_mem h)

theorem lookupTy_of_mem {x : String} {t : Ty} : ∀ {l : Scope}, distinct (l.map (·.1)) = true →
    (x, t) ∈ l → lookupTy x l = .some t
  | [], _, h => by simp at h
  | (y, u) :: l, hd, h => by
    simp only [List.map_cons, distinct, Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_mem,
      decide_eq_false_iff_not] at hd
    simp only [lookupTy]
    simp only [List.mem_cons, Prod.mk.injEq] at h
    rcases h with ⟨rfl, rfl⟩ | h
    · simp
    · have hne : (x == y) = false := by
        simp only [beq_eq_false_iff_ne]
        rintro rfl; exact hd.1 (List.mem_map_of_mem (f := (·.1)) h)
      simp only [hne, Bool.false_eq_true, ite_false]
      exact lookupTy_of_mem hd.2 h

theorem compScope_keys (p : Program) : (compScope p).map (·.1) = compNames p := by
  simp [compScope, compNames, Function.comp_def]

/-- A scope of entries of the component scope, none `L` marks, reads as
`scopeWithout p L` does. -/
theorem lookupTy_scopeWithout {p : Program} (hd : distinct (compNames p) = true) {L : String → Bool}
    {G : Scope} (hG : ∀ x t, (x, t) ∈ G → (x, t) ∈ compScope p ∧ L x = false) :
    ∀ x t, lookupTy x G = .some t → lookupTy x (scopeWithout p L) = .some t := by
  intro x t hx
  obtain ⟨hm, hL⟩ := hG x t (lookupTy_mem hx)
  simp only [scopeWithout, lookupTy_filter, hL, Bool.false_eq_true, ite_false]
  exact lookupTy_of_mem (by rw [compScope_keys]; exact hd) hm

theorem scopeWithout_none (p : Program) : scopeWithout p (fun _ => false) = compScope p := by
  simp [scopeWithout]

/-- **Well-typed environments satisfy `EnvOK`.** Every component name
reads a value of its declared type, or a derive or resource not yet
settled (`pending`), or a row slot outside its row (refused). -/
theorem EnvGood.envOK {p : Program} (hp : WellTyped p) {env : Env} (h : EnvGood p env) :
    EnvOK p (compScope p) env := by
  have := envOKE_gen (E := Legit) (L := fun _ => false) hp (fun _ h _ => h) h.prog h.present'
    (fun _ _ hx _ => h.slots.valTy hp hx) h.rows h.settled (.inl trivial)
  rw [scopeWithout_none] at this
  exact ⟨this.1, fun x t hx => GoodR.of_goodW (this.2.1 x t hx)⟩

/-- **Settled environments never read `pending`.** -/
theorem EnvGood.envOKE {p : Program} (hp : WellTyped p) {env : Env} (h : EnvGood p env)
    (hc : Complete p { derives := env.derives, resources := env.resources }) :
    EnvOKE p Strict (compScope p) env := by
  have := envOKE_gen (E := Strict) (L := fun _ => false) hp (fun _ h hn => ⟨h, hn⟩) h.prog h.present'
    (fun _ _ hx _ => h.slots.valTy hp hx) h.rows h.settled (SettledFor.of (.inl hc))
  rwa [scopeWithout_none] at this

end Contract
