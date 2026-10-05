/-
The component-level semantics against the expansion, one instance at a
time: the core of "expansion preserves meaning".

`instance_iff`: in an instance's frame, an expression of the fragment has
value `v` (`Contract.CompSem.ceval`, at some fuel) exactly when the
expression the expander makes of it (`Contract.Expand.substExpr` with the
use's substitution) has value `v` in the flat semantics — provided each of
the frame's names reads what its replacement reads (`Corr`). It composes
`Contract.ExpandFrame.frame_iff` (a frame is an environment) with
`Contract.ExpandSubst.subst_iff` (substitution is capture-avoiding and
meaning-preserving).

`Corr` holds name by name for the substitution the expander builds
(`Contract.Expand.baseMap`, `withDerives`): `prop_reads` (a prop reads its
argument, in the use site's frame, exactly as the substituted argument
reads in the flat environment, given that the use site's frame and the
flat environment correspond there), `state_reads` (a state reads its
instance's store exactly as the lifted slot `x#n` reads in the flat
environment, given that they hold the same value) and `derive_reads` (a
derive reads its body in the frame exactly as its resolved, substituted
body reads, given that resolution kept the body's meaning).
-/
import Contract.ExpandFrame
import Contract.ExpandSubstRev

namespace Contract.ExpandInstance

open Contract Components CompSem CompSemFacts Expand ExpandSubst ExpandFrame

theorem Same.trans {e₁ e₂ e₃ : Env} (h₁ : Same e₁ e₂) (h₂ : Same e₂ e₃) : Same e₁ e₃ :=
  ⟨h₁.fns.trans h₂.fns, h₁.shapes.trans h₂.shapes, h₁.routes.trans h₂.routes,
    h₁.strings.trans h₂.strings, h₁.now.trans h₂.now, h₁.resources.trans h₂.resources,
    h₁.resVals.trans h₂.resVals⟩

/-- The frame's names read, in the flat environment and locals, what their
replacements in `cs` read. -/
def Corr (ce : CEnv) (c : String) (id : InstId) (binds : List (String × Expr × Frame × Locals))
    (fe : Env) (Lf : Locals) (cs : Subst) (names : List String) : Prop :=
  ∀ y ∈ names, ∀ v, (∃ n, cvar n ce c id binds y = .ok v) ↔ Reads fe Lf cs y v

/-- **An instance's expression means what its expansion means.** In the
fragment (`Plain`, no call head substituted, no replacement a callback,
every free name one of the frame's), when the frame's names correspond
to their replacements, an expression has value `v` in the instance's
frame exactly when its substitution has value `v` in the flat
environment. -/
theorem instance_iff {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C)
    {fe : Env} {Lf : Locals} (hsame : Same ce.root fe) {cs : Subst} (hm : MapSmall cs)
    (hn : MapNoArrow cs) {e : Expr} (hp : Plain e = true) (hs : Scoped C binds [] (fv e))
    (hh : ∀ n ∈ callHeads e, cs.get n = .none) (hcorr : Corr ce c id binds fe Lf cs (fv e)) {v : Value} :
    (∃ n, ceval n ce (.inst c id binds) [] e = .ok v) ↔ EvalR fe false Lf (substExpr cs e) v := by
  rw [frame_iff hC hp hs]
  refine subst_iff (Same.trans (frameEnv_same ce C c id binds).symm hsame) hm hn ?_ hp hh
  intro y hy w
  have hmem : y ∈ frameNames C binds := hs y hy (by simp [lookup])
  rw [← hcorr y hy w, ← frameEnv_global hmem]
  simp [Res, lookup]

/-! ## Each of the frame's names -/

theorem reads_expr {fe : Env} {Lf : Locals} {cs : Subst} {y : String} {r : Expr} {v : Value}
    (hg : cs.get y = .some (replOf r)) : Reads fe Lf cs y v ↔ EvalR fe false Lf r v := by
  cases r <;> simp only [Reads, hg, replOf] <;> first | rfl | exact var_iff.symm

theorem typed_iff {fe : Env} {Lf : Locals} {e : Expr} {t : Ty} {v : Value} :
    EvalR fe false Lf (.typed e t) v ↔ EvalR fe false Lf e v :=
  ⟨fun h => by cases h; assumption, fun h => .typed h⟩

/-- A prop reads its argument in the use site's frame and locals. -/
theorem cvar_prop {n : Nat} {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {p : String}
    {a : Expr} {fp : Frame} {lp : Locals} (hd : ¬ C.derives.any (·.name == p))
    (hst : ¬ C.states.any (·.name == p)) (hb : lookupBind p binds = .some (a, fp, lp)) {v : Value} :
    cvar (n + 1) ce c id binds p = .ok v ↔ ceval n ce fp lp a = .ok v := by
  have hd' : C.derives.find? (·.name == p) = .none := by
    rw [List.find?_eq_none]; intro d hdm hdp; exact hd (List.any_eq_true.2 ⟨d, hdm, hdp⟩)
  simp only [cvar, hC, Except.bind_ok_iff]
  constructor
  · rintro ⟨C', hC', h⟩
    cases hC'
    simpa [hd', hst, hb] using h
  · intro h
    exact ⟨C, rfl, by simpa [hd', hst, hb] using h⟩

/-- **A prop corresponds** when its argument does: the argument, read in
the use site's frame, has the values its substitution has in the flat
environment, and the prop's replacement is that substitution (a `typed`
around it changes no value). -/
theorem prop_reads {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {p : String}
    {a : Expr} {fp : Frame} {lp : Locals} (hd : ¬ C.derives.any (·.name == p))
    (hst : ¬ C.states.any (·.name == p)) (hb : lookupBind p binds = .some (a, fp, lp))
    {fe : Env} {Lf : Locals} {cs : Subst} {r : Expr}
    (hg : cs.get p = .some (replOf r) ∨ ∃ t, cs.get p = .some (replOf (.typed r t)))
    (harg : ∀ v, (∃ n, ceval n ce fp lp a = .ok v) ↔ EvalR fe false Lf r v) :
    ∀ v, (∃ n, cvar n ce c id binds p = .ok v) ↔ Reads fe Lf cs p v := by
  intro v
  have hr : Reads fe Lf cs p v ↔ EvalR fe false Lf r v := by
    rcases hg with hg | ⟨t, hg⟩
    · exact reads_expr hg
    · rw [reads_expr hg, typed_iff]
  rw [hr, ← harg v]
  constructor
  · rintro ⟨n, h⟩
    cases n with
    | zero => simp [cvar] at h
    | succ n => exact ⟨n, (cvar_prop hC hd hst hb).1 h⟩
  · rintro ⟨n, h⟩
    exact ⟨n + 1, (cvar_prop hC hd hst hb).2 h⟩

/-- A state reads its instance's store. -/
theorem cvar_state {n : Nat} {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {x : String}
    (hd : ¬ C.derives.any (·.name == x)) (hst : C.states.any (·.name == x)) {v : Value} :
    cvar (n + 1) ce c id binds x = .ok v ↔ ∃ s, ce.store.find id = .some s ∧ lookup x s = .some v := by
  have hd' : C.derives.find? (·.name == x) = .none := by
    rw [List.find?_eq_none]; intro d hdm hdp; exact hd (List.any_eq_true.2 ⟨d, hdm, hdp⟩)
  simp only [cvar, hC, Except.bind_ok_iff]
  constructor
  · rintro ⟨C', hC', h⟩
    cases hC'
    simp only [hd', hst, ite_true] at h
    split at h
    · rename_i s hs
      exact ⟨s, hs, Option.elim_err_ok.mp h⟩
    · simp at h
  · rintro ⟨s, hs, hv⟩
    refine ⟨C, rfl, ?_⟩
    simp [hd', hst, hs, hv]

/-- **A state corresponds** when its lifted slot does: the flat
environment reads `x#n` as the value the instance's store holds for `x`,
and the state's replacement is `x#n`. -/
theorem state_reads {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {x : String}
    (hd : ¬ C.derives.any (·.name == x)) (hst : C.states.any (·.name == x))
    {fe : Env} {Lf : Locals} {cs : Subst} {m : String} (hg : cs.get x = .some (.name m))
    (hslot : ∀ v, (∃ s, ce.store.find id = .some s ∧ lookup x s = .some v) ↔ Res fe Lf m v) :
    ∀ v, (∃ n, cvar n ce c id binds x = .ok v) ↔ Reads fe Lf cs x v := by
  intro v
  simp only [Reads, hg]
  rw [← hslot v]
  constructor
  · rintro ⟨n, h⟩
    cases n with
    | zero => simp [cvar] at h
    | succ n => exact (cvar_state hC hd hst).1 h
  · intro h
    exact ⟨1, (cvar_state (n := 0) hC hd hst).2 h⟩

/-- A derive reads its body in the frame, with no locals. -/
theorem cvar_derive {n : Nat} {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {d : String}
    {decl : DeriveDecl} (hd : C.derives.find? (·.name == d) = .some decl) {v : Value} :
    cvar (n + 1) ce c id binds d = .ok v ↔ ceval n ce (.inst c id binds) [] decl.body = .ok v := by
  simp only [cvar, hC, Except.bind_ok_iff]
  constructor
  · rintro ⟨C', hC', h⟩; cases hC'; simpa [hd] using h
  · intro h; exact ⟨C, rfl, by simpa [hd] using h⟩

/-- **A derive corresponds** when its resolved body does: the
replacement the expander made of the derive reads in the flat environment
what the derive's body reads in the frame. (`instance_iff` gives that for
a body the expander keeps — resolution leaves a body that reads no other
derive and binds nothing as it is — substituted against the frame's other
names.) -/
theorem derive_reads {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {d : String}
    {decl : DeriveDecl} (hd : C.derives.find? (·.name == d) = .some decl)
    {fe : Env} {Lf : Locals} {cs : Subst} {r : Expr} (hg : cs.get d = .some (replOf r))
    (hbody : ∀ v, (∃ n, ceval n ce (.inst c id binds) [] decl.body = .ok v) ↔ EvalR fe false Lf r v) :
    ∀ v, (∃ n, cvar n ce c id binds d = .ok v) ↔ Reads fe Lf cs d v := by
  intro v
  rw [reads_expr hg, ← hbody v]
  constructor
  · rintro ⟨n, h⟩
    cases n with
    | zero => simp [cvar] at h
    | succ n => exact ⟨n, (cvar_derive hC hd).1 h⟩
  · rintro ⟨n, h⟩
    exact ⟨n + 1, (cvar_derive hC hd).2 h⟩

end Contract.ExpandInstance
