/-
The program-level invariant: in every configuration a well-typed program
reaches — boot, then any events — every root slot holds a value of its
declared type.

Honestly told, most of the work is the runtime's: boot checks each
initializer's value (`conforms`), and a commit checks each write and each
source answer before it lands, refusing the event otherwise. What typing
adds is what those checks rely on: component names are distinct, so a
slot's declared type is the one its writes are checked against; and a
`send` targets a mutation (`StmtsTy`), so the answer it lands in a slot is
checked at that slot's type — a `send` to a state would be checked against
`?` (`slotTy` of a non-mutation) and could put an `option` in a `number`
slot. Finiteness of numbers is likewise the runtime check's (a refusal), not
typing's: `ValTy` admits NaN, `conforms` does not, and the invariant is
stated with `conforms`, so it says slots hold finite numbers.
-/
import Contract.Axiomatic
import Contract.Observe
import Contract.Types

namespace Contract

/-- Every root slot is a declared state or mutation, holding a value the
runtime check admits at its declared type. -/
def SlotsOK (p : Program) (slots : List (String × Value)) : Prop :=
  ∀ x v, (x, v) ∈ slots → isSlot p x = true ∧ conforms p v (slotTy p x) = true

theorem SlotsOK.nil {p : Program} : SlotsOK p [] := by simp [SlotsOK]

theorem SlotsOK.setSlot {p : Program} {slots x v} (h : SlotsOK p slots)
    (hv : conforms p v (slotTy p x) = true) : SlotsOK p (setSlot slots x v) := by
  intro y w hy
  simp only [Contract.setSlot, List.mem_map] at hy
  obtain ⟨⟨y', w'⟩, hmem, heq⟩ := hy
  by_cases hxy : (x == y') = true
  · simp only [hxy, ite_true, Prod.mk.injEq] at heq
    obtain ⟨rfl, rfl⟩ := heq
    simp only [beq_iff_eq] at hxy
    subst hxy
    exact ⟨(h _ _ hmem).1, hv⟩
  · simp only [hxy] at heq
    simp only [Bool.false_eq_true, ite_false, Prod.mk.injEq] at heq
    obtain ⟨rfl, rfl⟩ := heq
    exact h _ _ hmem

theorem SlotsOK.applyWrites {p : Program} : ∀ {slots ws : List (String × Value)}, SlotsOK p slots →
    (∀ w ∈ ws, conforms p w.2 (slotTy p w.1) = true) → SlotsOK p (Contract.applyWrites slots ws)
  | _, [], h, _ => h
  | _, (x, v) :: ws, h, hw => by
    simp only [Contract.applyWrites, List.foldl_cons]
    exact SlotsOK.applyWrites (h.setSlot (hw (x, v) (by simp))) fun w hm => hw w (by simp [hm])

/-! ## Distinct names -/

theorem distinct_append {l₁ l₂ : List String} (h : distinct (l₁ ++ l₂) = true) :
    distinct l₁ = true ∧ distinct l₂ = true ∧ ∀ x ∈ l₂, x ∉ l₁ := by
  induction l₁ with
  | nil => simp [distinct] at h ⊢; exact h
  | cons y l₁ ih =>
    simp only [List.cons_append, distinct, Bool.and_eq_true, Bool.not_eq_true',
      List.contains_eq_mem, decide_eq_false_iff_not, List.mem_append, not_or] at h
    obtain ⟨⟨hy1, hy2⟩, hr⟩ := h
    obtain ⟨h1, h2, h3⟩ := ih hr
    refine ⟨?_, h2, fun x hx => ?_⟩
    · simp only [distinct, h1, Bool.and_true, Bool.not_eq_true', List.contains_eq_mem,
        decide_eq_false_iff_not]
      exact hy1
    · simp only [List.mem_cons, not_or]
      exact ⟨fun he => hy2 (he ▸ hx), h3 x hx⟩

theorem distinct_find {α} {name : α → String} : ∀ {l : List α} {a : α}, distinct (l.map name) = true → a ∈ l →
    l.find? (fun b => name b == name a) = .some a
  | [], _, _, ha => by simp at ha
  | b :: l, a, h, ha => by
    simp only [List.map_cons, distinct, Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_mem,
      decide_eq_false_iff_not, List.mem_map, not_exists, not_and] at h
    simp only [List.mem_cons] at ha
    rcases ha with rfl | ha
    · simp
    · have hne : (name b == name a) = false := by
        simp only [beq_eq_false_iff_ne]
        intro he
        exact h.1 a ha he.symm
      simp only [List.find?_cons, hne]
      exact distinct_find h.2 ha

/-- With distinct names, a state's slot type is its declared type. -/
theorem slotTy_state {p : Program} (hd : distinct (compNames p) = true) {st : StateDecl}
    (hst : st ∈ p.states) : slotTy p st.name = st.ty := by
  have h1 := (distinct_append (l₁ := p.states.map (·.name) ++ p.derives.map (·.name) ++
    p.resources.map (·.name)) (by simpa [compNames, List.append_assoc] using hd)).1
  have h2 := (distinct_append (l₁ := p.states.map (·.name) ++ p.derives.map (·.name)) h1).1
  have h3 := (distinct_append (l₁ := p.states.map (·.name)) h2).1
  simp only [slotTy, distinct_find (name := StateDecl.name) h3 hst]

/-- With distinct names, a mutation's slot type is `option` of its shape. -/
theorem slotTy_mutation {p : Program} (hd : distinct (compNames p) = true) {m : String}
    (hm : isMutation p m = true) :
    slotTy p m = .option (((p.mutations.find? (·.name == m)).map (·.ty)).getD .unknown) ∧
      ∃ md, p.mutations.find? (·.name == m) = .some md := by
  have h := distinct_append (l₁ := p.states.map (·.name) ++ p.derives.map (·.name) ++
    p.resources.map (·.name)) (l₂ := p.mutations.map (·.name)) (by simpa [compNames, List.append_assoc] using hd)
  simp only [isMutation, List.any_eq_true, beq_iff_eq] at hm
  obtain ⟨md, hmd, rfl⟩ := hm
  have hnot := h.2.2 md.name (List.mem_map_of_mem hmd)
  have hfind : p.states.find? (·.name == md.name) = .none := by
    rw [List.find?_eq_none]
    intro st hst he
    simp only [beq_iff_eq] at he
    exact hnot (by simp only [List.mem_append, List.mem_map]; exact .inl (.inl ⟨st, hst, he⟩))
  have hmf : p.mutations.find? (·.name == md.name) = .some md :=
    distinct_find (name := MutationDecl.name) h.2.1 hmd
  simp [slotTy, hfind, hmf]

/-! ## Sends target mutations -/

/-- Every `send` in a body targets a mutation. -/
def SendsOK (p : Program) : List Stmt → Prop
  | [] => True
  | .letS _ _ :: rest | .assign _ _ :: rest | .command _ _ :: rest | .refresh _ :: rest => SendsOK p rest
  | .send x _ _ :: rest => isMutation p x = true ∧ SendsOK p rest
  | .ifS _ thn els :: rest => SendsOK p thn ∧ SendsOK p els ∧ SendsOK p rest
  | .matchS _ _ sm nn :: rest => SendsOK p sm ∧ SendsOK p nn ∧ SendsOK p rest

theorem StmtsTy.sendsOK {p : Program} {G : Scope} : ∀ {Γ : Scope} (ss : List Stmt), StmtsTy p G Γ ss → SendsOK p ss
  | _, [], _ => by simp [SendsOK]
  | _, .letS _ _ :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h; obtain ⟨_, _, hr⟩ := h; exact StmtsTy.sendsOK rest hr
  | _, .assign _ _ :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h; exact StmtsTy.sendsOK rest h.2.2
  | _, .command _ _ :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h; exact StmtsTy.sendsOK rest h.2
  | _, .refresh _ :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h; exact StmtsTy.sendsOK rest h.2
  | _, .send _ _ _ :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h; exact ⟨h.1, StmtsTy.sendsOK rest h.2.2⟩
  | _, .ifS _ thn els :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h
    exact ⟨StmtsTy.sendsOK thn h.2.1, StmtsTy.sendsOK els h.2.2.1, StmtsTy.sendsOK rest h.2.2.2⟩
  | _, .matchS _ _ sm nn :: rest, h => by
    simp only [SendsOK]
    simp only [StmtsTy] at h
    obtain ⟨⟨_, _, hsm⟩, hnn, hr⟩ := h
    exact ⟨StmtsTy.sendsOK sm hsm, StmtsTy.sendsOK nn hnn, StmtsTy.sendsOK rest hr⟩

theorem ExecR.sends {p : Program} {env ls ss fx fx'} (h : ExecR env ls ss fx fx') (hs : SendsOK p ss)
    (hfx : ∀ s ∈ fx.sends, isMutation p s.1 = true) : ∀ s ∈ fx'.sends, isMutation p s.1 = true := by
  induction h with
  | nil => exact hfx
  | letS _ _ ih => simp only [SendsOK] at hs; exact ih hs hfx
  | assignRoot _ _ _ ih => simp only [SendsOK] at hs; exact ih hs hfx
  | assignRow _ _ _ _ _ ih => simp only [SendsOK] at hs; exact ih hs hfx
  | command _ _ ih => simp only [SendsOK] at hs; exact ih hs hfx
  | refresh _ ih => simp only [SendsOK] at hs; exact ih hs hfx
  | send _ _ ih =>
    simp only [SendsOK] at hs
    refine ih hs.2 fun s hm => ?_
    simp only [Effects.send, List.mem_append, List.mem_singleton] at hm
    rcases hm with hm | rfl
    · exact hfx s hm
    · exact hs.1
  | ifTrue _ _ _ ih₁ ih₂ =>
    simp only [SendsOK] at hs; exact ih₂ hs.2.2 (ih₁ hs.1 hfx)
  | ifFalse _ _ _ ih₁ ih₂ =>
    simp only [SendsOK] at hs; exact ih₂ hs.2.2 (ih₁ hs.2.1 hfx)
  | matchSome _ _ _ ih₁ ih₂ =>
    simp only [SendsOK] at hs; exact ih₂ hs.2.2 (ih₁ hs.1 hfx)
  | matchNone _ _ _ ih₁ ih₂ =>
    simp only [SendsOK] at hs; exact ih₂ hs.2.2 (ih₁ hs.2.1 hfx)

/-! ## Commits keep the invariant -/

theorem mapM_mem {α β ε} {f : α → Except ε β} : ∀ {l : List α} {r : List β}, l.mapM f = .ok r →
    ∀ b ∈ r, ∃ a ∈ l, f a = .ok b
  | [], r, h, b, hb => by simp at h; subst h; simp at hb
  | a :: l, r, h, b, hb => by
    simp only [List.mapM_cons, Except.bind_ok_iff, Except.pure_ok_iff] at h
    obtain ⟨b', hb', rs, hrs, rfl⟩ := h
    simp only [List.mem_cons] at hb
    rcases hb with rfl | hb
    · exact ⟨a, by simp, hb'⟩
    · obtain ⟨a', ha', hf⟩ := mapM_mem hrs b hb
      exact ⟨a', by simp [ha'], hf⟩

/-- The parts of `WellTyped` the invariant uses. -/
structure SlotTyped (p : Program) : Prop where
  names : distinct (compNames p) = true
  sends : ∀ a ∈ p.actions, SendsOK p a.body

theorem WellTyped.slotTyped {p : Program} (h : WellTyped p) : SlotTyped p :=
  ⟨h.names, fun a ha => StmtsTy.sendsOK _ (h.actions a ha)⟩

/-- A step through an action keeps every root slot of its type. -/
theorem runAction_slotsOK {p : Program} {o c name args rows c' out} (hp : SlotTyped p)
    (hc : SlotsOK p c.slots) (h : runAction p o c name args rows = (c', out)) : SlotsOK p c'.slots := by
  have keep : ∀ {e}, (c, Outcome.refused e) = (c', out) → SlotsOK p c'.slots := fun h' => by
    simp only [Prod.mk.injEq] at h'; rw [← h'.1]; exact hc
  simp only [runAction] at h
  split at h
  · exact keep h
  split at h
  · exact keep h
  next a ha =>
  split at h
  · exact keep h
  split at h
  · exact keep h
  split at h
  · exact keep h
  next fx hx =>
  split at h
  · exact keep h
  next answered hans =>
  split at h
  · exact keep h
  next hw =>
  have hsends := (exec_sound hx).sends (hp.sends a (List.mem_of_find?_eq_some ha)) (by simp)
  have hok : SlotsOK p (Contract.applyWrites c.slots (answered ++ fx.writes)) := by
    refine hc.applyWrites fun w hw' => ?_
    simp only [List.mem_append] at hw'
    rcases hw' with hw' | hw'
    · obtain ⟨⟨m, src, vs⟩, hmem, hf⟩ := mapM_mem hans w hw'
      simp only [Except.bind_ok_iff] at hf
      obtain ⟨v, -, hf⟩ := hf
      split at hf
      · obtain ⟨_, h', _⟩ := Except.bind_ok_iff.mp hf; cases h'
      · next hconf =>
        simp only [Except.pure_ok_iff] at hf
        subst hf
        have hm := hsends _ hmem
        obtain ⟨hty, md, hmd⟩ := slotTy_mutation hp.names hm
        simp only [hty, conforms]
        simpa using hconf
    · simp only [Bool.not_eq_true', Bool.not_eq_false, List.all_eq_true] at hw
      exact hw w (List.mem_append_left _ hw')
  split at h
  · exact keep h
  · simp only [Prod.mk.injEq] at h; rw [← h.1]; exact hok
  · simp only [Prod.mk.injEq] at h; rw [← h.1]; exact hok

theorem dispatch_slotsOK {p : Program} {o c target event payload} (hp : SlotTyped p)
    (hc : SlotsOK p c.slots) : SlotsOK p (dispatch p o c target event payload).1.slots := by
  simp only [dispatch]
  repeat' split
  all_goals first | exact hc | exact runAction_slotsOK hp hc (Prod.mk.eta.symm.trans rfl) | skip
  all_goals exact runAction_slotsOK hp hc rfl

theorem advance_slotsOK {p : Program} {o c t} (hp : SlotTyped p)
    (hc : SlotsOK p c.slots) : SlotsOK p (advance p o c t).1.slots := by
  simp only [advance]
  split
  · exact hc
  · rename_i hlt
    clear hlt
    generalize timerFireLimit = n
    induction n generalizing c with
    | zero =>
      simp only [advance.go]
      split <;> exact hc
    | succ n ih =>
      simp only [advance.go]
      split
      · exact hc
      split
      · exact hc
      next tm i _ =>
      split
      next c' h' => exact ih (runAction_slotsOK hp (by exact hc) h')
      next c' out h1 h' => exact runAction_slotsOK hp (by exact hc) h'

/-! ## Boot -/

theorem foldlM_inv {α β ε} {f : β → α → Except ε β} (I : List α → β → Prop)
    (step : ∀ a l b b', I (a :: l) b → f b a = .ok b' → I l b') :
    ∀ {l : List α} {b r : β}, I l b → l.foldlM f b = .ok r → I [] r
  | [], b, r, hb, h => by simp at h; subst h; exact hb
  | a :: l, b, r, hb, h => by
    simp only [List.foldlM_cons, Except.bind_ok_iff] at h
    obtain ⟨b', h1, h2⟩ := h
    exact foldlM_inv I step (step a l b b' hb h1) h2

/-- The names of the late root states among `l`. -/
def lateNames (l : List StateDecl) : List String :=
  (l.filter fun s => s.late && s.owner.isNone).map (·.name)

theorem isSlot_of_state {p : Program} {st : StateDecl} (h : st ∈ p.states) : isSlot p st.name = true := by
  simp only [isSlot, Bool.or_eq_true, List.any_eq_true, beq_iff_eq]
  exact .inl ⟨st, h, rfl⟩

theorem mem_setSlot {slots : List (String × Value)} {x y : String} {w v : Value}
    (h : (x, w) ∈ setSlot slots y v) : ∃ u, (x, u) ∈ slots ∧ ((y = x ∧ w = v) ∨ (y ≠ x ∧ w = u)) := by
  simp only [setSlot, List.mem_map] at h
  obtain ⟨⟨x', u⟩, hmem, heq⟩ := h
  by_cases hyx : (y == x') = true
  · simp only [hyx, ite_true, Prod.mk.injEq] at heq
    obtain ⟨rfl, rfl⟩ := heq
    exact ⟨u, hmem, .inl ⟨by simpa using hyx, rfl⟩⟩
  · simp only [hyx, Bool.false_eq_true, ite_false, Prod.mk.injEq] at heq
    obtain ⟨rfl, rfl⟩ := heq
    exact ⟨_, hmem, .inr ⟨by simpa using hyx, rfl⟩⟩

theorem lateNames_cons {a : StateDecl} {l : List StateDecl} :
    lateNames (a :: l) = if a.late && a.owner.isNone then a.name :: lateNames l else lateNames l := by
  simp only [lateNames, List.filter_cons]
  split <;> simp

theorem lateNames_mem {l : List StateDecl} {x : String} (h : x ∈ lateNames l) :
    ∃ a ∈ l, a.name = x := by
  simp only [lateNames, List.mem_map, List.mem_filter] at h
  obtain ⟨a, ⟨ha, _⟩, rfl⟩ := h
  exact ⟨a, ha, rfl⟩

theorem boot_slotsOK {p : Program} {o} (hp : SlotTyped p) : SlotsOK p (boot p o).1.slots := by
  -- After the boot initializers: every slot conforms, or is a late slot
  -- holding `()` until settlement.
  let I₁ : List StateDecl → List (String × Value) → Prop := fun rest slots =>
    (∀ a ∈ rest, a ∈ p.states) ∧ ∀ x w, (x, w) ∈ slots → isSlot p x = true ∧
      (conforms p w (slotTy p x) = true ∨ x ∈ lateNames p.states)
  -- During the late initializers: every slot conforms, or is a late slot
  -- still to come.
  let I₂ : List StateDecl → List (String × Value) → Prop := fun rest slots =>
    (∀ a ∈ rest, a ∈ p.states) ∧ ∀ x w, (x, w) ∈ slots → isSlot p x = true ∧
      (conforms p w (slotTy p x) = true ∨ x ∈ lateNames rest)
  simp only [boot]
  repeat' split
  all_goals first | exact SlotsOK.nil | skip
  rename_i _ _ init hinit _ _ _ _ _ _ _ _ late hlate _ _ _ _
  have h₁ : I₁ [] init := by
    refine foldlM_inv I₁ ?_ (l := p.states) (b := []) ⟨fun a ha => ha, by simp⟩ hinit
    intro a l b b' ⟨hl, hb⟩ hf
    have ha : a ∈ p.states := hl a (by simp)
    refine ⟨fun a' ha' => hl a' (by simp [ha']), ?_⟩
    split at hf
    · simp only [Except.pure_ok_iff] at hf; subst hf; exact hb
    split at hf
    · next hlate =>
      simp only [Except.pure_ok_iff] at hf; subst hf
      intro x w hx
      simp only [List.mem_append, List.mem_singleton, Prod.mk.injEq] at hx
      rcases hx with hx | ⟨rfl, rfl⟩
      · exact hb x w hx
      · next hown =>
        refine ⟨isSlot_of_state ha, .inr ?_⟩
        simp only [lateNames, List.mem_map, List.mem_filter, Bool.and_eq_true]
        exact ⟨a, ⟨ha, hlate, by simpa using hown⟩, rfl⟩
    · simp only [Except.bind_ok_iff] at hf
      obtain ⟨v, -, hf⟩ := hf
      split at hf
      · obtain ⟨_, h', _⟩ := Except.bind_ok_iff.mp hf; cases h'
      · next hconf =>
        simp only [Except.pure_ok_iff] at hf; subst hf
        intro x w hx
        simp only [List.mem_append, List.mem_singleton, Prod.mk.injEq] at hx
        rcases hx with hx | ⟨rfl, rfl⟩
        · exact hb x w hx
        · refine ⟨isSlot_of_state ha, .inl ?_⟩
          rw [slotTy_state hp.names ha]
          simpa using hconf
  have h₂ : I₂ [] late := by
    refine foldlM_inv I₂ ?_ (l := p.states) ⟨fun a ha => ha, fun x w hx => ?_⟩ hlate
    · intro a l b b' ⟨hl, hb⟩ hf
      have ha : a ∈ p.states := hl a (by simp)
      refine ⟨fun a' ha' => hl a' (by simp [ha']), ?_⟩
      split at hf
      · next hskip =>
        simp only [Except.pure_ok_iff] at hf; subst hf
        intro x w hx
        obtain ⟨hs, hc⟩ := hb x w hx
        refine ⟨hs, ?_⟩
        rw [lateNames_cons] at hc
        have : (a.late && a.owner.isNone) = false := by
          cases hl' : a.late <;> cases ho : a.owner <;> simp_all
        simpa [this] using hc
      · next hset =>
        simp only [Except.bind_ok_iff] at hf
        obtain ⟨v, -, hf⟩ := hf
        split at hf
        · obtain ⟨_, h', _⟩ := Except.bind_ok_iff.mp hf; cases h'
        · next hconf =>
          simp only [Except.pure_ok_iff] at hf; subst hf
          intro x w hx
          obtain ⟨u, hu, hcase⟩ := mem_setSlot hx
          obtain ⟨hs, hc⟩ := hb x u hu
          refine ⟨hs, ?_⟩
          rcases hcase with ⟨rfl, rfl⟩ | ⟨hne, rfl⟩
          · left; rw [slotTy_state hp.names ha]; simpa using hconf
          · rcases hc with hc | hc
            · exact .inl hc
            · right
              rw [lateNames_cons] at hc
              have : (a.late && a.owner.isNone) = true := by
                cases hl' : a.late <;> cases ho : a.owner <;> simp_all
              simp only [this, ite_true, List.mem_cons] at hc
              rcases hc with hc | hc
              · exact absurd hc.symm hne
              · exact hc
    · simp only [List.mem_append, List.mem_map] at hx
      rcases hx with hx | ⟨m, hm, heq⟩
      · exact h₁.2 x w hx
      · simp only [Prod.mk.injEq] at heq
        obtain ⟨rfl, rfl⟩ := heq
        have hmut : isMutation p m.name = true := by
          simp only [isMutation, List.any_eq_true, beq_iff_eq]; exact ⟨m, hm, rfl⟩
        refine ⟨by simp [isSlot, hmut], .inl ?_⟩
        rw [(slotTy_mutation hp.names hmut).1]
        simp [conforms]
  intro x w hx
  obtain ⟨hs, hc⟩ := h₂.2 x w hx
  refine ⟨hs, ?_⟩
  rcases hc with hc | hc
  · exact hc
  · simp [lateNames] at hc

/-! ## Every reachable configuration -/

/-- The configurations a program reaches: boot, then any events. -/
inductive Reachable (p : Program) (o : Oracle) : Config → Prop
  | boot : Reachable p o (boot p o).1
  | step : Reachable p o c → Reachable p o (Observe.step p o c ev).1

theorem step_slotsOK {p : Program} {o c} (hp : SlotTyped p) (hc : SlotsOK p c.slots) (ev : Observe.Event) :
    SlotsOK p (Observe.step p o c ev).1.slots := by
  cases ev <;> simp only [Observe.step]
  all_goals first | exact dispatch_slotsOK hp hc | exact advance_slotsOK hp hc

/-- **The slot invariant.** In every configuration a well-typed program
reaches, each root slot is a declared state or mutation holding a value
the runtime check admits at its declared type (`conforms`: of the type,
numbers finite). -/
theorem reachable_slotsOK {p : Program} {o c} (hp : WellTyped p) (h : Reachable p o c) : SlotsOK p c.slots := by
  induction h with
  | boot => exact boot_slotsOK hp.slotTyped
  | step _ ih => exact step_slotsOK hp.slotTyped ih _

/-- The same as types: each root slot holds a value of its declared type. -/
theorem reachable_valTy {p : Program} {o c} (hp : WellTyped p) (h : Reachable p o c) :
    ∀ x v, (x, v) ∈ c.slots → ValTy p v (slotTy p x) := by
  intro x v hx
  obtain ⟨hs, hc⟩ := reachable_slotsOK hp h x v hx
  refine conforms_valTy hp.shapes v hc ?_
  simp only [isSlot, Bool.or_eq_true, List.any_eq_true, beq_iff_eq] at hs
  rcases hs with ⟨st, hst, rfl⟩ | hm
  · rw [slotTy_state hp.names hst]; exact hp.states st hst
  · have hm' : isMutation p x = true := by simpa [isMutation] using hm
    obtain ⟨hty, md, hmd⟩ := slotTy_mutation hp.names hm'
    rw [hty, hmd]
    exact hp.mutations md (List.mem_of_find?_eq_some hmd)

end Contract
