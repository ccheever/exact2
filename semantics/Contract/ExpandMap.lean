/-
The substitution the expander builds for a use (`Contract.Expand.baseMap`,
`withDerives`), name by name: what `Contract.ExpandInstance.prop_reads`
and `state_reads` assume of it, proved of the expander's own functions.

`map_prop`: a prop's replacement is its argument substituted at the use
site (typed by the declaration when it is a bare `none` or `[]`).
`map_state`: a state's replacement is its lifted name `x#n`. Both for a
component whose props, injects, states, actions and derives have distinct
names (the checker refuses the rest).
-/
import Contract.ExpandInstance

namespace Contract.ExpandMap

open Contract Components Expand ExpandSubst

theorem find?_map_key {m : SMap} {k : String} (h : m.any (·.1 == k) = true) (v : Expr) :
    (m.map fun (a, b) => if a == k then (a, v) else (a, b)).find? (·.1 == k) = .some (k, v) := by
  induction m with
  | nil => simp at h
  | cons p m ih =>
    obtain ⟨a, b⟩ := p
    rw [List.map_cons, List.find?_cons]
    by_cases hak : a = k
    · subst hak; simp
    · have h' : m.any (·.1 == k) = true := by simpa [hak] using h
      have hb : (a == k) = false := by simpa using hak
      simp only [hb, Bool.false_eq_true, if_false]
      exact ih h'

theorem find?_map_other {m : SMap} {k k' : String} (hne : k' ≠ k) (v : Expr) :
    (m.map fun (a, b) => if a == k then (a, v) else (a, b)).find? (·.1 == k') = m.find? (·.1 == k') := by
  induction m with
  | nil => rfl
  | cons p m ih =>
    obtain ⟨a, b⟩ := p
    rw [List.map_cons, List.find?_cons, List.find?_cons]
    by_cases hak : a = k
    · subst hak
      have : (a == k') = false := by simp; exact fun h => hne h.symm
      simp only [beq_self_eq_true, if_true, this]
      exact ih
    · have hb : (a == k) = false := by simpa using hak
      simp only [hb, Bool.false_eq_true, if_false]
      split <;> simp_all

theorem get?_insert_self (m : SMap) (k : String) (v : Expr) : (m.insert k v).get? k = .some v := by
  unfold SMap.insert SMap.get?
  split
  · rename_i h; rw [find?_map_key h]; rfl
  · rename_i h
    have hn : m.find? (·.1 == k) = .none := by
      rw [List.find?_eq_none]; intro p hp hpk
      exact h (List.any_eq_true.2 ⟨p, hp, hpk⟩)
    simp [List.find?_append, hn]

theorem get?_insert_other (m : SMap) {k k' : String} (v : Expr) (hne : k' ≠ k) :
    (m.insert k v).get? k' = m.get? k' := by
  unfold SMap.insert SMap.get?
  split
  · rw [find?_map_other hne]
  · simp only [List.find?_append]
    cases m.find? (·.1 == k') with
    | some p => rfl
    | none =>
      have : (k == k') = false := by simp; exact fun h => hne h.symm
      simp [this]

theorem get?_insertAll_not (m : SMap) {k : String} :
    ∀ (kvs : List (String × Expr)), (∀ p ∈ kvs, p.1 ≠ k) → (insertAll m kvs).get? k = m.get? k
  | [], _ => rfl
  | (k', v) :: kvs, h => by
    simp only [insertAll]
    rw [get?_insertAll_not _ kvs (fun p hp => h p (by simp [hp])),
      get?_insert_other m v (fun he => h (k', v) (by simp) he.symm)]

theorem get?_insertAll_mem (m : SMap) {k : String} {v : Expr} :
    ∀ (kvs : List (String × Expr)), (kvs.map (·.1)).Nodup → (k, v) ∈ kvs →
      (insertAll m kvs).get? k = .some v
  | [], _, h => by simp at h
  | (k', v') :: kvs, hn, h => by
    simp only [List.map_cons, List.nodup_cons, List.mem_map] at hn
    simp only [insertAll]
    simp only [List.mem_cons, Prod.mk.injEq] at h
    rcases h with ⟨rfl, rfl⟩ | h
    · rw [get?_insertAll_not _ kvs (fun p hp hpk => hn.1 ⟨p, hp, hpk⟩), get?_insert_self]
    · exact get?_insertAll_mem _ kvs hn.2 h

/-- The value `propArgs` gives a prop. -/
def propValue (s : Subst) (pd : PropDecl) (a : Expr) : Expr :=
  let v := substExpr s a
  if pd.declared && untypedLeaf v then .typed v pd.ty else v

theorem propArgs_mem {s : Subst} {c : CComponent} {args : List (String × Expr)}
    {pa : List (String × Expr)} (h : propArgs s c args = .ok pa) {pd : PropDecl} (hpd : pd ∈ c.props)
    {a : Expr} (ha : (args.find? (·.1 == pd.name)).map (·.2) = .some a) :
    (pd.name, propValue s pd a) ∈ pa := by
  unfold propArgs at h
  generalize c.props = ps at h hpd
  induction ps generalizing pa with
  | nil => simp at hpd
  | cons q ps ih =>
    simp only [List.mapM_cons, bind, Except.bind] at h
    split at h
    · cases h
    · rename_i x hq
      split at h
      · cases h
      · rename_i xs hxs
        cases h
        simp only [List.mem_cons] at hpd ⊢
        rcases hpd with rfl | hpd
        · left
          revert hq; simp only [ha]; intro hq; cases hq; rfl
        · right; exact ih hxs hpd

theorem propArgs_keys {s : Subst} {c : CComponent} {args : List (String × Expr)}
    {pa : List (String × Expr)} (h : propArgs s c args = .ok pa) : pa.map (·.1) = c.props.map (·.name) := by
  unfold propArgs at h
  generalize c.props = ps at h
  induction ps generalizing pa with
  | nil => simp [pure, Except.pure] at h; subst h; rfl
  | cons q ps ih =>
    simp only [List.mapM_cons, bind, Except.bind] at h
    split at h
    · cases h
    · rename_i x hq
      split at h
      · cases h
      · rename_i xs hxs
        cases h
        simp only [List.map_cons, ih hxs, List.cons.injEq, and_true]
        revert hq; split <;> intro hq <;> cases hq; rfl

theorem injectArgs_keys {provides : List (String × Expr)} {c : CComponent} {ia : List (String × Expr)}
    (h : injectArgs provides c = .ok ia) : ia.map (·.1) = c.injects.map (·.name) := by
  unfold injectArgs at h
  generalize c.injects = ps at h
  induction ps generalizing ia with
  | nil => simp [pure, Except.pure] at h; subst h; rfl
  | cons q ps ih =>
    simp only [List.mapM_cons, bind, Except.bind] at h
    split at h
    · cases h
    · rename_i x hq
      split at h
      · cases h
      · rename_i xs hxs
        cases h
        simp only [List.map_cons, ih hxs, List.cons.injEq, and_true]
        revert hq; split <;> intro hq <;> cases hq; rfl

theorem subst_get_nil {m : SMap} {records : List String} {k : String} :
    ({ map := m, records } : Subst).get k = (m.get? k).map replOf := by
  simp [Subst.get]

/-- Distinct names across a component's props, injects, states, actions
and derives (what the checker's duplicate-name refusals guarantee). -/
structure Distinct (c : CComponent) : Prop where
  props : (c.props.map (·.name)).Nodup
  states : (c.states.map (·.name)).Nodup
  pi : ∀ x ∈ c.props.map (·.name), x ∉ c.injects.map (·.name)
  ps : ∀ x ∈ c.props.map (·.name), x ∉ c.states.map (·.name)
  pa : ∀ x ∈ c.props.map (·.name), x ∉ c.actions.map (·.name)
  pd : ∀ x ∈ c.props.map (·.name), x ∉ c.derives.map (·.name)
  sa : ∀ x ∈ c.states.map (·.name), x ∉ c.actions.map (·.name)
  sd : ∀ x ∈ c.states.map (·.name), x ∉ c.derives.map (·.name)

/-- **A prop's replacement** in the expander's substitution for a use: its
argument substituted at the use site, typed by the declaration when a
bare `none` or `[]`. -/
theorem map_prop {records : List String} {s : Subst} {c : CComponent}
    {args provides : List (String × Expr)} {n : Nat} {m : SMap} (hm : baseMap s c args provides n = .ok m)
    (hd : Distinct c) {acts derives : List (String × Expr)}
    (hacts : acts.map (·.1) = c.actions.map (·.name)) (hdv : ∀ p ∈ derives, p.1 ∈ c.derives.map (·.name))
    {pd : PropDecl} (hpd : pd ∈ c.props) {a : Expr} (ha : (args.find? (·.1 == pd.name)).map (·.2) = .some a) :
    ({ map := withDerives records (insertAll m acts) derives, records } : Subst).get pd.name =
      .some (replOf (propValue s pd a)) := by
  simp only [baseMap, bind, Except.bind] at hm
  split at hm
  · cases hm
  · rename_i pa hpa
    split at hm
    · cases hm
    · rename_i ia hia
      cases hm
      have hpn : pd.name ∈ c.props.map (·.name) := List.mem_map_of_mem hpd
      rw [subst_get_nil, withDerives, get?_insertAll_not _ _ (fun p hp hpk => by
          simp only [List.mem_map] at hp; obtain ⟨⟨d, e⟩, hde, rfl⟩ := hp
          simp only at hpk
          exact hd.pd _ hpn (hpk ▸ hdv _ hde))]
      rw [get?_insertAll_not _ _ (fun p hp hpk => by
          exact hd.pa _ hpn (by rw [← hpk, ← hacts]; exact List.mem_map_of_mem hp))]
      rw [get?_insertAll_not _ _ (fun p hp hpk => by
          simp only [List.mem_map] at hp; obtain ⟨st, hst, rfl⟩ := hp
          simp only at hpk
          exact hd.ps _ hpn (hpk ▸ List.mem_map_of_mem hst))]
      rw [get?_insertAll_not _ _ (fun p hp hpk => by
          exact hd.pi _ hpn (by rw [← hpk, ← injectArgs_keys hia]; exact List.mem_map_of_mem hp))]
      rw [get?_insertAll_mem _ _ (by rw [propArgs_keys hpa]; exact hd.props) (propArgs_mem hpa hpd ha)]
      rfl

/-- **A state's replacement** in the expander's substitution for a use:
its lifted name `x#n`. -/
theorem map_state {records : List String} {s : Subst} {c : CComponent}
    {args provides : List (String × Expr)} {n : Nat} {m : SMap} (hm : baseMap s c args provides n = .ok m)
    (hd : Distinct c) {acts derives : List (String × Expr)}
    (hacts : acts.map (·.1) = c.actions.map (·.name)) (hdv : ∀ p ∈ derives, p.1 ∈ c.derives.map (·.name))
    {st : StateDecl} (hst : st ∈ c.states) :
    ({ map := withDerives records (insertAll m acts) derives, records } : Subst).get st.name =
      .some (.name (lifted st.name n)) := by
  simp only [baseMap, bind, Except.bind] at hm
  split at hm
  · cases hm
  · rename_i pa hpa
    split at hm
    · cases hm
    · rename_i ia hia
      cases hm
      have hsn : st.name ∈ c.states.map (·.name) := List.mem_map_of_mem hst
      rw [subst_get_nil, withDerives, get?_insertAll_not _ _ (fun p hp hpk => by
          simp only [List.mem_map] at hp; obtain ⟨⟨d, e⟩, hde, rfl⟩ := hp
          simp only at hpk
          exact hd.sd _ hsn (hpk ▸ hdv _ hde))]
      rw [get?_insertAll_not _ _ (fun p hp hpk => by
          exact hd.sa _ hsn (by rw [← hpk, ← hacts]; exact List.mem_map_of_mem hp))]
      have hstates : ((c.states.map fun st => (st.name, Expr.var (lifted st.name n))).map (·.1)).Nodup := by
        simp only [List.map_map]
        exact hd.states
      rw [get?_insertAll_mem _ _ hstates (List.mem_map_of_mem (f := fun st => (st.name, Expr.var (lifted st.name n))) hst)]
      rfl

open CompSem ExpandInstance

theorem not_any_of_not_mem {α} {l : List α} {f : α → String} {x : String} (h : x ∉ l.map f) :
    ¬ l.any (fun a => f a == x) := by
  intro ha
  obtain ⟨a, hal, hax⟩ := List.any_eq_true.1 ha
  exact h (List.mem_map.2 ⟨a, hal, by simpa using hax⟩)

/-- **A use's prop corresponds** under the expander's own substitution for
the use, when its argument corresponds at the use site: the frame's prop
reads, at some fuel, exactly what the substituted argument reads in the
flat environment. -/
theorem use_prop {records : List String} {s : Subst} {c : CComponent}
    {args provides : List (String × Expr)} {n : Nat} {m : SMap} (hm : baseMap s c args provides n = .ok m)
    (hd : Distinct c) {acts derives : List (String × Expr)}
    (hacts : acts.map (·.1) = c.actions.map (·.name)) (hdv : ∀ p ∈ derives, p.1 ∈ c.derives.map (·.name))
    {pd : PropDecl} (hpd : pd ∈ c.props) {a : Expr} (ha : (args.find? (·.1 == pd.name)).map (·.2) = .some a)
    {ce : CEnv} {cname : String} {id : InstId} {binds : List (String × Expr × Frame × Locals)}
    (hC : ce.comp cname = .ok c) {fp : Frame} {lp : Locals} (hb : lookupBind pd.name binds = .some (a, fp, lp))
    {fe : Env} {Lf : Locals}
    (harg : ∀ v, (∃ k, ceval k ce fp lp a = .ok v) ↔ EvalR fe false Lf (substExpr s a) v) :
    ∀ v, (∃ k, cvar k ce cname id binds pd.name = .ok v) ↔
      Reads fe Lf ({ map := withDerives records (insertAll m acts) derives, records } : Subst) pd.name v := by
  have hpn : pd.name ∈ c.props.map (·.name) := List.mem_map_of_mem hpd
  refine prop_reads hC (not_any_of_not_mem (hd.pd _ hpn)) (not_any_of_not_mem (hd.ps _ hpn)) hb ?_ harg
  rw [map_prop hm hd hacts hdv hpd ha]
  by_cases hw : (pd.declared && untypedLeaf (substExpr s a)) = true
  · exact .inr ⟨pd.ty, by simp [propValue, hw]⟩
  · exact .inl (by simp [propValue, hw])

/-- **A use's state corresponds** under the expander's own substitution for
the use, when the flat environment reads its lifted slot `x#n` as the
instance's store holds `x`. -/
theorem use_state {records : List String} {s : Subst} {c : CComponent}
    {args provides : List (String × Expr)} {n : Nat} {m : SMap} (hm : baseMap s c args provides n = .ok m)
    (hd : Distinct c) {acts derives : List (String × Expr)}
    (hacts : acts.map (·.1) = c.actions.map (·.name)) (hdv : ∀ p ∈ derives, p.1 ∈ c.derives.map (·.name))
    {st : StateDecl} (hst : st ∈ c.states)
    {ce : CEnv} {cname : String} {id : InstId} {binds : List (String × Expr × Frame × Locals)}
    (hC : ce.comp cname = .ok c) {fe : Env} {Lf : Locals}
    (hslot : ∀ v, (∃ sl, ce.store.find id = .some sl ∧ lookup st.name sl = .some v) ↔
      ExpandSubst.Res fe Lf (lifted st.name n) v) :
    ∀ v, (∃ k, cvar k ce cname id binds st.name = .ok v) ↔
      Reads fe Lf ({ map := withDerives records (insertAll m acts) derives, records } : Subst) st.name v := by
  have hsn : st.name ∈ c.states.map (·.name) := List.mem_map_of_mem hst
  refine state_reads hC (not_any_of_not_mem (hd.sd _ hsn))
    (List.any_eq_true.2 ⟨st, hst, by simp⟩) (map_state hm hd hacts hdv hst) hslot

end Contract.ExpandMap
