/-
Boot fails only legitimately.

Boot initializes the root slots (each in the scope of the slots before it,
`rootScope`), starts the timers (each interval a literal), settles (the
derives and resources read no lifted slot, `settleScope`, so not the late
slots that still hold `()`), initializes the late slots (each against the
settled values and the late slots before it, `lateScope`) and renders. Each
stage, for a well-typed program, has a value or a `Legitimate` failure.
-/
import Contract.StepSound

namespace Contract

/-! ## Lists -/

theorem foldlM_goodW {α β} {E : Err → Prop} {f : β → α → Result β} (I : List α → β → Prop)
    (step : ∀ a l b, I (a :: l) b → GoodW E (I l) (f b a)) :
    ∀ (l : List α) (b : β), I l b → GoodW E (I []) (l.foldlM f b)
  | [], b, hb => by simp only [List.foldlM_nil]; exact hb
  | a :: l, b, hb => by
    simp only [List.foldlM_cons]
    exact GoodW.bind (step a l b hb) fun b' hb' => foldlM_goodW I step l b' hb'

theorem nodup_of_distinct : ∀ {l : List String}, distinct l = true → l.Nodup
  | [], _ => List.nodup_nil
  | x :: l, h => by
    simp only [distinct, Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_mem,
      decide_eq_false_iff_not] at h
    exact List.nodup_cons.mpr ⟨h.1, nodup_of_distinct h.2⟩

theorem drop_cons_eq {α} {l rest : List α} {k : Nat} {a : α} (h : l.drop k = a :: rest) :
    l[k]? = some a ∧ l.drop (k + 1) = rest ∧ l.take (k + 1) = l.take k ++ [a] := by
  have h1 : l[k]? = some a := by
    have := congrArg List.head? h
    simpa [List.head?_drop] using this
  refine ⟨h1, ?_, ?_⟩
  · have := congrArg List.tail h
    simpa [List.tail_drop] using this
  · rw [List.take_add_one, h1]; rfl

theorem mem_drop_index {α} {l : List α} {k : Nat} {x : α} (h : x ∈ l.drop k) :
    ∃ j, k ≤ j ∧ l[j]? = some x := by
  obtain ⟨i, hi⟩ := List.mem_iff_getElem?.mp h
  rw [List.getElem?_drop] at hi
  exact ⟨k + i, by omega, hi⟩

/-- Two states of distinct names at two indices with one name are one. -/
theorem state_index_eq {p : Program} (N : Names p) {j j' : Nat} {s s' : StateDecl}
    (h : p.states[j]? = some s) (h' : p.states[j']? = some s') (hn : s.name = s'.name) : j = j' := by
  have hnd := nodup_of_distinct N.states
  obtain ⟨hj, rfl⟩ := List.getElem?_eq_some_iff.mp h
  obtain ⟨hj', rfl⟩ := List.getElem?_eq_some_iff.mp h'
  exact (List.Nodup.getElem_inj (xs := p.states.map StateDecl.name) hnd (i := j) (j := j')
    (hi := by simpa using hj) (hj := by simpa using hj')).mp (by simpa using hn)

theorem lateNames_state {l : List StateDecl} {x : String} (h : x ∈ lateNames l) :
    ∃ s ∈ l, s.late = true ∧ s.owner = .none ∧ s.name = x := by
  simp only [lateNames, List.mem_map, List.mem_filter, Bool.and_eq_true, Option.isNone_iff_eq_none] at h
  obtain ⟨s, ⟨hs, hl, ho⟩, rfl⟩ := h
  exact ⟨s, hs, hl, ho, rfl⟩

/-- A slot during boot is of its type unless it is a late slot still to
be initialized. -/
theorem BootOK.valTy {p : Program} (hp : WellTyped p) {L : List String} {x : String} {v : Value}
    (h : BootOK p L x v) (hx : x ∉ L) : ValTy p v (slotTy p x) := by
  have hs : SlotsOK p [(x, v)] := by
    intro y w hy
    simp only [List.mem_singleton, Prod.mk.injEq] at hy
    obtain ⟨rfl, rfl⟩ := hy
    exact ⟨h.1, h.2.imp_right fun h' => h'.resolve_right hx⟩
  exact hs.valTy hp (by simp)

/-- What a late initializer reads is not a late slot still to come. -/
theorem lateScope_not_late {p : Program} (N : Names p) {k : Nat} {x : String} {t : Ty}
    (hx : (x, t) ∈ lateScope p k) : x ∉ lateNames (p.states.drop k) := by
  intro hl
  obtain ⟨s', hs', hlate', ho', rfl⟩ := lateNames_state hl
  obtain ⟨j', hj', hs'j⟩ := mem_drop_index hs'
  have hs'm : s' ∈ p.states := List.mem_of_getElem? hs'j
  simp only [lateScope, List.mem_append, List.mem_map, List.mem_filter, Prod.mk.injEq] at hx
  rcases hx with ((⟨⟨s, j⟩, ⟨hm, hc⟩, hn, -⟩ | ⟨d, hd, hn, -⟩) | ⟨r, hr, hn, -⟩) | ⟨m, hm, hn, -⟩
  · have hsj : p.states[j]? = some s := List.mk_mem_zipIdx_iff_getElem?.mp hm
    have := state_index_eq N hsj hs'j hn
    subst this
    rw [hsj] at hs'j; cases hs'j
    simp only [Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true', decide_eq_true_eq] at hc
    rcases hc.2 with h | h
    · simp [hlate'] at h
    · omega
  · exact N.derive_state d hd s' hs'm hn.symm
  · exact N.resource_state r hr s' hs'm hn.symm
  · exact N.mutation_state m hm s' hs'm hn.symm

/-! ## The boot initializers -/

/-- The root slots during their initialization: the states before the
`k`th that boot initializes have slots, and every slot is a boot slot. -/
def InitInv (p : Program) (rest : List StateDecl) (slots : List (String × Value)) : Prop :=
  ∃ k, rest = p.states.drop k ∧ (∀ s ∈ p.states.take k, s.owner = .none → s.name ∈ slots.map (·.1)) ∧
    ∀ x v, (x, v) ∈ slots → BootOK p (lateNames p.states) x v

theorem rootScope_mem {p : Program} {k : Nat} {x : String} {t : Ty} (h : (x, t) ∈ rootScope p k) :
    ∃ s ∈ p.states.take k, s.owner = .none ∧ s.late = false ∧ s.name = x ∧ s.ty = t := by
  simp only [rootScope, List.mem_map, List.mem_filter, Bool.and_eq_true, Option.isNone_iff_eq_none,
    Bool.not_eq_true', Prod.mk.injEq] at h
  obtain ⟨s, ⟨hs, ho, hl⟩, rfl, rfl⟩ := h
  exact ⟨s, hs, ho, hl, rfl, rfl⟩

/-- A root initializer's environment: the slots before it. -/
theorem initEnvOKE {p : Program} (hp : WellTyped p) {k : Nat} {slots : List (String × Value)}
    (hk : ∀ s ∈ p.states.take k, s.owner = .none → s.name ∈ slots.map (·.1))
    (hb : ∀ x v, (x, v) ∈ slots → BootOK p (lateNames p.states) x v) :
    EnvOKE p Strict (rootScope p k) { prog := p, slots } := by
  have N := Names.of hp.names
  let L : String → Bool := fun x => !((rootScope p k).any (·.1 == x))
  have hL : ∀ x, L x = false → ∃ s ∈ p.states.take k, s.owner = .none ∧ s.late = false ∧ s.name = x := by
    intro x hx
    simp only [L, Bool.not_eq_false', List.any_eq_true, beq_iff_eq] at hx
    obtain ⟨⟨y, t⟩, hm, rfl⟩ := hx
    obtain ⟨s, hs, ho, hl, hn, -⟩ := rootScope_mem hm
    exact ⟨s, hs, ho, hl, hn⟩
  refine EnvOKE.sub (G := scopeWithout p L) ?_ (lookupTy_scopeWithout hp.names fun x t hm => ?_)
  · refine envOKE_gen hp (fun _ h hn => ⟨h, hn⟩) rfl (fun x hx _ => ?_) (fun x v hv hx => ?_)
      RowsOK.nil ⟨by simp, by simp⟩ (.inr fun x hx => ?_)
    · obtain ⟨s, hs, ho, -, rfl⟩ := hL x hx
      exact hk s hs ho
    · obtain ⟨s, hs, -, hl, rfl⟩ := hL x hx
      refine (hb _ _ hv).valTy hp fun hm => ?_
      obtain ⟨s', hs', hl', -, hn⟩ := lateNames_state hm
      have := distinct_eq N.states hs' (List.mem_of_mem_take hs) hn
      subst this; simp [hl] at hl'
    · obtain ⟨s, hs, -, -, rfl⟩ := hL x hx
      have hsm := List.mem_of_mem_take hs
      exact ⟨fun d hd he => absurd he.symm (N.derive_state d hd s hsm),
        fun r hr he => absurd he.symm (N.resource_state r hr s hsm)⟩
  · obtain ⟨s, hs, ho, hl, rfl, rfl⟩ := rootScope_mem hm
    refine ⟨?_, by simp only [L, Bool.not_eq_false', List.any_eq_true, beq_iff_eq]; exact ⟨_, hm, rfl⟩⟩
    simp only [compScope, List.mem_append, List.mem_map, Prod.mk.injEq]
    exact .inl (.inl (.inl ⟨s, List.mem_of_mem_take hs, rfl, rfl⟩))

theorem initSlots_good {p : Program} (hp : WellTyped p) :
    GoodW Strict (fun s => ∀ x v, (x, v) ∈ s → BootOK p (lateNames p.states) x v) (initSlots p) := by
  have N := Names.of hp.names
  unfold initSlots
  refine GoodW.bind (foldlM_goodW (InitInv p) ?_ p.states [] ⟨0, rfl, by simp, by simp⟩) fun s hs => ?_
  · intro a l b ⟨k, hrest, hk, hb⟩
    obtain ⟨hka, hdrop, htake⟩ := drop_cons_eq hrest.symm
    have ham : a ∈ p.states := List.mem_of_getElem? hka
    have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
    have refusedS : ∀ w, Strict (.refused w) := fun _ => ⟨trivial, by simp⟩
    have app : ∀ v, BootOK p (lateNames p.states) a.name v → InitInv p l (b ++ [(a.name, v)]) := by
      intro v hv
      refine ⟨k + 1, hdrop.symm, fun s hs ho => ?_, fun x w hx => ?_⟩
      · rw [htake] at hs
        simp only [List.map_append, List.mem_append, List.map_cons, List.map_nil, List.mem_singleton]
        rcases List.mem_append.mp hs with hs | hs
        · exact .inl (hk s hs ho)
        · simp only [List.mem_singleton] at hs; subst hs; exact .inr rfl
      · rcases List.mem_append.mp hx with hx | hx
        · exact hb x w hx
        · simp only [List.mem_singleton, Prod.mk.injEq] at hx; obtain ⟨rfl, rfl⟩ := hx; exact hv
    split
    · next hown =>
      refine ⟨k + 1, hdrop.symm, fun s hs ho => ?_, hb⟩
      rw [htake] at hs
      rcases List.mem_append.mp hs with hs | hs
      · exact hk s hs ho
      · simp only [List.mem_singleton] at hs; subst hs; simp [ho] at hown
    next hown =>
    have ho : a.owner = .none := by simpa using hown
    try dsimp only
    split
    · next hlate =>
      refine app _ ⟨isSlot_of_state ham, .inr (.inr ?_)⟩
      simp only [lateNames, List.mem_map, List.mem_filter, Bool.and_eq_true]
      exact ⟨a, ⟨ham, hlate, by simp [ho]⟩, rfl⟩
    next hlate =>
    try dsimp only
    split
    · next hrouter =>
      split
      · next r _ =>
        exact app _ ⟨isSlot_of_state ham, .inr (.inl ⟨by simpa using hrouter, r, rfl⟩)⟩
      · simp only [throw, throwThe, MonadExceptOf.throw, bind, Except.bind, GoodW]; exact refusedS _
    next hrouter =>
    obtain ⟨t, ht, -⟩ := hp.rootInits k a hka ho (by simpa using hlate) (by simpa using hrouter)
    refine GoodW.bind (eval_sound_E hfn (fun _ h hn => ⟨h, hn⟩) (initEnvOKE hp hk hb) LocalsOK.nil ht)
      fun v _ => ?_
    split
    · simp only [throw, throwThe, MonadExceptOf.throw, bind, Except.bind, GoodW]; exact refusedS _
    · next hc =>
      refine app _ ⟨isSlot_of_state ham, .inl ?_⟩
      rw [slotTy_state hp.names ham]; simpa using hc
  · obtain ⟨_, _, _, hb⟩ := hs
    intro x v hx
    rcases List.mem_append.mp hx with hx | hx
    · exact hb x v hx
    · obtain ⟨m, hm, he⟩ := List.mem_map.mp hx
      simp only [Prod.mk.injEq] at he
      obtain ⟨rfl, rfl⟩ := he
      have hmut : isMutation p m.name = true := by
        simp only [isMutation, List.any_eq_true, beq_iff_eq]; exact ⟨m, hm, rfl⟩
      refine ⟨by simp [isSlot, hmut], .inl ?_⟩
      rw [(slotTy_mutation hp.names hmut).1]
      simp [conforms]

/-! ## The timers -/

theorem startTimers_good {p : Program} (hp : WellTyped p) (slots : List (String × Value)) :
    GoodW Strict (fun _ => True) (startTimers p slots) := by
  unfold startTimers
  refine mapM_goodW fun t ht => ?_
  split
  · simp [pure, Except.pure, GoodW]
  · obtain ⟨b, hb⟩ := hp.taskLiterals t ht
    rw [hb, show fuel = (fuel - 1) + 1 from rfl]
    simp [eval, Value.asNum, bind, Except.bind, pure, Except.pure, GoodW]

/-! ## The late initializers -/

/-- The slots during the late initializers: the boot slots' names, and
every slot a boot slot whose late slots still to come are the `k`th on. -/
def LateInv (p : Program) (keys : List String) (rest : List StateDecl) (slots : List (String × Value)) :
    Prop :=
  ∃ k, rest = p.states.drop k ∧ slots.map (·.1) = keys ∧
    ∀ x v, (x, v) ∈ slots → BootOK p (lateNames (p.states.drop k)) x v

theorem lateScope_comp {p : Program} {k : Nat} {x : String} {t : Ty} (h : (x, t) ∈ lateScope p k) :
    (x, t) ∈ compScope p := by
  simp only [lateScope, List.mem_append, List.mem_map, List.mem_filter, Prod.mk.injEq] at h
  simp only [compScope, List.mem_append, List.mem_map, Prod.mk.injEq]
  rcases h with ((⟨⟨s, j⟩, ⟨hm, -⟩, rfl, rfl⟩ | ⟨d, hd, rfl, rfl⟩) | ⟨r, hr, rfl, rfl⟩) | ⟨m, hm, rfl, rfl⟩
  · exact .inl (.inl (.inl ⟨s, List.mem_of_getElem? (List.mk_mem_zipIdx_iff_getElem?.mp hm), rfl, rfl⟩))
  · exact .inl (.inl (.inr ⟨d, hd, rfl, rfl⟩))
  · exact .inl (.inr ⟨r, hr, rfl, rfl⟩)
  · exact .inr ⟨m, hm, rfl, rfl⟩

/-- A late initializer's environment: the boot slots, the late ones
before it initialized, everything settled. -/
theorem lateEnvOKE {p : Program} (hp : WellTyped p) {k : Nat} {st : Settled} {slots : List (String × Value)}
    (hpres : SlotsPresent p slots) (hb : ∀ x v, (x, v) ∈ slots → BootOK p (lateNames (p.states.drop k)) x v)
    (hst : SettledOK p st) (hc : Complete p st) :
    EnvOKE p Strict (lateScope p k)
      { prog := p, slots, derives := st.derives, resources := st.resources, now := 0 } := by
  have N := Names.of hp.names
  let L : String → Bool := fun x => !((lateScope p k).any (·.1 == x))
  have hL : ∀ x, L x = false → ∃ t, (x, t) ∈ lateScope p k := by
    intro x hx
    simp only [L, Bool.not_eq_false', List.any_eq_true, beq_iff_eq] at hx
    obtain ⟨⟨y, t⟩, hm, rfl⟩ := hx
    exact ⟨t, hm⟩
  refine EnvOKE.sub (G := scopeWithout p L) ?_ (lookupTy_scopeWithout hp.names fun x t hm => ?_)
  · refine envOKE_gen hp (fun _ h hn => ⟨h, hn⟩) rfl (hpres.of _) (fun x v hv hx => ?_)
      RowsOK.nil ⟨hst.1, hst.2⟩ (SettledFor.of (.inl hc))
    obtain ⟨t, ht⟩ := hL x hx
    exact (hb _ _ hv).valTy hp (lateScope_not_late N ht)
  · exact ⟨lateScope_comp hm, by
      simp only [L, Bool.not_eq_false', List.any_eq_true, beq_iff_eq]; exact ⟨_, hm, rfl⟩⟩

theorem lateSlots_good {p : Program} (hp : WellTyped p) {st : Settled} {slots₀ : List (String × Value)}
    (hpres : SlotsPresent p slots₀) (hb : ∀ x v, (x, v) ∈ slots₀ → BootOK p (lateNames p.states) x v)
    (hst : SettledOK p st) (hc : Complete p st) :
    GoodW Strict (fun s => SlotsOK p s ∧ s.map (·.1) = slots₀.map (·.1)) (lateSlots p st slots₀) := by
  have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
  unfold lateSlots
  refine (foldlM_goodW (LateInv p (slots₀.map (·.1))) ?_ p.states slots₀
    ⟨0, rfl, rfl, by simpa using hb⟩).imp fun s ⟨k, hk, hkeys, hb'⟩ => ⟨fun x v hx => ?_, hkeys⟩
  · intro a l b ⟨k, hrest, hkeys, hbk⟩
    obtain ⟨hka, hdrop, -⟩ := drop_cons_eq hrest.symm
    have ham : a ∈ p.states := List.mem_of_getElem? hka
    rw [← hrest] at hbk
    try dsimp only
    split
    · next hskip =>
      refine ⟨k + 1, hdrop.symm, hkeys, fun x v hx => ?_⟩
      obtain ⟨h1, h2⟩ := hbk x v hx
      rw [hdrop]
      refine ⟨h1, h2.imp_right fun h => h.imp_right fun h => ?_⟩
      rw [lateNames_cons] at h
      have : (a.late && a.owner.isNone) = false := by
        cases hl : a.late <;> cases ho : a.owner <;> simp_all
      simpa [this] using h
    next hskip =>
    have hla : a.late = true := by cases hl : a.late <;> simp_all
    have ho : a.owner = .none := by cases ho : a.owner <;> simp_all
    obtain ⟨t, ht, -⟩ := hp.lateInits k a hka ho hla
    have hpb : SlotsPresent p b := hpres.of_keys hkeys
    refine GoodW.bind (eval_sound_E hfn (fun _ h hn => ⟨h, hn⟩)
      (lateEnvOKE hp hpb (by rw [← hrest]; exact hbk) hst hc) LocalsOK.nil ht) fun v _ => ?_
    split
    · simp only [throw, throwThe, MonadExceptOf.throw, bind, Except.bind, GoodW]; exact ⟨trivial, by simp⟩
    · next hconf =>
      refine ⟨k + 1, hdrop.symm, by rw [setSlot_keys]; exact hkeys, fun x w hx => ?_⟩
      obtain ⟨u, hu, hcase⟩ := mem_setSlot' hx
      obtain ⟨hs, hcu⟩ := hbk x u hu
      refine ⟨hs, ?_⟩
      rcases hcase with ⟨rfl, rfl⟩ | ⟨hne, rfl⟩
      · left; rw [slotTy_state hp.names ham]; simpa using hconf
      · rw [hdrop]
        refine hcu.imp_right fun h => h.imp_right fun h => ?_
        rw [lateNames_cons] at h
        have : (a.late && a.owner.isNone) = true := by simp [hla, ho]
        simp only [this, ite_true, List.mem_cons] at h
        rcases h with h | h
        · exact absurd h.symm hne
        · exact h
  · rw [← hk] at hb'
    obtain ⟨h1, h2⟩ := hb' x v hx
    exact ⟨h1, h2.imp_right fun h => h.resolve_right (by simp [lateNames])⟩

/-! ## Boot -/

theorem lateNames_lifted {p : Program} {x : String} (h : x ∈ lateNames p.states) : lifted p x = true := by
  obtain ⟨s, hs, hl, -, rfl⟩ := lateNames_state h
  simp only [lifted, List.any_eq_true, Bool.and_eq_true, beq_iff_eq, Bool.or_eq_true]
  exact ⟨s, hs, rfl, .inl hl⟩

/-- **Boot fails only legitimately.** For a well-typed program, boot
commits, or is refused for a `Legitimate` reason: never a type error, an
unbound name or a `pending` read. -/
theorem boot_sound {p : Program} (hp : WellTyped p) (o : Oracle) : OutcomeOK (boot p o).2 := by
  have N := Names.of hp.names
  have ofW : ∀ {α} {P : α → Prop} {r : Result α} {e}, GoodW Strict P r → r = .error e → Legitimate e :=
    fun h he => by rw [he] at h; exact Legitimate.of_strict h
  unfold boot
  split
  · next e he => exact ofW (initSlots_good hp) he
  next slots₀ hi =>
  have hb := initSlots_good hp
  rw [hi] at hb
  have hpres := initSlots_present hi
  split
  · next e he => exact ofW (startTimers_good hp slots₀) he
  next timers ht =>
  split
  · next e he =>
    exact ofW (settle_strict (o := o) (now := 0) (prev := {}) (force := []) N.resources
      (settleEnvOK hp (hpres.of _) fun x v hv hx =>
        (hb x v hv).valTy hp fun hm => by simp [lateNames_lifted hm] at hx) SettledOK.empty) he
  next st hst =>
  have hok := settle_settledOK N.resources SettledOK.empty hst
  have hc := settle_complete hst
  have hl := lateSlots_good hp hpres hb hok hc
  split
  · next e he => exact ofW hl he
  next slots hls =>
  rw [hls] at hl
  obtain ⟨hs, hkeys⟩ := hl
  dsimp only
  split
  · trivial
  · next e he =>
    exact ofW ((render_good hp (E := Strict) (fun _ h hn => ⟨h, hn⟩) fuel).1
      (cx := { env := { prog := p, slots, derives := st.derives, resources := st.resources, now := 0 },
               store := [] })
      ⟨rfl, hs, hpres.of_keys hkeys, RowsOK.nil, hok.1, hok.2⟩ (.inl hc) StoreOK.nil StoreOK.nil LocalsOK.nil
      hp.view) he

/-! ## Runs -/

/-- A refusal or a poison, and why. -/
def Outcome.failure : Outcome → Option Err
  | .ok => .none
  | .refused e => .some e
  | .poisoned e => .some e

theorem OutcomeOK.failure {out : Outcome} (h : OutcomeOK out) :
    ∀ e, out.failure = .some e → Legitimate e ∧ (∀ w, e ≠ .type w) ∧ (∀ x, e ≠ .unbound x) ∧ e ≠ .pending := by
  intro e he
  have hl : Legitimate e := by
    cases out <;> simp [Outcome.failure] at he <;> subst he <;> exact h
  exact ⟨hl, hl.not_type, hl.not_unbound, hl.not_pending⟩

/-- **Well-typed programs don't go wrong.** For a program the checker
accepts: boot commits or fails for a `Legitimate` reason; every reachable
configuration is well typed (`ConfigOK`); and every event from one, whatever
the oracle answers, lands in a well-typed configuration and commits, or
refuses or poisons for a `Legitimate` reason — never a type error, an
unbound name or a `pending` read. -/
theorem dont_go_wrong {p : Program} (hcheck : check p = true) :
    (∀ o, OutcomeOK (boot p o).2) ∧
    ∀ c, Reachable p c → ConfigOK p c ∧ ∀ o (ev : Event),
      ConfigOK p (ev.step p o c).1 ∧ OutcomeOK (ev.step p o c).2 := by
  have hp := check_sound hcheck
  refine ⟨boot_sound hp, fun c h => ?_⟩
  have hc := reachable_configOK hp h
  exact ⟨hc, fun o ev => step_sound hp hc o ev⟩

/-- The same, failure by failure: no step of a well-typed program, boot
included, fails with a type error, an unbound name or a `pending` read. -/
theorem never_wrong {p : Program} (hcheck : check p = true) :
    (∀ o e, (boot p o).2.failure = .some e →
      Legitimate e ∧ (∀ w, e ≠ .type w) ∧ (∀ x, e ≠ .unbound x) ∧ e ≠ .pending) ∧
    ∀ c, Reachable p c → ∀ o (ev : Event) e, (ev.step p o c).2.failure = .some e →
      Legitimate e ∧ (∀ w, e ≠ .type w) ∧ (∀ x, e ≠ .unbound x) ∧ e ≠ .pending := by
  obtain ⟨hb, hr⟩ := dont_go_wrong hcheck
  exact ⟨fun o => (hb o).failure, fun c h o ev => ((hr c h).2 o ev).2.failure⟩

end Contract
