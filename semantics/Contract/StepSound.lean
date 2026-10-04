/-
Well-typed programs don't go wrong.

`ConfigOK p c` is a well-typed configuration: root slots present and of
their types, settled derives and resources of theirs, every live row's
slots of theirs, every rendered element's handlers naming an existing
action with typed curried arguments, every timer running an action of no
parameters. For a program the checker accepts (`check p = true`):

* every reachable configuration is `ConfigOK` (`reachable_configOK`);
* every step from one — `dispatch` or `advance`, whatever the oracle —
  lands in a `ConfigOK` configuration and either commits or fails for a
  `Legitimate` reason: never a type error, an unbound name or a `pending`
  read (`step_sound`; with boot, `dont_go_wrong` in `Contract.BootSound`).
-/
import Contract.RenderSound
import Contract.SettleComplete
import Contract.TypeCheck

namespace Contract

/-! ## Legitimate failures -/

/-- Why a step of a well-typed program may fail. Every refusal or poison
is one of these (`Legitimate.of_strict`): there is no constructor for a
type error (`Err.type`), an unbound name (`Err.unbound`) or a `pending`
read — a step reads only settled values (`settle_complete`), and
settlement waits on its `pending` reads rather than failing with them.

* `refused` covers the data seam (an oracle with no answer for a call, a
  source or mutation answering a value of the wrong shape), a number that
  is not finite crossing a boundary (a derive, a slot written or
  initialized, an action's argument, a row key: `ValTy` admits NaN and the
  infinities, `conforms` does not), the host's own input (no element with
  that `testId`, no handler for that event, a payload that does not fit the
  action's remaining parameters), the router (a forged router value
  committed, a verb or read of a value of the shape `Router` that is not a
  valid router, `searchParam` of a forged entry, an empty or dot path
  segment, a launch of `/` the table refuses), a row slot read or written
  outside its row, a cycle among derives and resources, resource limits
  (the interpreter's fuel, the timer fire limit), and a configuration
  already poisoned.
* `unsupported` is what the semantics leaves out (`format*`, geometry
  reads, a payload for a form control, frame tasks, a mutation's `then`). -/
inductive Legitimate : Err → Prop
  | refused (why : String) : Legitimate (.refused why)
  | unsupported (what : String) : Legitimate (.unsupported what)

theorem Legitimate.of_strict {e : Err} (h : Strict e) : Legitimate e := by
  obtain ⟨h, hn⟩ := h
  cases e <;> simp [Legit] at h hn <;> constructor

theorem Legitimate.strict {e : Err} (h : Legitimate e) : Strict e := by
  cases h <;> exact ⟨trivial, by simp⟩

theorem Legitimate.not_type {e : Err} (h : Legitimate e) : ∀ w, e ≠ .type w := by
  intro w he; subst he; cases h

theorem Legitimate.not_unbound {e : Err} (h : Legitimate e) : ∀ x, e ≠ .unbound x := by
  intro x he; subst he; cases h

theorem Legitimate.not_pending {e : Err} (h : Legitimate e) : e ≠ .pending := by
  intro he; subst he; cases h

/-- A step's outcome is a commit, or a refusal or poison for a legitimate
reason. -/
def OutcomeOK : Outcome → Prop
  | .ok => True
  | .refused e => Legitimate e
  | .poisoned e => Legitimate e

/-! ## Well-typed configurations -/

/-- Every timer runs an action that exists and takes no parameters. -/
def TimersOK (p : Program) (timers : List Timer) : Prop :=
  ∀ tm ∈ timers, ∃ a, p.actions.find? (·.name == tm.action) = .some a ∧ a.params = []

/-- A well-typed configuration. Its slots are present and every derive and
resource settled, unless nothing can run (no element, no timer: the
configuration of a refused boot). -/
structure ConfigOK (p : Program) (c : Config) : Prop where
  slots : SlotsOK p c.slots
  present : (SlotsPresent p c.slots ∧ Complete p c.settled) ∨ (c.view = [] ∧ c.timers = [] ∧ c.armed = [])
  settled : SettledOK p c.settled
  store : StoreOK p c.store
  view : ViewOK p c.view
  timers : TimersOK p c.timers

theorem ConfigOK.empty {p : Program} : ConfigOK p Config.empty :=
  ⟨SlotsOK.nil, .inr ⟨rfl, rfl, rfl⟩, SettledOK.empty, StoreOK.nil, ViewOK.nil, fun _ h => nomatch h⟩

/-- The environment an action or a handler's arguments evaluate in. -/
theorem ConfigOK.envGood {p : Program} {c : Config} (hc : ConfigOK p c) (hpres : SlotsPresent p c.slots)
    (rows : List RowId) :
    EnvGood p { prog := p, slots := c.slots, derives := c.settled.derives,
                resources := c.settled.resources, rows := rowSlots c.store rows, now := c.now } :=
  ⟨rfl, hc.slots, hpres, hc.store.rowSlots rows, hc.settled.1, hc.settled.2⟩

/-! ## Pieces of a commit -/

theorem agree_of_conforms {p : Program} (hp : WellTyped p) :
    ∀ {params : List (String × Ty)} {args : List Value}, params.length = args.length →
      (params.zip args).all (fun (q, v) => argOk p q v) = true →
      (∀ q ∈ params, q.2.complete = true) → Agree p ((params.map (·.1)).zip args) params
  | [], [], _, _, _ => .nil
  | (x, t) :: params, v :: args, hl, hc, hcomp => by
    simp only [List.zip_cons_cons, List.all_cons, Bool.and_eq_true] at hc
    simp only [List.map_cons, List.zip_cons_cons]
    exact .cons (argOk_valTy hp.shapes hc.1 (hcomp (x, t) (by simp)))
      (agree_of_conforms hp (by simpa using hl) hc.2 fun q hq => hcomp q (by simp [hq]))
  | [], _ :: _, hl, _, _ | _ :: _, [], hl, _, _ => by simp at hl

theorem StoreOK.applyRowWrites {p : Program} {rows : List RowId} :
    ∀ {store : RowStore} {ws : List (String × Value)}, StoreOK p store →
      (∀ w ∈ ws, conforms p w.2 (slotTy p w.1) = true) → StoreOK p (Contract.applyRowWrites p store rows ws)
  | store, [], h, _ => h
  | store, (x, v) :: ws, h, hw => by
    simp only [Contract.applyRowWrites, List.foldl_cons]
    refine StoreOK.applyRowWrites (ws := ws) ?_ fun w hm => hw w (by simp [hm])
    have hv := hw (x, v) (by simp)
    split
    · intro i s hm
      simp only [List.mem_map] at hm
      obtain ⟨⟨i', s'⟩, hm', he⟩ := hm
      split at he
      · simp only [Prod.mk.injEq] at he; obtain ⟨rfl, rfl⟩ := he
        exact (h _ _ hm').setSlot hv
      · simp only [Prod.mk.injEq] at he; obtain ⟨rfl, rfl⟩ := he
        exact h _ _ hm'
    · exact h

/-- What settlement reads, in an environment whose root slots outside
`lifted p` are present and of their types: `EnvOK` for `settleScope p`. -/
theorem settleEnvOK {p : Program} (hp : WellTyped p) {slots : List (String × Value)} {now : F64}
    (hpres : ∀ x, lifted p x = false → ((∃ st ∈ p.states, st.name = x ∧ st.owner = .none) ∨
      ∃ m ∈ p.mutations, m.name = x) → x ∈ slots.map (·.1))
    (hslots : ∀ x v, (x, v) ∈ slots → lifted p x = false → ValTy p v (slotTy p x)) :
    SettleHyps p (settleScope p) slots now := by
  refine ⟨⟨hp.fns, hp.routeShapes⟩, fun d hd => ?_, fun r hr => (hp.resources r hr).imp fun _ h => h.1,
    fun st hst => ?_⟩
  · obtain ⟨t, ht, -⟩ := hp.derives d hd; exact ⟨t, ht⟩
  have := envOKE_gen (E := Legit) (L := lifted p)
    (env := { prog := p, slots, derives := st.derives, resources := st.resources, now })
    hp (fun _ h _ => h) rfl hpres hslots RowsOK.nil hst (.inl trivial)
  exact ⟨this.1, fun x t hx => GoodR.of_goodW (this.2.1 x t hx)⟩

/-- Settle and render against well-typed slots: settlement fails only
legitimately (never `pending`) and settles well-typed values, every derive
and resource; rendering fails only legitimately and renders a well-typed
view and row store. -/
theorem update_good {p : Program} (hp : WellTyped p) {o slots store now prev force}
    (hs : SlotsOK p slots) (hpres : SlotsPresent p slots) (hstore : StoreOK p store)
    (hprev : SettledOK p prev) :
    GoodW Strict (fun r => SettledOK p r.1 ∧ Complete p r.1 ∧
        GoodW Strict (fun r => ViewOK p r.1 ∧ StoreOK p r.2) r.2)
      (update p o slots store now prev force) := by
  have N := Names.of hp.names
  unfold update
  cases hst : settle p o slots now prev force with
  | error e =>
    have := settle_strict (o := o) (now := now) (force := force) N.resources
      (settleEnvOK hp (hpres.of _) (fun _ _ hx _ => hs.valTy hp hx)) hprev
    rw [hst] at this; exact this
  | ok st =>
    have hok := settle_settledOK N.resources hprev hst
    have hc := settle_complete hst
    refine ⟨hok, hc, ?_⟩
    exact (render_good hp (E := Strict) (fun _ h hn => ⟨h, hn⟩) fuel).1 ⟨rfl, hs, hpres, RowsOK.nil, hok⟩
      (.inl hc) hstore StoreOK.nil LocalsOK.nil hp.view

theorem mapM_goodW {α β} {E : Err → Prop} {f : α → Result β} : ∀ {xs : List α},
    (∀ x ∈ xs, GoodW E (fun _ => True) (f x)) → GoodW E (fun _ => True) (xs.mapM f)
  | [], _ => trivial
  | x :: xs, h => by
    simp only [List.mapM_cons]
    exact GoodW.bind (h x (by simp)) fun _ _ =>
      GoodW.bind (mapM_goodW fun y hy => h y (by simp [hy])) fun _ _ => trivial

/-! ## Actions -/

/-- **An action is safe.** In a well-typed configuration whose slots are
present, running an action that exists lands in a well-typed
configuration, and commits, or refuses or poisons legitimately. -/
theorem runAction_sound {p : Program} (hp : WellTyped p) {o c name args rows c' out}
    (hc : ConfigOK p c) (hlive : SlotsPresent p c.slots ∧ Complete p c.settled)
    (hname : ∃ a, p.actions.find? (·.name == name) = .some a)
    (h : runAction p o c name args rows = (c', out)) : ConfigOK p c' ∧ OutcomeOK out := by
  have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
  obtain ⟨hpres, hcomp⟩ := hlive
  have refusedS : ∀ w, Strict (.refused w) := fun _ => ⟨trivial, by simp⟩
  have keep : ∀ {e}, (c, Outcome.refused e) = (c', out) → Strict e → ConfigOK p c' ∧ OutcomeOK out :=
    fun h' he => by
      simp only [Prod.mk.injEq] at h'; rw [← h'.1, ← h'.2]; exact ⟨hc, Legitimate.of_strict he⟩
  simp only [runAction] at h
  split at h
  · exact keep h (refusedS _)
  split at h
  · next hnone => obtain ⟨a, ha⟩ := hname; rw [ha] at hnone; cases hnone
  next a ha =>
  have ham := List.mem_of_find?_eq_some ha
  split at h
  · exact keep h (refusedS _)
  next hlen =>
  split at h
  · exact keep h (refusedS _)
  next hconf =>
  have hloc : LocalsOK p a.params.reverse ((a.params.map (·.1)).zip args).reverse :=
    (Agree.reverse (agree_of_conforms hp (by simpa using hlen) (by simpa using hconf)
      (hp.params a ham))).localsOK
  have hexec := exec_sound_E (n := fuel) hfn (fun _ h hn => ⟨h, hn⟩) ((hc.envGood hpres rows).envOKE hp hcomp) hloc
    (hp.actions a ham)
  split at h
  · next e he => rw [he] at hexec; exact keep h hexec
  next fx hx =>
  rw [hx] at hexec
  split at h
  · next e he =>
    refine keep h ?_
    have : GoodW Strict (fun _ => True) (fx.sends.mapM fun (m, src, vs) => do
        let v ← o.ask src vs
        let ty := ((p.mutations.find? (·.name == m)).map (·.ty)).getD .unknown
        if !conforms p v ty then throw (Err.refused s!"mutation `{m}` answered with the wrong shape")
        pure (m, Value.some v)) := by
      refine mapM_goodW fun ⟨m, src, vs⟩ _ => GoodW.bind (ask_legit o src vs "") fun v _ => ?_
      dsimp only
      split
      · simp only [throw, throwThe, MonadExceptOf.throw, bind, Except.bind, GoodW]; exact refusedS _
      · trivial
    rw [he] at this; exact this
  next answered hans =>
  split at h
  · exact keep h (refusedS _)
  next hw =>
  generalize hS : List.foldl _ c.slots (answered ++ fx.writes) = S at h
  have hSeq : S = applyWrites c.slots (answered ++ fx.writes) := by rw [← hS]; rfl
  subst hSeq
  simp only [Bool.not_eq_true', Bool.not_eq_false, List.all_eq_true] at hw
  have hrow : ∀ w ∈ fx.rowWrites, conforms p w.2 (slotTy p w.1) = true :=
    fun w hm => hw w (List.mem_append_right _ hm)
  have hkeys := applyWrites_keys c.slots (answered ++ fx.writes)
  have hpres' : SlotsPresent p (applyWrites c.slots (answered ++ fx.writes)) := hpres.of_keys hkeys
  split at h
  · exact keep h (refusedS _)
  -- The slots after the commit, as `runAction_slotsOK` computes them.
  have hok : SlotsOK p (applyWrites c.slots (answered ++ fx.writes)) := by
    have hsends := (exec_sound hx).sends (hp.slotTyped.sends a ham) (by simp)
    refine hc.slots.applyWrites fun w hw' => ?_
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
    · exact hw w (List.mem_append_left _ hw')
  have hup := update_good (o := o) (now := c.now) (force := fx.refreshes) hp hok hpres'
    (hc.store.applyRowWrites (rows := rows) hrow) hc.settled
  split at h
  · next e he => rw [he] at hup; exact keep h hup
  · next st view live hu =>
    rw [hu] at hup
    obtain ⟨hst, hcm, hv, hl⟩ := hup
    simp only [Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    exact ⟨⟨hok, .inl ⟨hpres', hcm⟩, hst, hl, hv, hc.timers⟩, trivial⟩
  · next st e hu =>
    rw [hu] at hup
    obtain ⟨hst, hcm, he⟩ := hup
    simp only [Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    exact ⟨⟨hok, .inl ⟨hpres', hcm⟩, hst, hc.store.applyRowWrites hrow, hc.view, hc.timers⟩,
      Legitimate.of_strict he⟩

/-! ## Dispatch -/

/-- What a handler's arguments evaluate to, in a well-typed configuration:
values of types at most the action's leading parameters, or a legitimate
failure. So the action's arity and argument checks, on the curried part,
can refuse only a number that is not finite; the rest is the host's
payload. -/
theorem handler_args_good {p : Program} (hp : WellTyped p) {c : Config} {n : VNode} {h}
    (hc : ConfigOK p c) (hn : VNode.In n c.view) (hh : h ∈ n.handlers) :
    ∃ a, p.actions.find? (·.name == h.2.1) = .some a ∧ ∃ ts, Ty.lePrefix ts (a.params.map (·.2)) = true ∧
      (SlotsPresent p c.slots → Complete p c.settled →
        GoodW Strict (fun vs => ValTyL p vs ts) (evalList fuel (actionEnv p c n.rows) false n.locals h.2.2)) := by
  obtain ⟨a, ha, Γ, ts, hl, hts, hle⟩ := hc.view n hn h hh
  have hfn : ProgOK p := ⟨hp.fns, hp.routeShapes⟩
  exact ⟨a, ha, ts, hle, fun hpres hcomp =>
    evalList_sound_E hfn (fun _ h hn => ⟨h, hn⟩) ((hc.envGood hpres n.rows).envOKE hp hcomp) hl hts⟩

theorem VNode.In.ne_nil {n : VNode} {vs : List VNode} (h : VNode.In n vs) : vs ≠ [] := by
  cases h <;> simp

/-- **A dispatch is safe.** -/
theorem dispatch_sound {p : Program} (hp : WellTyped p) {o c target event payload c' out}
    (hc : ConfigOK p c) (h : dispatch p o c target event payload = (c', out)) :
    ConfigOK p c' ∧ OutcomeOK out := by
  have refusedS : ∀ w, Strict (.refused w) := fun _ => ⟨trivial, by simp⟩
  have keep : ∀ {e}, (c, Outcome.refused e) = (c', out) → Strict e → ConfigOK p c' ∧ OutcomeOK out :=
    fun h' he => by
      simp only [Prod.mk.injEq] at h'; rw [← h'.1, ← h'.2]; exact ⟨hc, Legitimate.of_strict he⟩
  simp only [dispatch] at h
  split at h
  · exact keep h (refusedS _)
  split at h
  · exact keep h (refusedS _)
  next n hn =>
  have hin := findTestId_in hn
  have hlive := hc.present.resolve_right fun ⟨hv, _⟩ => hin.ne_nil hv
  split at h
  · exact keep h (refusedS _)
  next ev a args hh =>
  obtain ⟨ad, had, ts, -, hargs⟩ := handler_args_good hp hc hin (List.mem_of_find?_eq_some hh)
  have hargs := hargs hlive.1 hlive.2
  simp only [actionEnv] at hargs
  split at h
  · exact keep h ⟨trivial, by simp⟩
  split at h
  · next e he => rw [he] at hargs; exact keep h hargs
  · exact runAction_sound hp hc hlive ⟨ad, had⟩ h

/-! ## Handler arguments at the action's boundary -/

mutual
/-- Every number in a value is finite. -/
def Value.Finite : Value → Prop
  | .num f => Number.isFinite f = true
  | .some v => Value.Finite v
  | .list xs => Value.FiniteAll xs
  | .record _ vs => Value.FiniteAll vs
  | _ => True
/-- Every number in each value is finite. -/
def Value.FiniteAll : List Value → Prop
  | [] => True
  | v :: vs => Value.Finite v ∧ Value.FiniteAll vs
end

/-- What separates `ValTy` from `conforms` is finiteness alone: a value of
a type whose numbers are finite passes the runtime check. -/
theorem conforms_of_valTy {p : Program} : ∀ (v : Value) {t : Ty}, ValTy p v t → v.Finite → conforms p v t = true
  | .num _, t, h, hf | .bool _, t, h, hf | .str _, t, h, hf | .unit, t, h, hf | .none, t, h, hf => by
    cases t <;> simp_all [ValTy, conforms, Value.Finite]
  | .some v, t, h, hf => by
    cases t <;> simp [ValTy] at h
    case option t => simp only [conforms]; exact conforms_of_valTy v h hf
  | .list xs, t, h, hf => by
    cases t <;> simp [ValTy] at h
    case list t => simp only [conforms]; exact all xs h hf
  | .record s vs, t, h, hf => by
    cases t <;> simp [ValTy] at h
    case record s' =>
      obtain ⟨rfl, sh, hsh, hfs⟩ := h
      simp only [conforms, hsh, beq_self_eq_true, Bool.true_and, Bool.and_eq_true, beq_iff_eq]
      exact ⟨hfs.length, fields vs sh.fields hfs hf⟩
where
  all : ∀ (xs : List Value) {t : Ty}, ValTys p xs t → Value.FiniteAll xs → conformsAll p xs t = true
    | [], _, _, _ => rfl
    | x :: xs, t, h, hf => by
      simp only [conformsAll, Bool.and_eq_true]
      exact ⟨conforms_of_valTy x h.1 hf.1, all xs h.2 hf.2⟩
  fields : ∀ (vs : List Value) (fs : List Field), FieldsTy p vs fs → Value.FiniteAll vs →
      conformsFields p vs fs = true
    | [], [], _, _ => rfl
    | v :: vs, f :: fs, h, hf => by
      simp only [conformsFields, Bool.and_eq_true]
      exact ⟨conforms_of_valTy v h.1 hf.1, fields vs fs h.2 hf.2⟩
    | [], _ :: _, _, _ => rfl
    | _ :: _, [], h, _ => by simp [FieldsTy] at h

/-- A handler's curried arguments, typed at most the action's leading
parameters and finite, pass the action's argument check, whatever
payload the host appends: what is left of the check is about the payload
alone (its arity and its values). -/
theorem curried_conform {p : Program} :
    ∀ {params : List (String × Ty)} {vs : List Value} {ts : List Ty} (payload : List Value),
      ValTyL p vs ts → Ty.lePrefix ts (params.map (·.2)) = true → Value.FiniteAll vs →
      vs.length ≤ params.length ∧
      ((params.zip (vs ++ payload)).all (fun (q, v) => argOk p q v) =
        ((params.drop vs.length).zip payload).all (fun (q, v) => argOk p q v))
  | params, [], [], payload, _, _, _ => by simp
  | (x, u) :: params, v :: vs, t :: ts, payload, hv, hle, hf => by
    simp only [ValTyL] at hv
    simp only [List.map_cons, Ty.lePrefix, Bool.and_eq_true] at hle
    obtain ⟨hl, he⟩ := curried_conform (params := params) payload hv.2 hle.2 hf.2
    refine ⟨by simpa using hl, ?_⟩
    simp only [List.cons_append, List.zip_cons_cons, List.all_cons, List.length_cons, List.drop_succ_cons]
    have hok : argOk p (x, u) v = true := by
      simp only [argOk]
      by_cases hx : hiddenParam x = true
      · simp only [hx, ↓reduceIte]; exact typed_of_valTy v (hv.1.mono hle.1)
      · simp only [hx, Bool.false_eq_true, ↓reduceIte]; exact conforms_of_valTy v (hv.1.mono hle.1) hf.1
    rw [hok, he, Bool.true_and]
  | [], _ :: _, _ :: _, _, _, hle, _ => by simp [Ty.lePrefix] at hle
  | _, [], _ :: _, _, hv, _, _ | _, _ :: _, [], _, hv, _, _ => by simp [ValTyL] at hv

/-! ## Moving the clock -/

theorem ConfigOK.withTimers {p : Program} {c : Config} (hc : ConfigOK p c)
    (hlive : SlotsPresent p c.slots ∧ Complete p c.settled)
    {ts : List Timer} (hts : TimersOK p ts) (t : F64) : ConfigOK p { c with timers := ts, now := t } :=
  ⟨hc.slots, .inl hlive, hc.settled, hc.store, hc.view, hts⟩

theorem ConfigOK.withNow {p : Program} {c : Config} (hc : ConfigOK p c) (t : F64) :
    ConfigOK p { c with now := t } :=
  ⟨hc.slots, hc.present, hc.settled, hc.store, hc.view, hc.timers⟩

theorem ConfigOK.withArmed {p : Program} {c : Config} (hc : ConfigOK p c)
    (hpres : SlotsPresent p c.slots ∧ Complete p c.settled)
    (ar : List (String × F64)) (t : F64) : ConfigOK p { c with armed := ar, now := t } :=
  ⟨hc.slots, .inl hpres, hc.settled, hc.store, hc.view, hc.timers⟩

/-- Nothing armed, no `then` due. -/
theorem dueThen_nil {p : Program} {t : F64} : dueThen p [] t = .none := by
  unfold dueThen
  generalize p.mutations = ms
  suffices ∀ (ms : List MutationDecl), ms.foldl _ Option.none = Option.none from this ms
  intro ms
  induction ms with
  | nil => rfl
  | cons m ms ih =>
    simp only [List.foldl_cons, List.find?_nil]
    cases m.andThen <;> exact ih

/-- A mutation's `then` names an action of the program. -/
theorem thenAction_exists {p : Program} (hp : WellTyped p) {a : String} (ha : a ∈ thenActions p) :
    ∃ ad, p.actions.find? (·.name == a) = .some ad := by
  obtain ⟨m, hm, hma⟩ := List.mem_filterMap.mp ha
  obtain ⟨ad, had, -⟩ := hp.thenActions m hm a hma
  exact ⟨ad, had⟩

theorem advance_go_sound {p : Program} (hp : WellTyped p) {o due pick finish}
    (hdue : ∀ c tm i, due c = Option.some (tm, i) → tm ∈ c.timers)
    (hpick : ∀ c m w a, pick c = Option.some (m, w, a) → a ∈ thenActions p ∧ c.armed ≠ [])
    (hfinish : ∀ c, ConfigOK p c → ConfigOK p (finish c).1 ∧ (finish c).2 = .ok) :
    ∀ n c, ConfigOK p c → ConfigOK p (advance.go p o due pick finish n c).1 ∧
      OutcomeOK (advance.go p o due pick finish n c).2
  | 0, c, hc => by
    rw [advance.go]
    split
    · obtain ⟨h1, h2⟩ := hfinish c hc; rw [h2]; exact ⟨h1, trivial⟩
    · exact ⟨hc, Legitimate.refused _⟩
  | n + 1, c, hc => by
    rw [advance.go]
    split
    · exact ⟨hc, Legitimate.refused _⟩
    split
    next m w a hpk =>
      dsimp only
      generalize hr : runAction p o
        { c with armed := c.armed.filter (·.1 != m), now := if c.now < w then w else c.now }
        a [] [] = r
      obtain ⟨c₂, out₂⟩ := r
      -- Something is armed, so the slots are present (a refused boot's
      -- configuration has nothing armed).
      have hpres := hc.present.resolve_right fun ⟨_, _, ha⟩ => (hpick c m w a hpk).2 ha
      have hc₁ := hc.withArmed hpres (c.armed.filter (·.1 != m)) (if c.now < w then w else c.now)
      obtain ⟨hc₂, hout₂⟩ := runAction_sound hp hc₁ hpres (thenAction_exists hp (hpick c m w a hpk).1) hr
      cases out₂ with
      | ok => exact advance_go_sound hp hdue hpick hfinish n c₂ hc₂
      | refused => exact ⟨hc₂, hout₂⟩
      | poisoned => exact ⟨hc₂, hout₂⟩
    split
    · obtain ⟨h1, h2⟩ := hfinish c hc; rw [h2]; exact ⟨h1, trivial⟩
    next tm i hd =>
    have htm := hdue c tm i hd
    have hpres := hc.present.resolve_right fun ⟨_, ht, _⟩ => by simp [ht] at htm
    obtain ⟨a, ha, hpa⟩ := hc.timers tm htm
    have hset : TimersOK p (c.timers.set i tm.fired) := fun y hy => by
      rcases List.mem_or_eq_of_mem_set hy with hy | rfl
      · exact hc.timers y hy
      · unfold Timer.fired; split <;> (try split) <;> exact ⟨a, ha, hpa⟩
    dsimp only
    generalize hr : runAction p o { c with timers := c.timers.set i tm.fired, now := tm.next }
      tm.action [] [] = r
    obtain ⟨c₂, out₂⟩ := r
    have hc₁ := hc.withTimers hpres hset tm.next
    obtain ⟨hc₂, hout₂⟩ := runAction_sound hp hc₁ hpres ⟨a, ha⟩ hr
    cases out₂ with
    | ok => exact advance_go_sound hp hdue hpick hfinish n c₂ hc₂
    | refused => exact ⟨hc₂, hout₂⟩
    | poisoned => exact ⟨hc₂, hout₂⟩

/-- **Moving the clock is safe.** -/
theorem advance_sound {p : Program} (hp : WellTyped p) {o c t c' out}
    (hc : ConfigOK p c) (h : advance p o c t = (c', out)) : ConfigOK p c' ∧ OutcomeOK out := by
  simp only [advance] at h
  split at h
  · simp only [Prod.mk.injEq] at h; obtain ⟨rfl, rfl⟩ := h; exact ⟨hc, trivial⟩
  obtain ⟨rfl, rfl⟩ : c' = _ ∧ out = _ := ⟨congrArg Prod.fst h.symm, congrArg Prod.snd h.symm⟩
  refine advance_go_sound hp ?_ ?_ ?_ _ c hc
  · intro c tm i hd
    rcases foldl_pick (by
        intro b x
        obtain ⟨xt, xi⟩ := x
        dsimp only
        split <;> (try split) <;> (try split) <;> simp) hd with h | h
    · cases h
    · exact List.mem_zipIdx h |>.2.2 ▸ List.getElem_mem _
  · intro c m w a hp
    have armed : ∀ {r}, dueThen p c.armed t = .some r → c.armed ≠ [] := fun h he => by
      rw [he, dueThen_nil] at h; cases h
    split at hp
    · split at hp
      · cases hp; exact ⟨dueThen_mem ‹_›, armed ‹_›⟩
      · cases hp
    · exact ⟨dueThen_mem hp, armed hp⟩
  · intro c hc
    exact ⟨hc.withNow _, rfl⟩

/-! ## Boot -/

theorem initSlots_present {p : Program} {s} (h : initSlots p = .ok s) : SlotsPresent p s := by
  simp only [initSlots, Except.bind_ok_iff, Except.pure_ok_iff] at h
  obtain ⟨s0, hs0, rfl⟩ := h
  have hI := foldlM_inv_rest
    (fun rest (slots : List (String × Value)) => ∀ st ∈ p.states, st.owner = .none →
      st.name ∈ slots.map (·.1) ∨ st ∈ rest) ?_ (l := p.states) (b := []) (fun st hst _ => .inr hst) hs0
  · refine ⟨fun st hst ho => ?_, fun m hm => ?_⟩
    · rcases hI st hst ho with h | h
      · simp only [List.map_append, List.mem_append]; exact .inl h
      · cases h
    · simp only [List.map_append, List.mem_append, List.map_map]
      right; exact List.mem_map.mpr ⟨m, hm, rfl⟩
  intro a l b b' hb hf st hst ho
  have keep : ∀ v, b' = b ++ [(a.name, v)] → st.name ∈ b'.map (·.1) ∨ st ∈ l := by
    intro v he; subst he
    rcases hb st hst ho with h | h
    · left; simp [h]
    · simp only [List.mem_cons] at h
      rcases h with rfl | h
      · left; simp
      · exact .inr h
  split at hf
  · next hown =>
    simp only [Except.pure_ok_iff] at hf; subst hf
    rcases hb st hst ho with h | h
    · exact .inl h
    · simp only [List.mem_cons] at h
      rcases h with rfl | h
      · simp [ho] at hown
      · exact .inr h
  split at hf
  · simp only [Except.pure_ok_iff] at hf; exact keep _ hf.symm
  split at hf
  · split at hf
    · simp only [Except.pure_ok_iff] at hf; exact keep _ hf.symm
    · simp [throw, throwThe, MonadExceptOf.throw] at hf
  simp only [Except.bind_ok_iff] at hf
  obtain ⟨v, -, hf⟩ := hf
  split at hf
  · obtain ⟨_, h', _⟩ := Except.bind_ok_iff.mp hf; cases h'
  · simp only [Except.pure_ok_iff] at hf; exact keep _ hf.symm

theorem lateSlots_keys {p : Program} {st slots slots'} (h : lateSlots p st slots = .ok slots') :
    slots'.map (·.1) = slots.map (·.1) := by
  refine foldlM_inv (fun s : List (String × Value) => s.map (·.1) = slots.map (·.1)) ?_ rfl h
  intro b a b' _ hb hf
  simp only at hf
  split at hf
  · simp only [Except.pure_ok_iff] at hf; exact hf ▸ hb
  simp only [Except.bind_ok_iff] at hf
  obtain ⟨w, -, hf⟩ := hf
  split at hf
  · simp at hf
  simp only [Except.pure_ok_iff] at hf
  subst hf
  rw [setSlot_keys]; exact hb

/-- **Boot lands in a well-typed configuration** (the empty one, when it is
refused). -/
theorem boot_configOK {p : Program} (hp : WellTyped p) (o : Oracle) : ConfigOK p (boot p o).1 := by
  have N := Names.of hp.names
  have hs := boot_slotsOK (o := o) hp.slotTyped
  generalize hb : boot p o = r at hs ⊢
  obtain ⟨c, out⟩ := r
  simp only at hs ⊢
  unfold boot at hb
  have empty : ∀ {e}, (Config.empty, Outcome.refused e) = (c, out) → ConfigOK p c := fun h => by
    simp only [Prod.mk.injEq] at h; rw [← h.1]; exact ConfigOK.empty
  split at hb
  · exact empty hb
  next slots₀ hi =>
  split at hb
  · exact empty hb
  next timers ht =>
  split at hb
  · exact empty hb
  next st hst =>
  split at hb
  · exact empty hb
  next slots hl =>
  have hpres : SlotsPresent p slots := (initSlots_present hi).of_keys (lateSlots_keys hl)
  have hsettled := settle_settledOK N.resources SettledOK.empty hst
  have hcomp := settle_complete hst
  have htimers : TimersOK p timers := by
    intro tm htm
    obtain ⟨t, htk, he⟩ := List.mem_map.mp (startTimers_actions ht tm htm)
    rw [← he]; exact hp.taskActions t htk
  dsimp only at hb
  split at hb
  · next view live hr =>
    simp only [Prod.mk.injEq] at hb
    obtain ⟨rfl, -⟩ := hb
    have hrender := (render_good hp (E := Legit) (fun _ h _ => h) fuel).1
      (cx := { env := { prog := p, slots, derives := st.derives, resources := st.resources, now := 0 },
               store := [] })
      ⟨rfl, hs, hpres, RowsOK.nil, hsettled.1, hsettled.2⟩ (.inr trivial) StoreOK.nil StoreOK.nil LocalsOK.nil
      hp.view
    rw [hr] at hrender
    exact ⟨hs, .inl ⟨hpres, hcomp⟩, hsettled, hrender.2, hrender.1, htimers⟩
  · exact empty hb

/-! ## Runs -/

/-- **Every reachable configuration of a well-typed program is
well typed**: root slots present and of their types, settled derives and
resources of theirs, live rows' slots of theirs, rendered handlers naming
existing actions with typed arguments, timers running actions of no
parameters. -/
theorem reachable_configOK {p : Program} (hp : WellTyped p) {c : Config} (h : Reachable p c) :
    ConfigOK p c := by
  induction h with
  | boot o => exact boot_configOK hp o
  | step o ev _ ih =>
    cases ev with
    | dispatch target event payload =>
      exact (dispatch_sound (o := o) (target := target) (event := event) (payload := payload) hp ih rfl).1
    | advance t => exact (advance_sound (o := o) (t := t) hp ih rfl).1

/-- **Every step is safe.** From a well-typed configuration, an event
(`dispatch` or `advance`, whatever the oracle answers) lands in a
well-typed configuration, and either commits or refuses or poisons for a
`Legitimate` reason. -/
theorem step_sound {p : Program} (hp : WellTyped p) {c : Config} (hc : ConfigOK p c) (o : Oracle)
    (ev : Event) : ConfigOK p (ev.step p o c).1 ∧ OutcomeOK (ev.step p o c).2 := by
  cases ev with
  | dispatch target event payload => exact dispatch_sound hp hc rfl
  | advance t => exact advance_sound hp hc rfl

end Contract
