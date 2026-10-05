/-
An instance's frame as an environment. In an instance's frame the
component-level evaluator (`Contract.CompSem.ceval`) is the flat one
(`Contract.Big.EvalR`) over an environment whose component-level names are
the frame's: each derive, state, prop and inject with the value it reads
in the frame (`frameEnv`). `frame_iff`: an expression of the fragment has
value `v` in the frame exactly when it has value `v` there.

So everything `Contract.ExpandSubst` proves of the flat semantics applies
to an instance: substituting, for each of the frame's names, an expression
with the same value is meaning-preserving.
-/
import Contract.ExpandSubst
import Contract.CompSemFacts

namespace Contract.ExpandFrame

open Contract Components CompSem CompSemFacts Expand ExpandSubst

/-- The names an instance's frame reads besides locals: its component's
derives and states, and its props and injects. -/
def frameNames (C : CComponent) (binds : List (String × Expr × Frame × Locals)) : List String :=
  C.derives.map (·.name) ++ C.states.map (·.name) ++ binds.map (·.1)

open Classical in
/-- What name `x` reads in the frame, if it reads anything. -/
noncomputable def frameVal (ce : CEnv) (c : String) (id : InstId)
    (binds : List (String × Expr × Frame × Locals)) (x : String) : Option Value :=
  if h : ∃ v, ∃ n, cvar n ce c id binds x = .ok v then .some (Classical.choose h) else .none

theorem frameVal_iff {ce c id binds x v} :
    frameVal ce c id binds x = .some v ↔ ∃ n, cvar n ce c id binds x = .ok v := by
  unfold frameVal
  split
  · rename_i h
    constructor
    · intro hv; cases hv; exact Classical.choose_spec h
    · rintro ⟨n, hn⟩
      obtain ⟨m, hm⟩ := Classical.choose_spec h
      exact congrArg _ (cvar_det hm hn)
  · rename_i h
    constructor
    · intro hv; cases hv
    · rintro ⟨n, hn⟩; exact absurd ⟨v, n, hn⟩ h

/-- The frame as a flat environment: its names as derives holding their
values; the rest (shapes, `fn`s, routes, resources, the clock) the root's. -/
noncomputable def frameEnv (ce : CEnv) (C : CComponent) (c : String) (id : InstId)
    (binds : List (String × Expr × Frame × Locals)) : Env :=
  { prog := { shapes := ce.root.prog.shapes, fns := ce.root.prog.fns, routes := ce.root.prog.routes,
              strings := ce.root.prog.strings, resources := ce.root.prog.resources,
              derives := (frameNames C binds).map fun x => { name := x, ty := .unknown, body := .none } },
    slots := [],
    derives := (frameNames C binds).filterMap fun x => (frameVal ce c id binds x).map (x, ·),
    resources := ce.root.resources, now := ce.root.now }

theorem frameEnv_same (ce : CEnv) (C : CComponent) (c : String) (id : InstId)
    (binds : List (String × Expr × Frame × Locals)) : Same ce.root (frameEnv ce C c id binds) :=
  ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem lookup_filterMap {f : String → Option Value} {x : String} :
    ∀ (ns : List String), lookup x (ns.filterMap fun y => (f y).map (y, ·)) = if x ∈ ns then f x else .none
  | [] => by simp [lookup]
  | y :: ns => by
    simp only [List.filterMap_cons, List.mem_cons]
    cases hf : f y with
    | none =>
      simp only [Option.map_none, lookup_filterMap ns]
      by_cases hxy : x = y
      · subst hxy; simp [hf]
      · simp [hxy]
    | some w =>
      simp only [Option.map_some, lookup, lookup_filterMap ns]
      by_cases hxy : x = y
      · subst hxy; simp [hf]
      · simp [hxy, beq_iff_eq]

/-- A frame name reads in the frame environment what it reads in the frame. -/
theorem frameEnv_global {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} {x : String} {v : Value}
    (hx : x ∈ frameNames C binds) :
    (frameEnv ce C c id binds).global x = .ok v ↔ ∃ n, cvar n ce c id binds x = .ok v := by
  have hany : ((frameNames C binds).map fun x => ({ name := x, ty := .unknown, body := .none } : DeriveDecl)).any
      (·.name == x) = true := by
    simp only [List.any_map, List.any_eq_true, Function.comp, beq_iff_eq]
    exact ⟨x, hx, rfl⟩
  simp only [Env.global, frameEnv, List.find?_nil, hany, ite_true, lookup_filterMap, hx]
  rw [← frameVal_iff]
  cases frameVal ce c id binds x <;> simp

/-- A name the frame reads is one of its names. -/
theorem cvar_mem {n ce c id binds x v} {C : CComponent} (hC : ce.comp c = .ok C)
    (h : cvar n ce c id binds x = .ok v) : x ∈ frameNames C binds := by
  cases n with
  | zero => simp [cvar] at h
  | succ n =>
    simp only [cvar, hC, Except.bind_ok_iff] at h
    obtain ⟨C', hC', h⟩ := h
    cases hC'
    simp only [frameNames, List.mem_append, List.mem_map]
    split at h
    · rename_i d hd
      exact .inl (.inl ⟨d, List.mem_of_find?_eq_some hd, by simpa using List.find?_some hd⟩)
    · split at h
      · rename_i hs
        simp only [List.any_eq_true, beq_iff_eq] at hs
        obtain ⟨s, hs, rfl⟩ := hs
        exact .inl (.inr ⟨s, hs, rfl⟩)
      · split at h
        · rename_i e f' ls' hb
          right
          clear h
          induction binds with
          | nil => simp [lookupBind] at hb
          | cons b bs ih =>
            obtain ⟨y, e0, f0, l0⟩ := b
            simp only [lookupBind] at hb
            split at hb
            · rename_i hy; exact ⟨(y, e0, f0, l0), by simp, by simp at hy; exact hy.symm⟩
            · obtain ⟨b', hb', hb'x⟩ := ih hb; exact ⟨b', by simp [hb'], hb'x⟩
        · simp at h

/-! ## The frame's evaluator is the flat one over the frame environment -/

theorem fsound_aux {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) : ∀ n,
    (∀ {ls e v}, ceval n ce (.inst c id binds) ls e = .ok v → Plain e = true →
      EvalR (frameEnv ce C c id binds) false ls e v) ∧
    (∀ {ls es vs}, cevalList n ce (.inst c id binds) ls es = .ok vs → PlainList es = true →
      ListR (frameEnv ce C c id binds) false ls es vs) ∧
    (∀ {ls es ss}, cevalDisplays n ce (.inst c id binds) ls es = .ok ss → PlainList es = true →
      DisplaysR (frameEnv ce C c id binds) false ls es ss) ∧
    (∀ {ls ps body xs i ys}, cevalMap n ce (.inst c id binds) ls ps body xs i = .ok ys →
      Plain body = true → MapR (frameEnv ce C c id binds) false ls ps body xs i ys) ∧
    (∀ {ls ps body xs i ys}, cevalFilter n ce (.inst c id binds) ls ps body xs i = .ok ys →
      Plain body = true → FilterR (frameEnv ce C c id binds) false ls ps body xs i ys) ∧
    (∀ {ls w b fs i vs}, cevalFields n ce (.inst c id binds) ls w b fs i = .ok vs →
      PlainFields w = true → FieldsR (frameEnv ce C c id binds) false ls w b fs i vs)
  | 0 => by
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ <;> intros <;>
      simp_all [ceval, cevalList, cevalDisplays, cevalMap, cevalFilter, cevalFields]
  | n + 1 => by
    obtain ⟨ihE, ihL, ihD, ihM, ihF, ihR⟩ := fsound_aux hC n
    have hs := frameEnv_same ce C c id binds
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
    · intro ls e v h hp
      cases e with
      | num => simp [ceval] at h; subst h; exact .num
      | str => simp [ceval] at h; subst h; exact .str
      | bool => simp [ceval] at h; subst h; exact .bool
      | none => simp [ceval] at h; subst h; exact .none
      | emptyList => simp [ceval] at h; subst h; exact .emptyList
      | some e =>
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h; simp at h2; subst h2
        exact .some (ihE h1 (by simpa [Plain] using hp))
      | template parts =>
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h; simp at h2; subst h2
        exact .template (ihD h1 (by simpa [Plain] using hp))
      | var x =>
        simp only [ceval] at h
        split at h
        next w hl => simp at h; subst h; exact .local hl
        next hl => exact .global hl rfl ((frameEnv_global (cvar_mem hC h)).2 ⟨n, h⟩)
      | member e f =>
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h
        split at h2
        next s fs =>
          split at h2
          next i hi => exact .member (ihE h1 (by simpa [Plain] using hp)) (hs.fieldIndex _ _ ▸ hi)
                          (Option.elim_err_ok.mp h2)
          next => simp at h2
        next => simp at h2
      | call name args =>
        have hp' : name ≠ "pending" ∧ name ≠ "failed" ∧ PlainList args = true := by
          simp only [Plain, Bool.and_eq_true, bne_iff_ne, ne_eq] at hp; exact ⟨hp.1.1, hp.1.2, hp.2⟩
        simp only [ceval] at h
        split at h
        next fd hfd =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨vs, h1, h2⟩ := h
          exact .fn hfd (ihL h1 hp'.2.2) (xferE hs (eval_sound h2) rfl)
        next hfd =>
          split at h
          next l ps body =>
            simp only [Except.bind_ok_iff, Value.asList_ok] at h
            obtain ⟨a, h1, xs, rfl, ys, h3, h4⟩ := h
            simp at h4; subst h4
            obtain ⟨hpl, _, _, hpb⟩ := plain_cb hp
            exact .map hfd (ihE h1 hpl) (ihM h3 hpb)
          next l ps body =>
            simp only [Except.bind_ok_iff, Value.asList_ok] at h
            obtain ⟨a, h1, xs, rfl, ys, h3, h4⟩ := h
            simp at h4; subst h4
            obtain ⟨hpl, _, _, hpb⟩ := plain_cb hp
            exact EvalR.filter hfd (ihE h1 hpl) (ihF h3 hpb)
          next x => exact absurd rfl hp'.1
          next x => exact absurd rfl hp'.2.1
          next =>
            simp only [Except.bind_ok_iff] at h
            obtain ⟨vs, h1, h2⟩ := h
            exact .stdlib hfd (ihL h1 hp'.2.2) (hs.stdlib_eq _ _ ▸ h2)
      | record shape base fields =>
        simp only [ceval] at h
        split at h
        next =>
          simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h
          obtain ⟨_, rfl, decl, hd, vs, h1, h2⟩ := h
          simp at h2; subst h2
          simp only [Plain, Bool.true_and] at hp
          exact .record (hs.shape _ ▸ Option.elim_err_ok.mp hd) (ihR h1 hp)
        next e =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨a, h1, h2⟩ := h
          simp only [Plain, Bool.and_eq_true] at hp
          split at h2
          next s' bs =>
            simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2
            obtain ⟨_, rfl, decl, hd, vs, h3, h4⟩ := h2
            simp at h4; subst h4
            exact .recordBase (ihE h1 hp.1) (hs.shape _ ▸ Option.elim_err_ok.mp hd) (ihR h3 hp.2)
          next => simp at h2
      | unary op e =>
        have hp' : Plain e = true := by simpa [Plain] using hp
        cases op <;> simp only [ceval, Except.bind_ok_iff] at h <;> obtain ⟨a, h1, h2⟩ := h <;>
          split at h2 <;> simp at h2 <;> subst h2
        · exact .neg (ihE h1 hp')
        · exact .not (ihE h1 hp')
      | binary op a b =>
        simp only [Plain, Bool.and_eq_true] at hp
        cases op
        case and =>
          simp only [ceval, Except.bind_ok_iff] at h
          obtain ⟨w, h1, h2⟩ := h
          split at h2
          next => simp at h2; subst h2; exact .andFalse (ihE h1 hp.1)
          next => exact .andTrue (ihE h1 hp.1) (ihE h2 hp.2)
          next => simp at h2
        case or =>
          simp only [ceval, Except.bind_ok_iff] at h
          obtain ⟨w, h1, h2⟩ := h
          split at h2
          next => simp at h2; subst h2; exact .orTrue (ihE h1 hp.1)
          next => exact .orFalse (ihE h1 hp.1) (ihE h2 hp.2)
          next => simp at h2
        all_goals
          rw [ceval_binary_strict (by simp) (by simp)] at h
          simp only [Except.bind_ok_iff] at h
          obtain ⟨va, h1, vb, h2, h3⟩ := h
          exact .binop (ihE h1 hp.1) (ihE h2 hp.2) h3
      | ternary c a b =>
        simp only [Plain, Bool.and_eq_true] at hp
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next => exact .ternaryTrue (ihE h1 hp.1.1) (ihE h2 hp.1.2)
        next => exact .ternaryFalse (ihE h1 hp.1.1) (ihE h2 hp.2)
        next => simp at h2
      | matchOpt s x a b =>
        obtain ⟨hps, _, hpa, hpb⟩ := plain_match hp
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next => exact .matchSome (ihE h1 hps) (ihE h2 hpa)
        next => exact .matchNone (ihE h1 hps) (ihE h2 hpb)
        next => simp at h2
      | arrow => simp [ceval] at h
      | letE x e body =>
        obtain ⟨hpv, _, hpb⟩ := plain_let hp
        simp only [ceval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        exact .letE (ihE h1 hpv) (ihE h2 hpb)
      | named => simp [ceval] at h
      | typed e ty =>
        simp only [ceval] at h
        exact .typed (ihE h (by simpa [Plain] using hp))
    · intro ls es vs h hp
      cases es with
      | nil => simp [cevalList] at h; subst h; exact .nil
      | cons e es =>
        simp only [PlainList, Bool.and_eq_true] at hp
        simp only [cevalList, Except.bind_ok_iff] at h
        obtain ⟨v, h1, vs', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1 hp.1) (ihL h2 hp.2)
    · intro ls es ss h hp
      cases es with
      | nil => simp [cevalDisplays] at h; subst h; exact .nil
      | cons e es =>
        simp only [PlainList, Bool.and_eq_true] at hp
        simp only [cevalDisplays, Except.bind_ok_iff] at h
        obtain ⟨v, h1, s, hs', ss', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1 hp.1) hs' (ihD h2 hp.2)
    · intro ls ps body xs i ys h hp
      cases xs with
      | nil => simp [cevalMap] at h; subst h; exact .nil
      | cons x xs =>
        simp only [cevalMap, Except.bind_ok_iff] at h
        obtain ⟨y, h1, ys', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1 hp) (ihM h2 hp)
    · intro ls ps body xs i ys h hp
      cases xs with
      | nil => simp [cevalFilter] at h; subst h; exact .nil
      | cons x xs =>
        simp only [cevalFilter, Except.bind_ok_iff] at h
        obtain ⟨k, h1, ys', h2, h3⟩ := h
        split at h3
        next => simp at h3; subst h3; exact .keep (ihE h1 hp) (ihF h2 hp)
        next => simp at h3; subst h3; exact .drop (ihE h1 hp) (ihF h2 hp)
        next => simp at h3
    · intro ls w b fs i vs h hp
      cases fs with
      | nil => simp [cevalFields] at h; subst h; exact .nil
      | cons f fs =>
        simp only [cevalFields] at h
        split at h
        next e he =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨v, h1, vs', h2, h3⟩ := h
          simp at h3; subst h3
          obtain ⟨k, hk⟩ := lookupField_mem he
          exact .written he (ihE h1 ((fields_mem hk).2.1 hp)) (ihR h2 hp)
        next he =>
          split at h
          next bs =>
            simp only [Except.bind_ok_iff] at h
            obtain ⟨v, h1, vs', h2, h3⟩ := h
            simp at h3; subst h3
            exact .base he (Option.elim_err_ok.mp h1) (ihR h2 hp)
          next => simp at h

/-- Every free name of `e` is a local or one of the frame's names. -/
def Scoped (C : CComponent) (binds : List (String × Expr × Frame × Locals)) (ls : Locals)
    (names : List String) : Prop :=
  ∀ y ∈ names, lookup y ls = .none → y ∈ frameNames C binds

theorem Scoped.sub {C binds ls} {ns ms : List String} (h : Scoped C binds ls ns) (hm : ∀ y ∈ ms, y ∈ ns) :
    Scoped C binds ls ms := fun y hy hl => h y (hm y hy) hl

theorem Scoped.bindM {C binds ls} {subj : Expr} {x : String} {w : Value} {a b : Expr}
    (h : Scoped C binds ls (fv (.matchOpt subj x a b))) : Scoped C binds ((x, w) :: ls) (fv a) := by
  intro y hy hl
  have hyx : y ≠ x := by rintro rfl; simp [lookup] at hl
  rw [lookup_cons_ne hyx] at hl
  exact h y (by simp [fv, hy, hyx]) hl

theorem Scoped.bindL {C binds ls} {x : String} {w : Value} {v b : Expr}
    (h : Scoped C binds ls (fv (.letE x v b))) : Scoped C binds ((x, w) :: ls) (fv b) := by
  intro y hy hl
  have hyx : y ≠ x := by rintro rfl; simp [lookup] at hl
  rw [lookup_cons_ne hyx] at hl
  exact h y (by simp [fv, hy, hyx]) hl

theorem lookup_bindParams_isSome {ps : List String} {y : String} {x : Value} {i : Nat} {L : Locals}
    (hl : ps.length ≤ 2) (hy : y ∈ ps) : (lookup y (bindParams ps x i L)).isSome := by
  match ps, hl, hy with
  | [p], _, hy => simp at hy; subst hy; simp [bindParams, lookup]
  | [p, q], _, hy =>
    simp only [bindParams, lookup]
    by_cases hq : y = q
    · simp [hq]
    · simp at hy; rcases hy with rfl | rfl
      · simp [hq]
      · exact absurd rfl hq

theorem Scoped.params {C binds ls} {ps : List String} {x : Value} {i : Nat} {e : Expr}
    (hl : ps.length ≤ 2) (h : Scoped C binds ls (fv (.arrow ps e))) :
    Scoped C binds (bindParams ps x i ls) (fv e) := by
  intro y hy hn
  by_cases hp : y ∈ ps
  · have := lookup_bindParams_isSome (x := x) (i := i) (L := ls) hl hp
    rw [hn] at this; cases this
  · rw [lookup_bindParams_not hp] at hn
    exact h y (by simp [fv, hy, hp]) hn

theorem cevalList_mono {n m ce f ls es vs} (hm : n ≤ m) (h : cevalList n ce f ls es = .ok vs) :
    cevalList m ce f ls es = .ok vs := (cmono_aux n).2.2.2.1 hm h
theorem cevalDisplays_mono {n m ce f ls es ss} (hm : n ≤ m) (h : cevalDisplays n ce f ls es = .ok ss) :
    cevalDisplays m ce f ls es = .ok ss := (cmono_aux n).2.2.2.2.1 hm h
theorem cevalMap_mono {n m ce f ls ps body xs i ys} (hm : n ≤ m)
    (h : cevalMap n ce f ls ps body xs i = .ok ys) : cevalMap m ce f ls ps body xs i = .ok ys :=
  (cmono_aux n).2.2.2.2.2.1 hm h
theorem cevalFilter_mono {n m ce f ls ps body xs i ys} (hm : n ≤ m)
    (h : cevalFilter n ce f ls ps body xs i = .ok ys) : cevalFilter m ce f ls ps body xs i = .ok ys :=
  (cmono_aux n).2.2.2.2.2.2.1 hm h
theorem cevalFields_mono {n m ce f ls w b fs i vs} (hm : n ≤ m)
    (h : cevalFields n ce f ls w b fs i = .ok vs) : cevalFields m ce f ls w b fs i = .ok vs :=
  (cmono_aux n).2.2.2.2.2.2.2 hm h

local macro "clift" h:term : term =>
  `(by first
    | exact ceval_mono (by omega) $h | exact cevalList_mono (by omega) $h
    | exact cevalDisplays_mono (by omega) $h | exact cevalMap_mono (by omega) $h
    | exact cevalFilter_mono (by omega) $h | exact cevalFields_mono (by omega) $h
    | exact eval_mono (by omega) $h)

theorem ceval_stdlib_intro {n ce c id binds ls name args vs v}
    (hfd : ce.root.prog.fns.find? (·.name == name) = .none)
    (h1 : cevalList n ce (.inst c id binds) ls args = .ok vs)
    (h2 : stdlib ce.root name vs = .ok v) : ceval (n + 1) ce (.inst c id binds) ls (.call name args) = .ok v := by
  simp only [ceval, hfd]
  split
  next => exact absurd h2 stdlib_map
  next => exact absurd h2 stdlib_filter
  next => exact absurd h2 stdlib_pending
  next => exact absurd h2 stdlib_failed
  next => simp only [Except.bind_ok_iff]; exact ⟨_, h1, h2⟩

section complete
variable {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
  {binds : List (String × Expr × Frame × Locals)}

mutual
theorem fcE (hC : ce.comp c = .ok C) {inFn ls e v} (h : EvalR (frameEnv ce C c id binds) inFn ls e v)
    (hf : inFn = false) (hp : Plain e = true) (hs : Scoped C binds ls (fv e)) :
    ∃ n, ceval n ce (.inst c id binds) ls e = .ok v :=
  match h with
  | .num => ⟨1, rfl⟩
  | .str => ⟨1, rfl⟩
  | .bool => ⟨1, rfl⟩
  | .none => ⟨1, rfl⟩
  | .emptyList => ⟨1, rfl⟩
  | .some h1 => by
    obtain ⟨n, h1⟩ := fcE hC h1 hf (by simpa [Plain] using hp) (by simpa [fv] using hs)
    exact ⟨n + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, h1, rfl⟩⟩
  | .template h1 => by
    obtain ⟨n, h1⟩ := fcD hC h1 hf (by simpa [Plain] using hp) (by simpa [fv] using hs)
    exact ⟨n + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, h1, rfl⟩⟩
  | .local hl => ⟨1, by simp [ceval, hl]⟩
  | .global hl hi hg => by
    rename_i x
    obtain ⟨n, hn⟩ := (frameEnv_global (hs x (by simp [fv]) hl)).1 hg
    exact ⟨n + 1, by simp [ceval, hl, hn]⟩
  | .member h1 hi hv => by
    obtain ⟨n, h1⟩ := fcE hC h1 hf (by simpa [Plain] using hp) (by simpa [fv] using hs)
    have hi' := ((frameEnv_same ce C c id binds).fieldIndex _ _).trans hi
    exact ⟨n + 1, by
      simp only [ceval, Except.bind_ok_iff]
      exact ⟨_, h1, by simp [hi', hv]⟩⟩
  | .fn hfd ha hb => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcL hC ha hf hp.2 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := eval_complete (xferE (frameEnv_same ce C c id binds).symm hb rfl)
    exact ⟨max n1 n2 + 1, by
      simp only [ceval]
      rw [show ce.root.prog.fns = (frameEnv ce C c id binds).prog.fns from rfl, hfd]
      simp only [Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .map hfd hl hm => by
    obtain ⟨hpl, hlen, _, hpb⟩ := plain_cb hp
    obtain ⟨n1, h1⟩ := fcE hC hl hf hpl
      (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    obtain ⟨n2, h2⟩ := fcM hC hm hf hpb hlen
      (hs.sub fun y hy => by simp only [fv, fvList, List.mem_append, List.mem_filter] at hy ⊢; grind)
    exact ⟨max n1 n2 + 1, by
      simp only [ceval]
      rw [show ce.root.prog.fns = (frameEnv ce C c id binds).prog.fns from rfl, hfd]
      simp only [Except.bind_ok_iff]; exact ⟨_, clift h1, _, rfl, _, clift h2, rfl⟩⟩
  | .filter hfd hl hm => by
    obtain ⟨hpl, hlen, _, hpb⟩ := plain_cb hp
    obtain ⟨n1, h1⟩ := fcE hC hl hf hpl
      (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    obtain ⟨n2, h2⟩ := fcF hC hm hf hpb hlen
      (hs.sub fun y hy => by simp only [fv, fvList, List.mem_append, List.mem_filter] at hy ⊢; grind)
    exact ⟨max n1 n2 + 1, by
      simp only [ceval]
      rw [show ce.root.prog.fns = (frameEnv ce C c id binds).prog.fns from rfl, hfd]
      simp only [Except.bind_ok_iff]; exact ⟨_, clift h1, _, rfl, _, clift h2, rfl⟩⟩
  | .pendingSettled .. | .pendingOther .. | .failedSettled .. | .failedOther .. => by
    simp [Plain] at hp
  | .stdlib hfd ha hv => by
    rename_i name args vs
    have hp' : name ≠ "pending" ∧ name ≠ "failed" ∧ PlainList args = true := by
      simp only [Plain, Bool.and_eq_true, bne_iff_ne, ne_eq] at hp; exact ⟨hp.1.1, hp.1.2, hp.2⟩
    obtain ⟨n, h1⟩ := fcL hC ha hf hp'.2.2 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    have hv' : stdlib ce.root name vs = .ok v := (frameEnv_same ce C c id binds).stdlib_eq _ _ ▸ hv
    exact ⟨n + 1, ceval_stdlib_intro hfd h1 hv'⟩
  | .record hd h1 => by
    simp only [Plain, Bool.true_and] at hp
    obtain ⟨n, h1⟩ := fcR hC h1 hf hp (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    have hd' := ((frameEnv_same ce C c id binds).shape _).trans hd
    exact ⟨n + 1, by
      simp only [ceval, Except.bind_ok_iff]
      exact ⟨_, rfl, _, by simp [hd'], _, h1, rfl⟩⟩
  | .recordBase hb hd h1 => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, hb⟩ := fcE hC hb hf hp.1 (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    obtain ⟨n2, h1⟩ := fcR hC h1 hf hp.2 (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    have hd' := ((frameEnv_same ce C c id binds).shape _).trans hd
    exact ⟨max n1 n2 + 1, by
      simp only [ceval, Except.bind_ok_iff]
      exact ⟨_, clift hb, by
        simp only [Except.bind_ok_iff]
        exact ⟨_, rfl, _, by rw [hd']; rfl, _, clift h1, rfl⟩⟩⟩
  | .neg h1 | .not h1 => by
    obtain ⟨n, h1⟩ := fcE hC h1 hf (by simpa [Plain] using hp) (by simpa [fv] using hs)
    exact ⟨n + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, h1, rfl⟩⟩
  | .andFalse h1 | .orTrue h1 => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n, h1⟩ := fcE hC h1 hf hp.1 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    exact ⟨n + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, h1, rfl⟩⟩
  | .andTrue h1 h2 | .orFalse h1 h2 => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hp.2 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .binop h1 h2 h3 => by
    rename_i a va b vb op
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hp.2 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    have hand : op ≠ .and := by rintro rfl; exact binop_and h3
    have hor : op ≠ .or := by rintro rfl; exact binop_or h3
    exact ⟨max n1 n2 + 1, by
      rw [ceval_binary_strict hand hor]
      simp only [Except.bind_ok_iff]; exact ⟨_, clift h1, _, clift h2, h3⟩⟩
  | .ternaryTrue h1 h2 => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1.1 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hp.1.2 (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .ternaryFalse h1 h2 => by
    simp only [Plain, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1.1 (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hp.2 (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .matchSome h1 h2 => by
    obtain ⟨hps, _, hpa, _⟩ := plain_match hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hps (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hpa
      (Scoped.bindM hs)
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .matchNone h1 h2 => by
    obtain ⟨hps, _, _, hpb⟩ := plain_match hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hps (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hpb (hs.sub fun y hy => by simp [fv, fvList, fvOpt, hy])
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .letE h1 h2 => by
    obtain ⟨hpv, _, hpb⟩ := plain_let hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hpv (hs.sub fun y hy => by simp only [fv, fvList, fvFields, fvOpt, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcE hC h2 hf hpb
      (Scoped.bindL hs)
    exact ⟨max n1 n2 + 1, by simp only [ceval, Except.bind_ok_iff]; exact ⟨_, clift h1, clift h2⟩⟩
  | .typed h1 => by
    obtain ⟨n, h1⟩ := fcE hC h1 hf (by simpa [Plain] using hp) (by simpa [fv] using hs)
    exact ⟨n + 1, by simp only [ceval]; exact h1⟩
theorem fcL (hC : ce.comp c = .ok C) {inFn ls es vs} (h : ListR (frameEnv ce C c id binds) inFn ls es vs)
    (hf : inFn = false) (hp : PlainList es = true) (hs : Scoped C binds ls (fvList es)) :
    ∃ n, cevalList n ce (.inst c id binds) ls es = .ok vs :=
  match h with
  | .nil => ⟨1, rfl⟩
  | .cons h1 h2 => by
    simp only [PlainList, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1 (hs.sub fun y hy => by simp only [fvList, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcL hC h2 hf hp.2 (hs.sub fun y hy => by simp only [fvList, List.mem_append] at hy ⊢; grind)
    exact ⟨max n1 n2 + 1, by simp only [cevalList, Except.bind_ok_iff]; exact ⟨_, clift h1, _, clift h2, rfl⟩⟩
theorem fcD (hC : ce.comp c = .ok C) {inFn ls es ss} (h : DisplaysR (frameEnv ce C c id binds) inFn ls es ss)
    (hf : inFn = false) (hp : PlainList es = true) (hs : Scoped C binds ls (fvList es)) :
    ∃ n, cevalDisplays n ce (.inst c id binds) ls es = .ok ss :=
  match h with
  | .nil => ⟨1, rfl⟩
  | .cons h1 hd h2 => by
    simp only [PlainList, Bool.and_eq_true] at hp
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp.1 (hs.sub fun y hy => by simp only [fvList, List.mem_append] at hy ⊢; grind)
    obtain ⟨n2, h2⟩ := fcD hC h2 hf hp.2 (hs.sub fun y hy => by simp only [fvList, List.mem_append] at hy ⊢; grind)
    exact ⟨max n1 n2 + 1, by
      simp only [cevalDisplays, Except.bind_ok_iff]; exact ⟨_, clift h1, _, hd, _, clift h2, rfl⟩⟩
theorem fcM (hC : ce.comp c = .ok C) {inFn ls ps body xs i ys}
    (h : MapR (frameEnv ce C c id binds) inFn ls ps body xs i ys) (hf : inFn = false)
    (hp : Plain body = true) (hl : ps.length ≤ 2) (hs : Scoped C binds ls (fv (.arrow ps body))) :
    ∃ n, cevalMap n ce (.inst c id binds) ls ps body xs i = .ok ys :=
  match h with
  | .nil => ⟨1, rfl⟩
  | .cons h1 h2 => by
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp (Scoped.params hl hs)
    obtain ⟨n2, h2⟩ := fcM hC h2 hf hp hl hs
    exact ⟨max n1 n2 + 1, by simp only [cevalMap, Except.bind_ok_iff]; exact ⟨_, clift h1, _, clift h2, rfl⟩⟩
theorem fcF (hC : ce.comp c = .ok C) {inFn ls ps body xs i ys}
    (h : FilterR (frameEnv ce C c id binds) inFn ls ps body xs i ys) (hf : inFn = false)
    (hp : Plain body = true) (hl : ps.length ≤ 2) (hs : Scoped C binds ls (fv (.arrow ps body))) :
    ∃ n, cevalFilter n ce (.inst c id binds) ls ps body xs i = .ok ys :=
  match h with
  | .nil => ⟨1, rfl⟩
  | .keep h1 h2 | .drop h1 h2 => by
    obtain ⟨n1, h1⟩ := fcE hC h1 hf hp (Scoped.params hl hs)
    obtain ⟨n2, h2⟩ := fcF hC h2 hf hp hl hs
    exact ⟨max n1 n2 + 1, by simp only [cevalFilter, Except.bind_ok_iff]; exact ⟨_, clift h1, _, clift h2, rfl⟩⟩
theorem fcR (hC : ce.comp c = .ok C) {inFn ls w b fs i vs}
    (h : FieldsR (frameEnv ce C c id binds) inFn ls w b fs i vs) (hf : inFn = false)
    (hp : PlainFields w = true) (hs : Scoped C binds ls (fvFields w)) :
    ∃ n, cevalFields n ce (.inst c id binds) ls w b fs i = .ok vs :=
  match h with
  | .nil => ⟨1, rfl⟩
  | .written he h1 h2 => by
    obtain ⟨k, hk⟩ := lookupField_mem he
    obtain ⟨n1, h1⟩ := fcE hC h1 hf ((fields_mem hk).2.1 hp) (hs.sub (fun y hy => fields_fv hk y hy))
    obtain ⟨n2, h2⟩ := fcR hC h2 hf hp hs
    exact ⟨max n1 n2 + 1, by
      simp only [cevalFields, he, Except.bind_ok_iff]; exact ⟨_, clift h1, _, clift h2, rfl⟩⟩
  | .base he hb h2 => by
    obtain ⟨n2, h2⟩ := fcR hC h2 hf hp hs
    exact ⟨n2 + 1, by
      simp only [cevalFields, he, Except.bind_ok_iff]; exact ⟨_, by simp [hb], _, h2, rfl⟩⟩
end

end complete

/-- **An instance's frame is an environment.** In the fragment, an
expression whose free names are locals or the frame's has value `v` in the
frame (at some fuel) exactly when it has value `v` in `frameEnv`. -/
theorem frame_iff {ce : CEnv} {C : CComponent} {c : String} {id : InstId}
    {binds : List (String × Expr × Frame × Locals)} (hC : ce.comp c = .ok C) {ls : Locals} {e : Expr}
    {v : Value} (hp : Plain e = true) (hs : Scoped C binds ls (fv e)) :
    (∃ n, ceval n ce (.inst c id binds) ls e = .ok v) ↔ EvalR (frameEnv ce C c id binds) false ls e v :=
  ⟨fun ⟨n, h⟩ => (fsound_aux hC n).1 h hp, fun h => fcE hC h rfl hp hs⟩

end Contract.ExpandFrame
