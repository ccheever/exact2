/-
Invariants of whole runs: what holds of every configuration a program can
reach, from boot, by any sequence of host events.

The laws in `Contract.Axiomatic` are about one action. This file lifts
them to runs. A host can do only two things to a configuration: deliver an
event to an element it shows (`dispatch`) or move the clock (`advance`).
The first runs the action of a handler the view rendered, the second the
actions of timers boot started; neither changes anything except through
`runAction`. So an invariant of a program is proved by
`Reachable.invariant`: it holds after `boot`, every `runAction` of a
handler the program's view declares preserves it, every `runAction` of a
task's action does, and moving the clock does.

For a property of one slot (the common case), `Reachable.slotIn` reduces
those obligations to the values the slot can start with (`SlotOrigin`)
and to the action bodies (`BodyKeeps`): a body that never assigns or sends
into the slot keeps it (`Stmt.noAssigns`, `Stmt.noSends`, decided by
`decide`), and one that does is proved with the Hoare logic (`wp`,
`BodyKeeps.of_wp`) against `actionEnv`. `Event.step_slotIn` is the same
for a property every action keeps, from any configuration;
`Reachable.advance_untouched` says what the clock cannot change.

Nothing here needs `conforms`, `Value.equal` or `Value.same`: a step that
refuses keeps the configuration, so only what a commit writes matters, and
`runAction_commit` says that. (`Contract.TypeInvariant` is the invariant
that does read `conforms`: every slot of its declared type.)
-/
import Contract.Axiomatic

namespace Contract

/-! ## Where handlers come from -/

/-- `n` stands somewhere in the tree `vs` (in preorder, at any depth). -/
inductive VNode.In : VNode → List VNode → Prop
  | head : VNode.In n (n :: rest)
  | child : VNode.In n m.children → VNode.In n (m :: rest)
  | tail : VNode.In n rest → VNode.In n (m :: rest)

theorem findTestId_in {id : String} :
    ∀ {vs : List VNode} {m}, findTestId id vs = .some m → VNode.In m vs
  | [], _, h => by simp [findTestId] at h
  | ⟨_, _, _, _, _, _, _, _, cs⟩ :: rest, m, h => by
    rw [findTestId] at h
    split at h
    · cases h; exact .head
    · split at h
      next m' hc => cases h; exact .child (findTestId_in hc)
      next => exact .tail (findTestId_in h)

mutual
/-- The handlers an element of the view declares, at any depth: every
event a rendered element can carry is one of these. -/
def Node.handlerList : Node → List (String × String × List Expr)
  | .element _ _ _ hs cs => hs ++ Node.handlerLists cs
  | .when _ _ a b => Node.handlerLists a ++ Node.handlerLists b
  | .each _ _ _ _ _ body => Node.handlerLists body
  | .matchN _ _ _ a b => Node.handlerLists a ++ Node.handlerLists b
def Node.handlerLists : List Node → List (String × String × List Expr)
  | [] => []
  | n :: ns => n.handlerList ++ Node.handlerLists ns
end

/-- Every element in `vs`, at any depth, carries only handlers in `H`. -/
def HandlersFrom (H : List (String × String × List Expr)) (vs : List VNode) : Prop :=
  ∀ n, VNode.In n vs → ∀ h ∈ n.handlers, h ∈ H

theorem HandlersFrom.nil {H} : HandlersFrom H [] := fun _ h => nomatch h

theorem HandlersFrom.mono {H H' vs} (h : HandlersFrom H vs) (hs : ∀ x ∈ H, x ∈ H') :
    HandlersFrom H' vs := fun n hn x hx => hs _ (h n hn x hx)

theorem VNode.In.append {n} : ∀ {a b : List VNode}, VNode.In n (a ++ b) → VNode.In n a ∨ VNode.In n b
  | [], _, h => .inr h
  | _ :: _, _, .head => .inl .head
  | _ :: _, _, .child h => .inl (.child h)
  | _ :: a, b, .tail h => (VNode.In.append (a := a) (b := b) h).elim (fun h => .inl (.tail h)) .inr

theorem HandlersFrom.append {H a b} (ha : HandlersFrom H a) (hb : HandlersFrom H b) :
    HandlersFrom H (a ++ b) := fun n hn => (VNode.In.append hn).elim (ha n) (hb n)

theorem HandlersFrom.single {H} {v : VNode} (hv : ∀ x ∈ v.handlers, x ∈ H)
    (hc : HandlersFrom H v.children) : HandlersFrom H [v] := fun n hn => by
  cases hn with
  | head => exact hv
  | child h => exact hc n h
  | tail h => exact nomatch h

theorem handlerLists_append {a b : List Node} :
    Node.handlerLists (a ++ b) = Node.handlerLists a ++ Node.handlerLists b := by
  induction a with
  | nil => simp [Node.handlerLists]
  | cons n ns ih => simp [Node.handlerLists, ih]

/-- A rendered element carries only handlers its view declares. -/
theorem render_handlers : ∀ fuel,
    (∀ {cx ls nodes live vs live'}, render fuel cx ls nodes live = .ok (vs, live') →
      HandlersFrom (Node.handlerLists nodes) vs) ∧
    (∀ {cx ls tag arm body live vs live'}, render.renderArm fuel cx ls tag arm body live = .ok (vs, live') →
      HandlersFrom (Node.handlerLists body) vs) ∧
    (∀ {cx ls tag x ix key body items i seen live vs live'},
      render.renderRows fuel cx ls tag x ix key body items i seen live = .ok (vs, live') →
      HandlersFrom (Node.handlerLists body) vs)
  | 0 => by
    refine ⟨?_, ?_, ?_⟩ <;> intros <;> simp_all [render, render.renderArm, render.renderRows]
  | n + 1 => by
    obtain ⟨ihR, ihA, ihW⟩ := render_handlers n
    refine ⟨?_, ?_, ?_⟩
    · intro cx ls nodes live vs live' h
      cases nodes with
      | nil => simp [render] at h; rw [h.1]; exact .nil
      | cons node rest =>
        unfold render at h
        simp only [Except.bind_ok_iff] at h
        obtain ⟨⟨here, l1⟩, hx, ⟨more, l2⟩, hy, h⟩ := h
        simp only [Except.pure_ok_iff, Prod.mk.injEq] at h
        obtain ⟨rfl, -⟩ := h
        simp only [Node.handlerLists]
        refine HandlersFrom.append ?_ ((ihR hy).mono fun _ h => List.mem_append_right _ h)
        refine HandlersFrom.mono (H := node.handlerList) ?_ fun _ h => List.mem_append_left _ h
        cases node with
        | element tag pos props hs children =>
          simp only [Except.bind_ok_iff, Except.pure_ok_iff] at hx
          obtain ⟨_, _, _, _, ⟨kids, l3⟩, hk, hx⟩ := hx
          simp only [Prod.mk.injEq] at hx
          obtain ⟨rfl, -⟩ := hx
          simp only [Node.handlerList]
          exact .single (fun _ h => List.mem_append_left _ h)
            ((ihR hk).mono fun _ h => List.mem_append_right _ h)
        | when tag c thn els =>
          simp only [Except.bind_ok_iff] at hx
          obtain ⟨v, _, hx⟩ := hx
          simp only [Node.handlerList]
          split at hx
          · exact (ihA hx).mono fun _ h => List.mem_append_left _ h
          · exact (ihA hx).mono fun _ h => List.mem_append_right _ h
          · simp at hx
        | matchN tag sub x sm nn =>
          simp only [Except.bind_ok_iff] at hx
          obtain ⟨v, _, hx⟩ := hx
          simp only [Node.handlerList]
          split at hx
          · exact (ihA hx).mono fun _ h => List.mem_append_left _ h
          · exact (ihA hx).mono fun _ h => List.mem_append_right _ h
          · simp at hx
        | each tag x ix list key body =>
          simp only [Except.bind_ok_iff] at hx
          obtain ⟨_, _, _, _, hx⟩ := hx
          simp only [Node.handlerList]
          exact ihW hx
    · intro cx ls tag arm body live vs live' h
      unfold render.renderArm at h
      simp only [Except.bind_ok_iff] at h
      obtain ⟨_, _, h⟩ := h
      exact ihR h
    · intro cx ls tag x ix key body items i seen live vs live' h
      cases items with
      | nil => unfold render.renderRows at h; simp at h; rw [h.1]; exact .nil
      | cons item items =>
        unfold render.renderRows at h
        simp only [Except.bind_ok_iff] at h
        obtain ⟨_, _, _, _, _, _, ⟨vs1, l1⟩, h1, ⟨vs2, l2⟩, h2, h⟩ := h
        simp only [Except.pure_ok_iff, Prod.mk.injEq] at h
        obtain ⟨rfl, -⟩ := h
        exact (ihR h1).append (ihW h2)

/-! ## What a step can change -/

theorem update_render {p o slots store now prev force st r}
    (h : update p o slots store now prev force = .ok (st, r)) :
    ∃ cx, render fuel cx [] p.view [] = r := by
  simp only [update, Except.bind_ok_iff, Except.ok_ok_iff, Prod.mk.injEq] at h
  obtain ⟨_, _, -, rfl⟩ := h
  exact ⟨_, rfl⟩

/-- An action keeps the timers, and its view is the old one or the
program's view rendered. -/
theorem runAction_frame {p o c name args rows c' out}
    (h : runAction p o c name args rows = (c', out)) :
    c'.timers = c.timers ∧
      (c'.view = c.view ∨ ∃ cx live, render fuel cx [] p.view [] = .ok (c'.view, live)) := by
  simp only [runAction] at h
  repeat' split at h
  all_goals first
    | (simp only [Prod.mk.injEq] at h; obtain ⟨rfl, -⟩ := h; exact ⟨rfl, .inl rfl⟩)
    | (simp only [Prod.mk.injEq] at h; obtain ⟨rfl, -⟩ := h
       obtain ⟨cx, hr⟩ := update_render (by assumption)
       exact ⟨rfl, .inr ⟨cx, _, hr⟩⟩)

/-- A dispatch either refuses with the configuration unchanged, or runs
the action of a handler on an element the view shows: the handler's
curried arguments evaluated in the element's scope, then the payload. -/
theorem dispatch_cases {p o c target event payload c' out}
    (h : dispatch p o c target event payload = (c', out)) :
    c' = c ∨ ∃ n ev a args env vs, VNode.In n c.view ∧ (ev, a, args) ∈ n.handlers ∧
      ListR env false n.locals args vs ∧ runAction p o c a (vs ++ payload.toList) n.rows = (c', out) := by
  simp only [dispatch] at h
  split at h
  · exact .inl (Prod.mk.inj h).1.symm
  split at h
  · exact .inl (Prod.mk.inj h).1.symm
  next n hn =>
  split at h
  · exact .inl (Prod.mk.inj h).1.symm
  next ev a args hh =>
  split at h
  · exact .inl (Prod.mk.inj h).1.symm
  split at h
  · exact .inl (Prod.mk.inj h).1.symm
  next vs hvs =>
  exact .inr ⟨n, ev, a, args, _, vs, findTestId_in hn, List.mem_of_find?_eq_some hh,
    evalList_sound hvs, h⟩

/-- `advance`'s loop, for any choice of the due timer that picks one of
the configuration's timers, any choice of the `then` that runs an action
of `T`, and any way of finishing that keeps `J`. -/
theorem advance_go {p o due pick finish} (J : Config → Prop) (A T : List String)
    (hdue : ∀ c tm i, due c = Option.some (tm, i) → tm ∈ c.timers)
    (hpick : ∀ c m w a, pick c = Option.some (m, w, a) → a ∈ T)
    (hfinish : ∀ c, J c → (∀ tm ∈ c.timers, tm.action ∈ A) → J (finish c).1 ∧
      ∀ tm ∈ (finish c).1.timers, tm.action ∈ A)
    (hclock : ∀ (c : Config) ts t ar, J c → (∀ tm ∈ ts, tm.action ∈ A) →
      J { c with timers := ts, now := t, armed := ar })
    (hrun : ∀ c c' out a, J c → (a ∈ A ∨ a ∈ T) → runAction p o c a [] [] = (c', out) → J c') :
    ∀ n c, J c → (∀ tm ∈ c.timers, tm.action ∈ A) →
      J (advance.go p o due pick finish n c).1 ∧
        ∀ tm ∈ (advance.go p o due pick finish n c).1.timers, tm.action ∈ A
  | 0, c, hJ, hA => by
    rw [advance.go]
    split
    · exact hfinish c hJ hA
    · exact ⟨hJ, hA⟩
  | n + 1, c, hJ, hA => by
    rw [advance.go]
    split
    · exact ⟨hJ, hA⟩
    split
    next m w a hp =>
      dsimp only
      generalize hr : runAction p o
        { c with armed := c.armed.filter (·.1 != m), now := if c.now < w then w else c.now }
        a [] [] = r
      have hJ' := hclock c c.timers (if c.now < w then w else c.now) (c.armed.filter (·.1 != m)) hJ hA
      obtain ⟨c₂, out₂⟩ := r
      have hf := (runAction_frame hr).1
      have hJ₂ := hrun _ _ _ _ hJ' (.inr (hpick c m w a hp)) hr
      have hA₂ : ∀ y ∈ c₂.timers, y.action ∈ A := by rw [hf]; exact hA
      cases out₂ with
      | ok => exact advance_go J A T hdue hpick hfinish hclock hrun n c₂ hJ₂ hA₂
      | refused => exact ⟨hJ₂, hA₂⟩
      | poisoned => exact ⟨hJ₂, hA₂⟩
    split
    · exact hfinish c hJ hA
    next tm i hd =>
    have htm := hA tm (hdue c tm i hd)
    have hset : ∀ y ∈ c.timers.set i tm.fired, y.action ∈ A := fun y hy => by
      rcases List.mem_or_eq_of_mem_set hy with hy | rfl
      · exact hA y hy
      · unfold Timer.fired; split <;> (try split) <;> exact htm
    dsimp only
    generalize hr : runAction p o { c with timers := c.timers.set i tm.fired, now := tm.next }
      tm.action [] [] = r
    have hJ' := hclock c _ tm.next c.armed hJ hset
    obtain ⟨c₂, out₂⟩ := r
    have hf := (runAction_frame hr).1
    have hJ₂ := hrun _ _ _ _ hJ' (.inl htm) hr
    have hA₂ : ∀ y ∈ c₂.timers, y.action ∈ A := by rw [hf]; exact hset
    cases out₂ with
    | ok => exact advance_go J A T hdue hpick hfinish hclock hrun n c₂ hJ₂ hA₂
    | refused => exact ⟨hJ₂, hA₂⟩
    | poisoned => exact ⟨hJ₂, hA₂⟩

theorem advance_go_eq {p o due pick finish} (J : Config → Prop) (A T : List String)
    (hdue : ∀ c tm i, due c = Option.some (tm, i) → tm ∈ c.timers)
    (hpick : ∀ c m w a, pick c = Option.some (m, w, a) → a ∈ T)
    (hfinish : ∀ c, J c → (∀ tm ∈ c.timers, tm.action ∈ A) → J (finish c).1 ∧
      ∀ tm ∈ (finish c).1.timers, tm.action ∈ A)
    (hclock : ∀ (c : Config) ts t ar, J c → (∀ tm ∈ ts, tm.action ∈ A) →
      J { c with timers := ts, now := t, armed := ar })
    (hrun : ∀ c c' out a, J c → (a ∈ A ∨ a ∈ T) → runAction p o c a [] [] = (c', out) → J c')
    {n c c' out} (hJ : J c) (hA : ∀ tm ∈ c.timers, tm.action ∈ A)
    (h : advance.go p o due pick finish n c = (c', out)) : J c' ∧ ∀ tm ∈ c'.timers, tm.action ∈ A := by
  have := advance_go J A T hdue hpick hfinish hclock hrun n c hJ hA
  rw [h] at this; exact this

theorem foldl_pick{α : Type} {f : Option α → α → Option α} (hf : ∀ b x, f b x = b ∨ f b x = .some x) :
    ∀ {l : List α} {init r}, l.foldl f init = .some r → init = .some r ∨ r ∈ l
  | [], _, _, h => .inl h
  | x :: l, init, r, h => by
    rcases foldl_pick hf (l := l) h with h | h
    · rcases hf init x with h' | h'
      · exact .inl (h'.symm.trans h)
      · rw [h'] at h; cases h; exact .inr (List.mem_cons_self ..)
    · exact .inr (List.mem_cons_of_mem _ h)

theorem foldl_inv {α β : Type} {f : β → α → β} (P : β → Prop) :
    ∀ (l : List α) (init : β), P init → (∀ b x, x ∈ l → P b → P (f b x)) → P (l.foldl f init)
  | [], _, h, _ => h
  | x :: l, init, h, hf =>
    foldl_inv P l (f init x) (hf init x (List.mem_cons_self ..) h)
      fun b y hy => hf b y (List.mem_cons_of_mem _ hy)

/-- The `then` an advance runs is a mutation's. -/
theorem dueThen_mem {p : Program} {armed t m w a} (h : dueThen p armed t = .some (m, w, a)) :
    a ∈ thenActions p := by
  unfold dueThen at h
  refine foldl_inv
    (fun (b : Option (String × F64 × String)) => ∀ r, b = .some r → r.2.2 ∈ thenActions p)
    p.mutations Option.none (fun _ h => nomatch h) ?_ (m, w, a) h
  intro b x hx hb r hr
  split at hr
  next a' _ _ ha' _ =>
    have hmem : a' ∈ thenActions p := List.mem_filterMap.mpr ⟨x, hx, ha'⟩
    split at hr
    · split at hr
      · cases hr; exact hmem
      · split at hr
        · cases hr; exact hmem
        · exact hb r hr
    · exact hb r hr
  · exact hb r hr

/-- Moving the clock only runs timers' actions and armed `then`s: an
invariant that every such action preserves, and that does not depend on
the timers, the clock or what is armed, holds after an advance. -/
theorem advance_preserves {p o} (I : Config → Prop) (A : List String)
    (hclock : ∀ (c : Config) ts t ar, I c → I { c with timers := ts, now := t, armed := ar })
    (hrun : ∀ c c' out a, I c → (a ∈ A ∨ a ∈ thenActions p) → runAction p o c a [] [] = (c', out) → I c')
    {c t c' out} (hI : I c) (hA : ∀ tm ∈ c.timers, tm.action ∈ A)
    (h : advance p o c t = (c', out)) : I c' ∧ ∀ tm ∈ c'.timers, tm.action ∈ A := by
  simp only [advance] at h
  split at h
  · cases h; exact ⟨hI, hA⟩
  refine advance_go_eq I A (thenActions p) ?_ ?_ ?_ (fun c ts t ar hc _ => hclock c ts t ar hc) hrun hI hA h
  · intro c tm i hd
    rcases foldl_pick (by
        intro b x
        obtain ⟨xt, xi⟩ := x
        dsimp only
        split <;> (try split) <;> (try split) <;> simp) hd with h | h
    · cases h
    · exact List.mem_zipIdx h |>.2.2 ▸ List.getElem_mem _
  · intro c m w a hp
    split at hp
    · split at hp
      · cases hp; exact dueThen_mem ‹_›
      · cases hp
    · exact dueThen_mem hp
  · intro c hc ha
    exact ⟨hclock c c.timers _ c.armed hc, ha⟩

/-! ## Boot -/

@[simp] theorem Except.map_throw_ok {ε α β} {f : α → β} {e : ε} {b : β} :
    (f <$> (throw e : Except ε α)) = .ok b ↔ False :=
  ⟨fun h => (nomatch h), False.elim⟩

/-- A boot is refused with the empty configuration, or it initialized the
slots, started the timers and rendered the program's view. -/
theorem boot_cases {p o c out} (h : boot p o = (c, out)) :
    c = Config.empty ∨ ∃ slots₀ timers st live, initSlots p = .ok slots₀ ∧
      startTimers p slots₀ = .ok timers ∧ lateSlots p st slots₀ = .ok c.slots ∧
      (∃ cx, render fuel cx [] p.view [] = .ok (c.view, live)) ∧ c.timers = timers := by
  simp only [boot] at h
  repeat' split at h
  all_goals first
    | exact .inl (Prod.mk.inj h).1.symm
    | (simp only [Prod.mk.injEq] at h; obtain ⟨rfl, -⟩ := h
       exact .inr ⟨_, _, _, _, by assumption, by assumption, by assumption, ⟨_, by assumption⟩, rfl⟩)

theorem foldlM_inv {α β ε : Type} {f : β → α → Except ε β} (P : β → Prop) :
    ∀ {l : List α} {init r}, (∀ b a b', a ∈ l → P b → f b a = .ok b' → P b') →
      P init → l.foldlM f init = .ok r → P r
  | [], _, _, _, hi, h => by simp at h; exact h ▸ hi
  | a :: l, init, r, hf, hi, h => by
    simp only [List.foldlM_cons, Except.bind_ok_iff] at h
    obtain ⟨b, hb, h⟩ := h
    exact foldlM_inv P (fun b a b' ha => hf b a b' (List.mem_cons_of_mem _ ha))
      (hf _ _ _ (List.mem_cons_self ..) hi hb) h

theorem mapM_mem {α β ε : Type} {f : α → Except ε β} :
    ∀ {l : List α} {r}, l.mapM f = .ok r → ∀ y ∈ r, ∃ x ∈ l, f x = .ok y
  | [], r, h, y, hy => by simp at h; subst h; cases hy
  | a :: l, r, h, y, hy => by
    simp only [List.mapM_cons, Except.bind_ok_iff, Except.pure_ok_iff] at h
    obtain ⟨b, hb, bs, hbs, rfl⟩ := h
    rcases List.mem_cons.mp hy with rfl | hy
    · exact ⟨a, List.mem_cons_self .., hb⟩
    · obtain ⟨x, hx, hfx⟩ := mapM_mem hbs y hy
      exact ⟨x, List.mem_cons_of_mem _ hx, hfx⟩

/-- Every timer runs a task's action. -/
theorem startTimers_actions {p slots timers} (h : startTimers p slots = .ok timers) :
    ∀ tm ∈ timers, tm.action ∈ p.tasks.map (·.action) := by
  intro tm htm
  obtain ⟨t, ht, hf⟩ := mapM_mem h tm htm
  split at hf
  · simp only [Except.pure_ok_iff] at hf
    subst hf; exact List.mem_map_of_mem ht
  · simp only [Except.bind_ok_iff, Except.pure_ok_iff] at hf
    obtain ⟨_, _, _, _, rfl⟩ := hf
    exact List.mem_map_of_mem ht

/-- Where a slot's value at boot comes from: a root state's initializer
(or `()`, for a late slot before boot settlement), or a mutation, which
starts as `none`, or the router slot, which starts at the launch of `/`. -/
def SlotOrigin (p : Program) (x : String) (v : Value) : Prop :=
  (∃ st ∈ p.states, st.name = x ∧ st.owner = .none ∧
    ((st.late = true ∧ v = .unit) ∨ ∃ env, EvalR env false [] st.init v)) ∨
  (∃ m ∈ p.mutations, m.name = x ∧ v = .none) ∨
  (p.router = .some x ∧ ∃ r, Route.launch p.routes "/" = .ok r ∧ v = Route.routerValue p.routes r)

theorem mem_setSlot {x y : String} {v w : Value} :
    ∀ {s : List (String × Value)}, (x, w) ∈ setSlot s y v → (x, w) ∈ s ∨ (x = y ∧ w = v)
  | [], h => by simp [setSlot] at h
  | (z, u) :: s, h => by
    simp only [setSlot, List.map_cons, List.mem_cons] at h
    rcases h with h | h
    · by_cases hyz : y = z
      · simp only [hyz, BEq.rfl, ite_true, Prod.mk.injEq] at h
        exact .inr ⟨h.1.trans hyz.symm, h.2⟩
      · have : (y == z) = false := by simpa using hyz
        simp only [this, Bool.false_eq_true, ite_false] at h
        exact .inl (h ▸ List.mem_cons_self ..)
    · rcases mem_setSlot (s := s) h with h | h
      · exact .inl (List.mem_cons_of_mem _ h)
      · exact .inr h

/-- Every slot boot makes has a `SlotOrigin`. -/
theorem boot_slots_origin {p o c out} (h : boot p o = (c, out)) :
    ∀ x v, (x, v) ∈ c.slots → SlotOrigin p x v := by
  rcases boot_cases h with rfl | ⟨s₀, _, st, _, hi, -, hl, -, -⟩
  · intro x v hx; cases hx
  have h₀ : ∀ x v, (x, v) ∈ s₀ → SlotOrigin p x v := by
    simp only [initSlots, Except.bind_ok_iff, Except.pure_ok_iff] at hi
    obtain ⟨s, hs, rfl⟩ := hi
    have : ∀ x v, (x, v) ∈ s → SlotOrigin p x v := by
      refine foldlM_inv (fun s => ∀ x v, (x, v) ∈ s → SlotOrigin p x v) ?_ (by simp) hs
      intro b a b' ha hb hf
      split at hf
      · simp only [Except.pure_ok_iff] at hf; exact hf ▸ hb
      split at hf
      · simp only [Except.pure_ok_iff] at hf
        subst hf
        intro x v hx
        rcases List.mem_append.mp hx with hx | hx
        · exact hb x v hx
        · simp only [List.mem_singleton, Prod.mk.injEq] at hx
          obtain ⟨rfl, rfl⟩ := hx
          exact .inl ⟨a, ha, rfl, by simpa using ‹¬a.owner.isSome = true›, .inl ⟨‹_›, rfl⟩⟩
      split at hf
      · split at hf
        · simp only [Except.pure_ok_iff] at hf
          subst hf
          intro x v hx
          rcases List.mem_append.mp hx with hx | hx
          · exact hb x v hx
          · simp only [List.mem_singleton, Prod.mk.injEq] at hx
            obtain ⟨rfl, rfl⟩ := hx
            exact .inr (.inr ⟨by simpa using ‹(p.router == Option.some a.name) = true›, _, ‹_›, rfl⟩)
        · simp [throw, throwThe, MonadExceptOf.throw] at hf
      simp only [Except.bind_ok_iff] at hf
      obtain ⟨w, hw, hf⟩ := hf
      split at hf
      · simp at hf
      simp only [Except.pure_ok_iff] at hf
      subst hf
      intro x v hx
      rcases List.mem_append.mp hx with hx | hx
      · exact hb x v hx
      · simp only [List.mem_singleton, Prod.mk.injEq] at hx
        obtain ⟨rfl, rfl⟩ := hx
        exact .inl ⟨a, ha, rfl, by simpa using ‹¬a.owner.isSome = true›, .inr ⟨_, eval_sound hw⟩⟩
    intro x v hx
    rcases List.mem_append.mp hx with hx | hx
    · exact this x v hx
    · obtain ⟨m, hm, he⟩ := List.mem_map.mp hx
      simp only [Prod.mk.injEq] at he
      exact .inr (.inl ⟨m, hm, he.1, he.2.symm⟩)
  refine foldlM_inv (fun s => ∀ x v, (x, v) ∈ s → SlotOrigin p x v) ?_ h₀ hl
  intro b a b' ha hb hf
  simp only at hf
  split at hf
  · simp only [Except.pure_ok_iff] at hf; exact hf ▸ hb
  simp only [Except.bind_ok_iff] at hf
  obtain ⟨w, hw, hf⟩ := hf
  split at hf
  · simp at hf
  simp only [Except.pure_ok_iff] at hf
  subst hf
  intro x v hx
  rcases mem_setSlot hx with hx | ⟨rfl, rfl⟩
  · exact hb x v hx
  · have hown : a.owner = .none := by
      cases h' : a.owner <;> simp_all
    exact .inl ⟨a, ha, rfl, hown, .inr ⟨_, eval_sound hw⟩⟩

/-! ## Runs -/

/-- What a host can do to a configuration. -/
inductive Event where
  | dispatch (target event : String) (payload : Option Value)
  | advance (t : F64)

def Event.step (p : Program) (o : Oracle) (c : Config) : Event → Config × Outcome
  | .dispatch target event payload => Contract.dispatch p o c target event payload
  | .advance t => Contract.advance p o c t

/-- The configurations a program reaches: its boot, then any sequence of
events. Each step may meet a different oracle: the sources need not answer
the same question the same way twice, so what holds of every reachable
configuration holds whatever the data sources do. -/
inductive Reachable (p : Program) : Config → Prop
  | boot (o : Oracle) : Reachable p (Contract.boot p o).1
  | step (o : Oracle) (ev : Event) : Reachable p c → Reachable p (ev.step p o c).1

/-- The actions the clock runs: the tasks' and the mutations' `then`s. -/
def clockActions (p : Program) : List String := p.tasks.map (·.action) ++ thenActions p

/-- The frame every reachable configuration has: its elements carry only
handlers the view declares, and its timers run only tasks' actions. -/
def Framed (p : Program) (c : Config) : Prop :=
  HandlersFrom (Node.handlerLists p.view) c.view ∧ ∀ tm ∈ c.timers, tm.action ∈ p.tasks.map (·.action)

theorem runAction_framed {p o c name args rows c' out} (hf : Framed p c)
    (h : runAction p o c name args rows = (c', out)) : Framed p c' := by
  obtain ⟨ht, hv⟩ := runAction_frame h
  refine ⟨?_, ht ▸ hf.2⟩
  rcases hv with hv | ⟨cx, live, hr⟩
  · exact hv ▸ hf.1
  · exact (render_handlers fuel).1 hr

/-- `Reachable.invariant`, with the frame carried along. -/
theorem Reachable.invariant_framed {p : Program} (I : Config → Prop)
    (hboot : ∀ o, I (Contract.boot p o).1)
    (hclock : ∀ (c : Config) ts t ar, I c → I { c with timers := ts, now := t, armed := ar })
    (hhandler : ∀ o c c' out ev a args env ls vs payload rows, I c →
      (ev, a, args) ∈ Node.handlerLists p.view → ListR env false ls args vs →
      runAction p o c a (vs ++ payload) rows = (c', out) → I c')
    (htask : ∀ o c c' out, ∀ a ∈ clockActions p, I c →
      runAction p o c a [] [] = (c', out) → I c') :
    ∀ c, Reachable p c → I c ∧ Framed p c := by
  intro c h
  induction h with
  | boot o =>
    refine ⟨hboot o, ?_⟩
    generalize hb : Contract.boot p o = r
    obtain ⟨c, out⟩ := r
    rcases boot_cases hb with h | ⟨_, timers, _, live, -, ht, -, ⟨cx, hr⟩, htm⟩
    · subst h; exact ⟨.nil, fun _ h => nomatch h⟩
    · exact ⟨(render_handlers fuel).1 hr, htm ▸ startTimers_actions ht⟩
  | step o ev _ ih =>
    obtain ⟨hI, hF⟩ := ih
    cases ev with
    | dispatch target event payload =>
      simp only [Event.step]
      generalize hd : Contract.dispatch p o _ target event payload = r
      obtain ⟨c', out⟩ := r
      rcases dispatch_cases hd with h | h
      · subst h; exact ⟨hI, hF⟩
      · obtain ⟨n, ev, a, args, env, vs, hn, hh, hvs, hr⟩ := h
        exact ⟨hhandler o _ _ _ ev a args env _ vs _ _ hI (hF.1 n hn _ hh) hvs hr, runAction_framed hF hr⟩
    | advance t =>
      simp only [Event.step]
      generalize hd : Contract.advance p o _ t = r
      obtain ⟨c', out⟩ := r
      have := advance_preserves (p := p) (o := o) (fun c => I c ∧ HandlersFrom (Node.handlerLists p.view) c.view)
        (p.tasks.map (·.action)) (fun c ts t ar h => ⟨hclock c ts t ar h.1, h.2⟩)
        (fun c c' out a h ha hr => ⟨htask o c c' out a (List.mem_append.mpr ha) h.1 hr, by
            obtain ⟨-, hv⟩ := runAction_frame hr
            rcases hv with hv | ⟨cx, live, hr⟩
            · exact hv ▸ h.2
            · exact (render_handlers fuel).1 hr⟩)
        ⟨hI, hF.1⟩ hF.2 hd
      exact ⟨this.1.1, this.1.2, this.2⟩

/-- **Invariants of runs.** `I` holds of every reachable configuration
when it holds after boot, is preserved by every action a handler of the
view names (whatever the handler's arguments evaluate to, and whatever
payload follows them), by every task's action, and by moving the clock.
A refused step keeps the configuration, so it needs no case. -/
theorem Reachable.invariant {p : Program} (I : Config → Prop)
    (hboot : ∀ o, I (Contract.boot p o).1)
    (hclock : ∀ (c : Config) ts t ar, I c → I { c with timers := ts, now := t, armed := ar })
    (hhandler : ∀ o c c' out ev a args env ls vs payload rows, I c →
      (ev, a, args) ∈ Node.handlerLists p.view → ListR env false ls args vs →
      runAction p o c a (vs ++ payload) rows = (c', out) → I c')
    (htask : ∀ o c c' out, ∀ a ∈ clockActions p, I c →
      runAction p o c a [] [] = (c', out) → I c') :
    ∀ c, Reachable p c → I c :=
  fun c h => (Reachable.invariant_framed I hboot hclock hhandler htask c h).1

/-- A reachable configuration's elements carry only handlers the view
declares, and its timers run only tasks' actions. -/
theorem Reachable.framed {p c} (h : Reachable p c) : Framed p c :=
  (Reachable.invariant_framed (fun _ => True) (fun _ => trivial) (fun _ _ _ _ _ => trivial)
    (fun _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ => trivial) (fun _ _ _ _ _ _ _ _ => trivial) c h).2

/-! ## Reading simple expressions

What the big-step relation says of the expressions action bodies are made
of, as rewrite rules: with these `simp` turns a `wp` over a concrete body
into a statement about values. -/

@[simp] theorem EvalR.str_iff {env inFn ls s v} : EvalR env inFn ls (.str s) v ↔ v = .str s :=
  ⟨fun h => by cases h; rfl, fun h => h ▸ .str⟩
@[simp] theorem EvalR.bool_iff {env inFn ls b v} : EvalR env inFn ls (.bool b) v ↔ v = .bool b :=
  ⟨fun h => by cases h; rfl, fun h => h ▸ .bool⟩
@[simp] theorem EvalR.num_iff {env inFn ls b v} : EvalR env inFn ls (.num b) v ↔ v = .num (F64.ofBits b) :=
  ⟨fun h => by cases h; rfl, fun h => h ▸ .num⟩
@[simp] theorem EvalR.none_iff {env inFn ls v} : EvalR env inFn ls .none v ↔ v = .none :=
  ⟨fun h => by cases h; rfl, fun h => h ▸ .none⟩

/-- A name bound locally reads its binding. -/
theorem EvalR.var_local {env inFn ls x w v} (hl : lookup x ls = .some w) :
    EvalR env inFn ls (.var x) v ↔ v = w :=
  ⟨fun h => (by
    cases h with
    | «local» h => rw [hl] at h; cases h; rfl
    | global h => rw [hl] at h; cases h),
   fun h => h ▸ .local hl⟩

/-- A name not bound locally, outside a `fn`, reads the component's. -/
theorem EvalR.var_global {env ls x v} (hl : lookup x ls = .none) :
    EvalR env false ls (.var x) v ↔ env.global x = .ok v :=
  ⟨fun h => (by
    cases h with
    | «local» h => rw [hl] at h; cases h
    | global _ _ h => exact h),
   fun h => .global hl rfl h⟩

@[simp] theorem EvalR.not_iff {env inFn ls e v} :
    EvalR env inFn ls (.unary .not e) v ↔ ∃ b, EvalR env inFn ls e (.bool b) ∧ v = .bool !b :=
  ⟨fun h => (by cases h with | «not» h => exact ⟨_, h, rfl⟩),
   fun ⟨_, h, hv⟩ => hv ▸ .not h⟩

/-- A root state's value as an action reads it. -/
theorem actionEnv_global {p c rows x st} (hst : p.states.find? (·.name == x) = .some st)
    (hown : st.owner = .none) :
    (actionEnv p c rows).global x = (lookup x c.slots).elim (.error (.unbound x)) .ok := by
  simp [Env.global, actionEnv, hst, hown]

/-! ## Invariants of one slot -/

/-- Slot `x`, where present, holds a value with property `V`. -/
def SlotIn (x : String) (V : Value → Prop) (slots : List (String × Value)) : Prop :=
  ∀ v, lookup x slots = .some v → V v

theorem lookup_mem {x : String} {v : Value} :
    ∀ {l : List (String × Value)}, lookup x l = .some v → (x, v) ∈ l
  | [], h => by simp [lookup] at h
  | (y, w) :: l, h => by
    simp only [lookup] at h
    split at h
    next hy => cases h; simp at hy; subst hy; exact List.mem_cons_self ..
    next => exact List.mem_cons_of_mem _ (lookup_mem h)

theorem lookup_none {x : String} :
    ∀ {l : List (String × Value)}, (∀ v, (x, v) ∉ l) → lookup x l = .none
  | [], _ => rfl
  | (y, w) :: l, h => by
    simp only [lookup]
    split
    next hy => simp at hy; subst hy; exact absurd (List.mem_cons_self ..) (h w)
    next => exact lookup_none fun v hv => h v (List.mem_cons_of_mem _ hv)

/-- Writes keep `SlotIn x V` when every write to `x` has `V`. -/
theorem applyWrites_slotIn {x V} {slots ws : List (String × Value)} (hs : SlotIn x V slots)
    (hw : ∀ v, (x, v) ∈ ws → V v) : SlotIn x V (applyWrites slots ws) := by
  intro v hv
  rw [lookup_applyWrites] at hv
  cases hl : lookup x slots with
  | none => rw [hl] at hv; cases hv
  | some u =>
    rw [hl] at hv
    simp only [Option.map_some, Option.some.injEq] at hv
    cases hr : lookup x ws.reverse with
    | none => rw [hr] at hv; simp at hv; exact hv ▸ hs u hl
    | some w =>
      rw [hr] at hv; simp at hv
      exact hv ▸ hw w (List.mem_reverse.mp (lookup_mem hr))

theorem Answered.mem {o : Oracle} :
    ∀ {sends ans}, Answered o sends ans → ∀ m w, (m, w) ∈ ans →
      ∃ src vs v, (m, src, vs) ∈ sends ∧ w = .some v
  | _, _, .nil, _, _, h => nomatch h
  | _, _, .cons (src := src) (vs := vs) (v := v) _ ha, m, w, h => by
    rcases List.mem_cons.mp h with h | h
    · simp only [Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      exact ⟨src, vs, v, List.mem_cons_self .., rfl⟩
    · obtain ⟨s, a, b, hs, hw⟩ := ha.mem m w h
      exact ⟨s, a, b, List.mem_cons_of_mem _ hs, hw⟩

/-- **A slot property kept by an action.** It holds after a step when it
held before and every outcome of the action's body writes `x` only with
values in `V` — and, if the body sends into `x`, any answer wrapped in
`some` is in `V`. The obligation is about the body alone (`ExecR` against
the configuration's `actionEnv`): prove it with `wp` or `Derives`. -/
theorem runAction_slotIn {p o c name args rows c' out x V} (hs : SlotIn x V c.slots)
    (hbody : ∀ a, p.actions.find? (·.name == name) = .some a → ∀ fx,
      ExecR (actionEnv p c rows) (actionLocals a args) a.body {} fx →
      (∀ v, (x, v) ∈ fx.writes → V v) ∧ (∀ s ∈ fx.sends, s.1 = x → ∀ v, V (.some v)))
    (h : runAction p o c name args rows = (c', out)) : SlotIn x V c'.slots := by
  cases out with
  | refused e => rw [runAction_refused h]; exact hs
  | ok | poisoned =>
    obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h (by intro e he; cases he)
    obtain ⟨hw, hsend⟩ := hbody a ha fx hx
    rw [hsl]
    refine applyWrites_slotIn hs fun v hv => ?_
    rcases List.mem_append.mp hv with hv | hv
    · obtain ⟨src, vs, w, hm, rfl⟩ := hA.mem x v hv
      exact hsend _ hm rfl w
    · exact hw v hv

mutual
/-- `s` never assigns `x`. -/
def Stmt.noAssign (x : String) : Stmt → Bool
  | .assign t _ => t != x
  | .ifS _ a b => Stmt.noAssigns x a && Stmt.noAssigns x b
  | .matchS _ _ a b => Stmt.noAssigns x a && Stmt.noAssigns x b
  | _ => true
def Stmt.noAssigns (x : String) : List Stmt → Bool
  | [] => true
  | s :: ss => s.noAssign x && Stmt.noAssigns x ss
end

mutual
/-- `s` never sends into `x`. -/
def Stmt.noSend (x : String) : Stmt → Bool
  | .send t _ _ => t != x
  | .ifS _ a b => Stmt.noSends x a && Stmt.noSends x b
  | .matchS _ _ a b => Stmt.noSends x a && Stmt.noSends x b
  | _ => true
def Stmt.noSends (x : String) : List Stmt → Bool
  | [] => true
  | s :: ss => s.noSend x && Stmt.noSends x ss
end

/-- A block that never assigns `x` adds no write to it. -/
theorem ExecR.noAssign {x env ls ss fx fx'} (h : ExecR env ls ss fx fx') (hn : Stmt.noAssigns x ss = true) :
    ∀ v, (x, v) ∈ fx'.writes → (x, v) ∈ fx.writes := by
  induction h with
  | nil => exact fun _ h => h
  | letS _ _ ih => exact ih (by simpa [Stmt.noAssigns, Stmt.noAssign] using hn)
  | assignRoot _ _ _ ih =>
    simp only [Stmt.noAssigns, Stmt.noAssign, Bool.and_eq_true, bne_iff_ne, ne_eq] at hn
    intro v hv
    have := ih hn.2 v hv
    simp only [Effects.write, List.mem_append, List.mem_singleton, Prod.mk.injEq] at this
    rcases this with h | ⟨rfl, -⟩
    · exact h
    · exact absurd rfl hn.1
  | assignRow _ _ _ _ _ ih =>
    simp only [Stmt.noAssigns, Bool.and_eq_true] at hn
    exact ih hn.2
  | command _ _ ih | send _ _ ih | refresh _ ih =>
    simp only [Stmt.noAssigns, Bool.and_eq_true] at hn
    exact ih hn.2
  | ifTrue _ _ _ ih₁ ih₂ | ifFalse _ _ _ ih₁ ih₂ =>
    simp only [Stmt.noAssigns, Stmt.noAssign, Bool.and_eq_true] at hn
    first
      | exact fun v hv => ih₁ hn.1.1 v (ih₂ hn.2 v hv)
      | exact fun v hv => ih₁ hn.1.2 v (ih₂ hn.2 v hv)
  | matchSome _ _ _ ih₁ ih₂ | matchNone _ _ _ ih₁ ih₂ =>
    simp only [Stmt.noAssigns, Stmt.noAssign, Bool.and_eq_true] at hn
    first
      | exact fun v hv => ih₁ hn.1.1 v (ih₂ hn.2 v hv)
      | exact fun v hv => ih₁ hn.1.2 v (ih₂ hn.2 v hv)

/-- A block that never sends into `x` adds no send to it. -/
theorem ExecR.noSend {x env ls ss fx fx'} (h : ExecR env ls ss fx fx') (hn : Stmt.noSends x ss = true) :
    ∀ s ∈ fx'.sends, s.1 = x → s ∈ fx.sends := by
  induction h with
  | nil => exact fun _ h _ => h
  | letS _ _ ih => exact ih (by simpa [Stmt.noSends, Stmt.noSend] using hn)
  | send _ _ ih =>
    simp only [Stmt.noSends, Stmt.noSend, Bool.and_eq_true, bne_iff_ne, ne_eq] at hn
    intro s hs hx
    have := ih hn.2 s hs hx
    simp only [Effects.send, List.mem_append, List.mem_singleton] at this
    rcases this with h | rfl
    · exact h
    · exact absurd hx hn.1
  | assignRoot _ _ _ ih | assignRow _ _ _ _ _ ih | command _ _ ih | refresh _ ih =>
    simp only [Stmt.noSends, Bool.and_eq_true] at hn
    exact ih hn.2
  | ifTrue _ _ _ ih₁ ih₂ | ifFalse _ _ _ ih₁ ih₂ =>
    simp only [Stmt.noSends, Stmt.noSend, Bool.and_eq_true] at hn
    first
      | exact fun s hs hx => ih₁ hn.1.1 s (ih₂ hn.2 s hs hx) hx
      | exact fun s hs hx => ih₁ hn.1.2 s (ih₂ hn.2 s hs hx) hx
  | matchSome _ _ _ ih₁ ih₂ | matchNone _ _ _ ih₁ ih₂ =>
    simp only [Stmt.noSends, Stmt.noSend, Bool.and_eq_true] at hn
    first
      | exact fun s hs hx => ih₁ hn.1.1 s (ih₂ hn.2 s hs hx) hx
      | exact fun s hs hx => ih₁ hn.1.2 s (ih₂ hn.2 s hs hx) hx

/-- An action whose body never assigns or sends into `x` leaves it as it
was. -/
theorem runAction_untouched {p o c name args rows c' out x}
    (hn : ∀ a, p.actions.find? (·.name == name) = .some a →
      Stmt.noAssigns x a.body = true ∧ Stmt.noSends x a.body = true)
    (h : runAction p o c name args rows = (c', out)) : lookup x c'.slots = lookup x c.slots := by
  cases out with
  | refused e => rw [runAction_refused h]
  | ok | poisoned =>
    obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h (by intro e he; cases he)
    obtain ⟨h₁, h₂⟩ := hn a ha
    rw [hsl]
    refine applyWrites_unwritten (lookup_none fun v hv => ?_)
    rcases List.mem_append.mp hv with hv | hv
    · obtain ⟨src, vs, w, hm, -⟩ := hA.mem x v hv
      exact nomatch ExecR.noSend hx h₂ _ hm rfl
    · exact nomatch ExecR.noAssign hx h₁ v hv

/-- What `runAction_slotIn` asks of the body of the action `name`, run
from `c`: every outcome writes `x` only with values in `V`, and any
answer a send into `x` could land is in `V`. -/
def BodyKeeps (p : Program) (x : String) (V : Value → Prop) (c : Config) (name : String)
    (args : List Value) (rows : List RowId) : Prop :=
  ∀ a, p.actions.find? (·.name == name) = .some a → ∀ fx,
    ExecR (actionEnv p c rows) (actionLocals a args) a.body {} fx →
    (∀ v, (x, v) ∈ fx.writes → V v) ∧ (∀ s ∈ fx.sends, s.1 = x → ∀ v, V (.some v))

/-- `BodyKeeps` as a postcondition: the effects so far write `x` only
with values in `V`, and any answer a send into `x` could land is in `V`. -/
def Keeps (x : String) (V : Value → Prop) : Assn := fun _ _ fx =>
  (∀ v, (x, v) ∈ fx.writes → V v) ∧ (∀ s ∈ fx.sends, s.1 = x → ∀ v, V (.some v))

/-- `BodyKeeps` by the weakest precondition of each body. -/
theorem BodyKeeps.of_wp {p x V c name args rows}
    (h : ∀ a ∈ p.actions, a.name = name →
      wp a.body (Keeps x V) (actionEnv p c rows) (actionLocals a args) {}) :
    BodyKeeps p x V c name args rows := fun a ha _ hx => by
  have hn := List.find?_some ha
  simp only [beq_iff_eq] at hn
  exact wp_sound hx (h a (List.mem_of_find?_eq_some ha) hn)

/-- A body that never assigns or sends into `x` keeps any property of it. -/
theorem BodyKeeps.untouched {p x V c name args rows}
    (hn : ∀ a, p.actions.find? (·.name == name) = .some a →
      Stmt.noAssigns x a.body = true ∧ Stmt.noSends x a.body = true) :
    BodyKeeps p x V c name args rows := fun a ha _ hx =>
  ⟨fun v hv => (nomatch ExecR.noAssign hx (hn a ha).1 v hv),
   fun s hs hx' => (nomatch ExecR.noSend hx (hn a ha).2 s hs hx')⟩

/-- A property of slot `x` holds after boot when every value it can start
with has it. -/
theorem boot_slotIn {p o x V} (h : ∀ v, SlotOrigin p x v → V v) :
    SlotIn x V (boot p o).1.slots := fun v hv =>
  h v (boot_slots_origin (out := (boot p o).2) rfl x v (lookup_mem hv))

/-- **Invariants of one slot.** `SlotIn x V` holds of every reachable
configuration when every value `x` can start with has `V`, and the body
of every action a handler or a task names keeps it. -/
theorem Reachable.slotIn {p : Program} {x : String} {V : Value → Prop}
    (hboot : ∀ v, SlotOrigin p x v → V v)
    (hhandler : ∀ c ev a args env ls vs payload rows, SlotIn x V c.slots →
      (ev, a, args) ∈ Node.handlerLists p.view → ListR env false ls args vs →
      BodyKeeps p x V c a (vs ++ payload) rows)
    (htask : ∀ c, ∀ a ∈ clockActions p, SlotIn x V c.slots → BodyKeeps p x V c a [] []) :
    ∀ c, Reachable p c → SlotIn x V c.slots :=
  Reachable.invariant (fun c => SlotIn x V c.slots) (fun _ => boot_slotIn hboot) (fun _ _ _ _ h => h)
    (fun _ c _ _ ev a args env ls vs payload rows hs hh hvs hr =>
      runAction_slotIn hs (hhandler c ev a args env ls vs payload rows hs hh hvs) hr)
    (fun _ c _ _ a ha hs hr => runAction_slotIn hs (htask c a ha hs) hr)

/-- A slot no action assigns can only change by a send's answer, which is
always a `some`: a property of it that every `some` has, every action
keeps. -/
theorem BodyKeeps.of_noAssign {p x V c name args rows}
    (hn : ∀ a ∈ p.actions, Stmt.noAssigns x a.body = true) (hV : ∀ v, V (.some v)) :
    BodyKeeps p x V c name args rows := fun a ha _ hx =>
  ⟨fun v hv => (nomatch ExecR.noAssign hx (hn a (List.mem_of_find?_eq_some ha)) v hv),
   fun _ _ _ v => hV v⟩

/-- **A slot property every action keeps** holds after any event from any
configuration where it held, reachable or not: a dispatch runs one action
or none, an advance a sequence of them. -/
theorem Event.step_slotIn {p o c x V} (ev : Event)
    (hk : ∀ c name args rows, SlotIn x V c.slots → BodyKeeps p x V c name args rows)
    (hs : SlotIn x V c.slots) : SlotIn x V (ev.step p o c).1.slots := by
  cases ev with
  | dispatch target event payload =>
    simp only [Event.step]
    generalize hd : Contract.dispatch p o c target event payload = r
    obtain ⟨c', out⟩ := r
    rcases dispatch_cases hd with h | ⟨n, ev, a, args, env, vs, -, -, -, hr⟩
    · subst h; exact hs
    · exact runAction_slotIn hs (hk _ _ _ _ hs) hr
  | advance t =>
    simp only [Event.step]
    generalize hd : Contract.advance p o c t = r
    obtain ⟨c', out⟩ := r
    exact (advance_preserves (fun c => SlotIn x V c.slots) (c.timers.map (·.action))
      (fun _ _ _ _ h => h) (fun _ _ _ a h _ hr => runAction_slotIn h (hk _ _ _ _ h) hr) hs
      (fun tm h => List.mem_map_of_mem h) hd).1

/-- **What the clock cannot change.** When no task's action assigns or
sends into `x`, moving the clock leaves `x` as it was, in every reachable
configuration. -/
theorem Reachable.advance_untouched {p c o t c' out x} (hc : Reachable p c)
    (hn : ∀ a ∈ p.actions, a.name ∈ clockActions p →
      Stmt.noAssigns x a.body = true ∧ Stmt.noSends x a.body = true)
    (h : advance p o c t = (c', out)) : lookup x c'.slots = lookup x c.slots :=
  (advance_preserves (fun c' => lookup x c'.slots = lookup x c.slots) (p.tasks.map (·.action))
    (fun _ _ _ _ h => h)
    (fun _ _ _ a h ha hr => by
      rw [runAction_untouched (fun ad had => hn ad (List.mem_of_find?_eq_some had) (by
        have := List.find?_some had
        simp only [beq_iff_eq] at this
        exact this ▸ List.mem_append.mpr ha)) hr, h])
    rfl hc.framed.2 h).1

end Contract
