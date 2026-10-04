/-
Substitution is meaning-preserving: the capture-avoiding substitution the
expander is built from (`Contract.Expand.substExpr`, subst.rs) commutes
with evaluation.

`subst_iff` (in `Contract.ExpandSubstRev`, with the backward direction;
the forward one, `sfE`, is here): if, for every free name of an
expression, its replacement read in the target environment has exactly the
values the name has in the source environment (`Agree`), then the
expression evaluates to `v` in the source exactly when its substitution
evaluates to `v` in the target. A binder is renamed apart exactly when a
replacement mentions its name, so an argument a parent passes in is never
captured by a child's binder, and a renamed binder never captures a name
of the body: that is what the proof of `agree_enter` checks.

The fragment (`Plain`): no `pending`/`failed` (they read their argument's
*name*, which substitution changes), callbacks of at most two parameters
(the ones `map` and `filter` bind), and no call head the substitution
replaces (a call names a `fn` or a roster entry, never a prop).
-/
import Contract.Expand
import Contract.Big

namespace Contract.ExpandSubst

open Contract Expand

/-! ## Reading a name -/

/-- What `x` reads outside a `fn`: a local, else a component-level name. -/
def Res (env : Env) (ls : Locals) (x : String) (v : Value) : Prop :=
  lookup x ls = .some v ∨ (lookup x ls = .none ∧ env.global x = .ok v)

theorem var_iff {env : Env} {ls x v} : EvalR env false ls (.var x) v ↔ Res env ls x v := by
  constructor
  · intro h
    cases h with
    | «local» h => exact .inl h
    | global h _ hg => exact .inr ⟨h, hg⟩
  · rintro (h | ⟨h, hg⟩)
    · exact .local h
    · exact .global h rfl hg

theorem lookup_cons_ne {x y : String} {w : Value} {ls : Locals} (h : x ≠ y) :
    lookup x ((y, w) :: ls) = lookup x ls := by
  simp [lookup, h]

theorem lookup_cons_eq {x : String} {w : Value} {ls : Locals} : lookup x ((x, w) :: ls) = .some w := by
  simp [lookup]

theorem res_cons_ne {env : Env} {x y : String} {w : Value} {ls : Locals} {v} (h : x ≠ y) :
    Res env ((y, w) :: ls) x v ↔ Res env ls x v := by
  simp [Res, lookup_cons_ne h]

theorem res_cons_eq {env : Env} {x : String} {w : Value} {ls : Locals} {v} :
    Res env ((x, w) :: ls) x v ↔ w = v := by
  simp [Res, lookup_cons_eq]

/-! ## Environments that agree on everything but names -/

/-- Two environments that agree on what an expression reads besides its
component-level names: `fn`s, shapes, the route table, the clock, and the
resources (`pending` reads them by name). -/
structure Same (e₁ e₂ : Env) : Prop where
  fns : e₁.prog.fns = e₂.prog.fns
  shapes : e₁.prog.shapes = e₂.prog.shapes
  routes : e₁.prog.routes = e₂.prog.routes
  strings : e₁.prog.strings = e₂.prog.strings
  now : e₁.now = e₂.now
  resources : e₁.prog.resources = e₂.prog.resources
  resVals : e₁.resources = e₂.resources

theorem Same.symm {e₁ e₂ : Env} (h : Same e₁ e₂) : Same e₂ e₁ :=
  ⟨h.fns.symm, h.shapes.symm, h.routes.symm, h.strings.symm, h.now.symm, h.resources.symm,
    h.resVals.symm⟩

theorem Same.shape {e₁ e₂ : Env} (h : Same e₁ e₂) (s : String) : e₁.shape s = e₂.shape s := by
  simp [Env.shape, h.shapes]

theorem Same.fieldIndex {e₁ e₂ : Env} (h : Same e₁ e₂) (s f : String) :
    e₁.fieldIndex s f = e₂.fieldIndex s f := by
  simp [Env.fieldIndex, h.shape]

theorem Same.stdlib_eq {e₁ e₂ : Env} (h : Same e₁ e₂) (f : String) (vs : List Value) :
    Contract.stdlib e₁ f vs = Contract.stdlib e₂ f vs := by
  unfold Contract.stdlib
  rw [h.now, h.routes, h.strings]

mutual
/-- Inside a `fn` no component-level name is read: environments that agree
on the rest evaluate a body alike. -/
theorem xferE {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls e v}
    (h : EvalR e₁ inFn ls e v) (hf : inFn = true) : EvalR e₂ inFn ls e v :=
  match h with
  | .num => .num
  | .str => .str
  | .bool => .bool
  | .none => .none
  | .emptyList => .emptyList
  | .some h => .some (xferE hs h hf)
  | .template h => .template (xferD hs h hf)
  | .local h => .local h
  | .global _ hi _ => absurd (hi.symm.trans hf) Bool.false_ne_true
  | .member h hi hv => .member (xferE hs h hf) (hs.fieldIndex _ _ ▸ hi) hv
  | .fn hfd ha hb => .fn (hs.fns ▸ hfd) (xferL hs ha hf) (xferE hs hb rfl)
  | .map hfd hl hm => .map (hs.fns ▸ hfd) (xferE hs hl hf) (xferM hs hm hf)
  | .filter hfd hl hm => .filter (hs.fns ▸ hfd) (xferE hs hl hf) (xferF hs hm hf)
  | .pendingSettled hfd hr hv => .pendingSettled (hs.fns ▸ hfd) (hs.resources ▸ hr) (hs.resVals ▸ hv)
  | .pendingOther hfd hr => .pendingOther (hs.fns ▸ hfd) (hs.resources ▸ hr)
  | .failedSettled hfd hr hv => .failedSettled (hs.fns ▸ hfd) (hs.resources ▸ hr) (hs.resVals ▸ hv)
  | .failedOther hfd hr => .failedOther (hs.fns ▸ hfd) (hs.resources ▸ hr)
  | .stdlib hfd ha hv => .stdlib (hs.fns ▸ hfd) (xferL hs ha hf) (hs.stdlib_eq _ _ ▸ hv)
  | .record hd h => .record (hs.shape _ ▸ hd) (xferR hs h hf)
  | .recordBase hb hd h => .recordBase (xferE hs hb hf) (hs.shape _ ▸ hd) (xferR hs h hf)
  | .neg h => .neg (xferE hs h hf)
  | .not h => .not (xferE hs h hf)
  | .andFalse h => .andFalse (xferE hs h hf)
  | .andTrue h1 h2 => .andTrue (xferE hs h1 hf) (xferE hs h2 hf)
  | .orTrue h => .orTrue (xferE hs h hf)
  | .orFalse h1 h2 => .orFalse (xferE hs h1 hf) (xferE hs h2 hf)
  | .binop h1 h2 h3 => .binop (xferE hs h1 hf) (xferE hs h2 hf) h3
  | .ternaryTrue h1 h2 => .ternaryTrue (xferE hs h1 hf) (xferE hs h2 hf)
  | .ternaryFalse h1 h2 => .ternaryFalse (xferE hs h1 hf) (xferE hs h2 hf)
  | .matchSome h1 h2 => .matchSome (xferE hs h1 hf) (xferE hs h2 hf)
  | .matchNone h1 h2 => .matchNone (xferE hs h1 hf) (xferE hs h2 hf)
  | .letE h1 h2 => .letE (xferE hs h1 hf) (xferE hs h2 hf)
  | .typed h => .typed (xferE hs h hf)
theorem xferL {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls es vs}
    (h : ListR e₁ inFn ls es vs) (hf : inFn = true) : ListR e₂ inFn ls es vs :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => .cons (xferE hs h1 hf) (xferL hs h2 hf)
theorem xferD {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls es ss}
    (h : DisplaysR e₁ inFn ls es ss) (hf : inFn = true) : DisplaysR e₂ inFn ls es ss :=
  match h with
  | .nil => .nil
  | .cons h1 hd h2 => .cons (xferE hs h1 hf) hd (xferD hs h2 hf)
theorem xferM {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls ps body xs i ys}
    (h : MapR e₁ inFn ls ps body xs i ys) (hf : inFn = true) : MapR e₂ inFn ls ps body xs i ys :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => .cons (xferE hs h1 hf) (xferM hs h2 hf)
theorem xferF {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls ps body xs i ys}
    (h : FilterR e₁ inFn ls ps body xs i ys) (hf : inFn = true) : FilterR e₂ inFn ls ps body xs i ys :=
  match h with
  | .nil => .nil
  | .keep h1 h2 => .keep (xferE hs h1 hf) (xferF hs h2 hf)
  | .drop h1 h2 => .drop (xferE hs h1 hf) (xferF hs h2 hf)
theorem xferR {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls w b fs i vs}
    (h : FieldsR e₁ inFn ls w b fs i vs) (hf : inFn = true) : FieldsR e₂ inFn ls w b fs i vs :=
  match h with
  | .nil => .nil
  | .written he h1 h2 => .written he (xferE hs h1 hf) (xferR hs h2 hf)
  | .base he hb h2 => .base he hb (xferR hs h2 hf)
end

/-! ## Free names -/

mutual
theorem mem_free : ∀ (e : Expr) (B : List String) (y : String),
    y ∈ freeNames B e ↔ y ∈ freeNames [] e ∧ y ∉ B
  | .var n, B, y => by
    by_cases h : n ∈ B <;> simp [freeNames, h] <;> intro hy <;> subst hy <;> simp_all
  | .call n args, B, y => by
    have := mem_freeList args B y
    by_cases h : n ∈ B <;> simp [freeNames, h, this] <;> grind
  | .record s base fields, B, y => by
    have hf := mem_freeFields fields B y
    cases base with
    | none =>
      by_cases h : s ∈ B <;> simp [freeNames, h, hf] <;> grind
    | some b =>
      have := mem_free b B y
      have h0 := mem_free b [] y
      by_cases h : s ∈ B <;> simp [freeNames, h, hf, this] <;> grind
  | .member o _, B, y | .named _ o, B, y | .typed o _, B, y | .some o, B, y | .unary _ o, B, y => by
    simp only [freeNames]; exact mem_free o B y
  | .binary _ a b, B, y => by
    simp only [freeNames, List.mem_append, mem_free a B y, mem_free b B y]; grind
  | .ternary a b c, B, y => by
    simp only [freeNames, List.mem_append, mem_free a B y, mem_free b B y, mem_free c B y]; grind
  | .matchOpt s x a b, B, y => by
    have h1 := mem_free s B y
    have h2 := mem_free b B y
    have h3 := mem_free a (x :: B) y
    have h4 := mem_free a [x] y
    simp only [freeNames, List.mem_append]
    simp only [List.mem_cons, List.not_mem_nil, or_false] at h3 h4
    grind
  | .letE x v b, B, y => by
    have h1 := mem_free v B y
    have h3 := mem_free b (x :: B) y
    have h4 := mem_free b [x] y
    simp only [freeNames, List.mem_append]
    simp only [List.mem_cons, List.not_mem_nil, or_false] at h3 h4
    grind
  | .arrow ps b, B, y => by
    have h3 := mem_free b (ps ++ B) y
    have h4 := mem_free b ps y
    simp only [freeNames, List.append_nil]
    simp only [List.mem_append] at h3
    grind
  | .template parts, B, y => by simp only [freeNames]; exact mem_freeList parts B y
  | .num _, B, y | .str _, B, y | .bool _, B, y | .none, B, y | .emptyList, B, y => by
    simp [freeNames]
theorem mem_freeList : ∀ (es : List Expr) (B : List String) (y : String),
    y ∈ freeNamesList B es ↔ y ∈ freeNamesList [] es ∧ y ∉ B
  | [], B, y => by simp [freeNamesList]
  | e :: es, B, y => by
    simp only [freeNamesList, List.mem_append, mem_free e B y, mem_freeList es B y]; grind
theorem mem_freeFields : ∀ (es : List (String × Expr)) (B : List String) (y : String),
    y ∈ freeNamesFields B es ↔ y ∈ freeNamesFields [] es ∧ y ∉ B
  | [], B, y => by simp [freeNamesFields]
  | (_, e) :: es, B, y => by
    simp only [freeNamesFields, List.mem_append, mem_free e B y, mem_freeFields es B y]; grind
end

theorem mem_free_bind {e : Expr} {x y : String} (h : y ∈ freeNames [] e) (hne : y ≠ x) :
    y ∈ freeNames [x] e := by
  rw [mem_free]; simp [h, hne]

/-! ## Callbacks of at most two parameters -/

mutual
/-- Every callback has at most two parameters: the ones `map` and
`filter` bind (a third would read whatever its name means outside). -/
def Small : Expr → Bool
  | .arrow ps b => decide (ps.length ≤ 2) && Small b
  | .call _ args => SmallList args
  | .record _ base fields =>
    (match base with | .some b => Small b | .none => true) && SmallFields fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => Small o
  | .binary _ a b => Small a && Small b
  | .ternary a b c => Small a && Small b && Small c
  | .matchOpt s _ a b => Small s && Small a && Small b
  | .letE _ v b => Small v && Small b
  | .template parts => SmallList parts
  | .var _ | .num _ | .str _ | .bool _ | .none | .emptyList => true
def SmallList : List Expr → Bool
  | [] => true
  | e :: es => Small e && SmallList es
def SmallFields : List (String × Expr) → Bool
  | [] => true
  | (_, e) :: es => Small e && SmallFields es
end

theorem lookup_bindParams_mem {ps : List String} {y : String} {x : Value} {i : Nat} {L L' : Locals}
    (hl : ps.length ≤ 2) (hy : y ∈ ps) :
    lookup y (bindParams ps x i L) = lookup y (bindParams ps x i L') := by
  match ps, hl, hy with
  | [p], _, hy => simp at hy; subst hy; simp [bindParams, lookup]
  | [p, q], _, hy =>
    simp only [bindParams, lookup]
    by_cases hq : y = q
    · simp [hq]
    · simp at hy; rcases hy with rfl | rfl
      · simp [hq]
      · exact absurd rfl hq

theorem lookup_bindParams_not {ps : List String} {y : String} {x : Value} {i : Nat} {L : Locals}
    (hy : y ∉ ps) : lookup y (bindParams ps x i L) = lookup y L := by
  match ps, hy with
  | [], _ => rfl
  | [p], hy => simp at hy; simp [bindParams, lookup, hy]
  | p :: q :: _, hy =>
    simp at hy; simp [bindParams, lookup, hy.1, hy.2.1]

/-! ## Weakening: only the free names' locals matter -/

/-- Two local scopes that agree on the names `ns`. -/
def On (ns : List String) (L L' : Locals) : Prop := ∀ y ∈ ns, lookup y L = lookup y L'

theorem On.cons {L L' : Locals} {x : String} {w : Value} {e : Expr}
    (h : On (freeNames [x] e) L L') : On (freeNames [] e) ((x, w) :: L) ((x, w) :: L') := by
  intro y hy
  by_cases hx : y = x
  · subst hx; simp [lookup]
  · rw [lookup_cons_ne hx, lookup_cons_ne hx]; exact h y (mem_free_bind hy hx)

theorem On.params {ps : List String} {L L' : Locals} {x : Value} {i : Nat} {e : Expr}
    (hl : ps.length ≤ 2) (h : On (freeNames ps e) L L') :
    On (freeNames [] e) (bindParams ps x i L) (bindParams ps x i L') := by
  intro y hy
  by_cases hp : y ∈ ps
  · exact lookup_bindParams_mem hl hp
  · rw [lookup_bindParams_not hp, lookup_bindParams_not hp]
    exact h y ((mem_free e ps y).2 ⟨hy, hp⟩)

theorem On.sub {ns ms : List String} {L L' : Locals} (h : On ns L L') (hm : ∀ y ∈ ms, y ∈ ns) :
    On ms L L' := fun y hy => h y (hm y hy)

theorem smallField : ∀ {w : List (String × Expr)} {f : String} {e : Expr},
    SmallFields w = true → lookupField f w = .some e → Small e = true
  | [], _, _, _, h => by simp [lookupField] at h
  | (g, x) :: w, f, e, hs, h => by
    simp only [SmallFields, Bool.and_eq_true] at hs
    simp only [lookupField] at h
    split at h
    · cases h; exact hs.1
    · exact smallField hs.2 h

theorem freeField : ∀ {w : List (String × Expr)} {f : String} {e : Expr},
    lookupField f w = .some e → ∀ y ∈ freeNames [] e, y ∈ freeNamesFields [] w
  | [], _, _, h => by simp [lookupField] at h
  | (g, x) :: w, f, e, h => by
    intro y hy
    simp only [lookupField] at h
    simp only [freeNamesFields, List.mem_append]
    split at h
    · cases h; exact .inl hy
    · exact .inr (freeField h y hy)

mutual
theorem wkE {env : Env} {inFn L L' e v} (h : EvalR env inFn L e v) (hs : Small e = true)
    (ha : On (freeNames [] e) L L') : EvalR env inFn L' e v :=
  match h with
  | .num => .num
  | .str => .str
  | .bool => .bool
  | .none => .none
  | .emptyList => .emptyList
  | .some h => .some (wkE h (by simpa [Small] using hs) (by simpa [freeNames] using ha))
  | .template h => .template (wkD h (by simpa [Small] using hs) (by simpa [freeNames] using ha))
  | .local hl => by rename_i x; exact .local ((ha x (by simp [freeNames])) ▸ hl)
  | .global hl hi hg => by rename_i x; exact .global ((ha x (by simp [freeNames])) ▸ hl) hi hg
  | .member h hi hv => .member (wkE h (by simpa [Small] using hs) (by simpa [freeNames] using ha)) hi hv
  | .fn hfd hargs hb => by
    simp only [Small] at hs
    exact .fn hfd (wkL hargs hs (ha.sub fun y hy => by simp [freeNames]; exact .inr hy)) hb
  | .map hfd hl hm => by
    simp only [Small, SmallList, Bool.and_eq_true, decide_eq_true_eq] at hs
    exact .map hfd (wkE hl hs.1 (ha.sub fun y hy => by simp [freeNames, freeNamesList]; exact .inr (.inl hy)))
      (wkM hm hs.2.1.1 hs.2.1.2
        (ha.sub fun y hy => by simp [freeNames, freeNamesList]; exact .inr (.inr hy)))
  | .filter hfd hl hm => by
    simp only [Small, SmallList, Bool.and_eq_true, decide_eq_true_eq] at hs
    exact .filter hfd (wkE hl hs.1 (ha.sub fun y hy => by simp [freeNames, freeNamesList]; exact .inr (.inl hy)))
      (wkF hm hs.2.1.1 hs.2.1.2
        (ha.sub fun y hy => by simp [freeNames, freeNamesList]; exact .inr (.inr hy)))
  | .pendingSettled hfd hr hv => .pendingSettled hfd hr hv
  | .pendingOther hfd hr => .pendingOther hfd hr
  | .failedSettled hfd hr hv => .failedSettled hfd hr hv
  | .failedOther hfd hr => .failedOther hfd hr
  | .stdlib hfd hargs hv => by
    simp only [Small] at hs
    exact .stdlib hfd (wkL hargs hs (ha.sub fun y hy => by simp [freeNames]; exact .inr hy)) hv
  | .record hd h => by
    simp only [Small, Bool.true_and] at hs
    exact .record hd (wkR h hs (ha.sub fun y hy => by simp [freeNames]; exact .inr hy))
  | .recordBase hb hd h => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .recordBase (wkE hb hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inl hy))) hd
      (wkR h hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inr hy)))
  | .neg h => .neg (wkE h (by simpa [Small] using hs) (by simpa [freeNames] using ha))
  | .not h => .not (wkE h (by simpa [Small] using hs) (by simpa [freeNames] using ha))
  | .andFalse h => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .andFalse (wkE h hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
  | .andTrue h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .andTrue (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr hy))
  | .orTrue h => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .orTrue (wkE h hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
  | .orFalse h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .orFalse (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr hy))
  | .binop h1 h2 h3 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .binop (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr hy)) h3
  | .ternaryTrue h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .ternaryTrue (wkE h1 hs.1.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.1.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inl hy)))
  | .ternaryFalse h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .ternaryFalse (wkE h1 hs.1.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inr hy)))
  | .matchSome h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .matchSome (wkE h1 hs.1.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.1.2 (On.cons (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inr hy))))
  | .matchNone h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .matchNone (wkE h1 hs.1.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (ha.sub fun y hy => by simp [freeNames]; exact .inr (.inl hy)))
  | .letE h1 h2 => by
    simp only [Small, Bool.and_eq_true] at hs
    exact .letE (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNames]; exact .inl hy))
      (wkE h2 hs.2 (On.cons (ha.sub fun y hy => by simp [freeNames]; exact .inr hy)))
  | .typed h => .typed (wkE h (by simpa [Small] using hs) (by simpa [freeNames] using ha))
theorem wkL {env : Env} {inFn L L' es vs} (h : ListR env inFn L es vs) (hs : SmallList es = true)
    (ha : On (freeNamesList [] es) L L') : ListR env inFn L' es vs :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => by
    simp only [SmallList, Bool.and_eq_true] at hs
    exact .cons (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNamesList]; exact .inl hy))
      (wkL h2 hs.2 (ha.sub fun y hy => by simp [freeNamesList]; exact .inr hy))
theorem wkD {env : Env} {inFn L L' es ss} (h : DisplaysR env inFn L es ss) (hs : SmallList es = true)
    (ha : On (freeNamesList [] es) L L') : DisplaysR env inFn L' es ss :=
  match h with
  | .nil => .nil
  | .cons h1 hd h2 => by
    simp only [SmallList, Bool.and_eq_true] at hs
    exact .cons (wkE h1 hs.1 (ha.sub fun y hy => by simp [freeNamesList]; exact .inl hy)) hd
      (wkD h2 hs.2 (ha.sub fun y hy => by simp [freeNamesList]; exact .inr hy))
theorem wkM {env : Env} {inFn L L' ps body xs i ys} (h : MapR env inFn L ps body xs i ys)
    (hl : ps.length ≤ 2) (hs : Small body = true) (ha : On (freeNames ps body) L L') :
    MapR env inFn L' ps body xs i ys :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => .cons (wkE h1 hs (On.params hl ha)) (wkM h2 hl hs ha)
theorem wkF {env : Env} {inFn L L' ps body xs i ys} (h : FilterR env inFn L ps body xs i ys)
    (hl : ps.length ≤ 2) (hs : Small body = true) (ha : On (freeNames ps body) L L') :
    FilterR env inFn L' ps body xs i ys :=
  match h with
  | .nil => .nil
  | .keep h1 h2 => .keep (wkE h1 hs (On.params hl ha)) (wkF h2 hl hs ha)
  | .drop h1 h2 => .drop (wkE h1 hs (On.params hl ha)) (wkF h2 hl hs ha)
theorem wkR {env : Env} {inFn L L' w b fs i vs} (h : FieldsR env inFn L w b fs i vs)
    (hs : SmallFields w = true) (ha : On (freeNamesFields [] w) L L') : FieldsR env inFn L' w b fs i vs :=
  match h with
  | .nil => .nil
  | .written he h1 h2 => .written he (wkE h1 (smallField hs he) (ha.sub (freeField he))) (wkR h2 hs ha)
  | .base he hb h2 => .base he hb (wkR h2 hs ha)
end

/-- Weakening both ways: locals that agree on the free names read alike. -/
theorem wk_iff {env : Env} {inFn L L' e v} (hs : Small e = true) (ha : On (freeNames [] e) L L') :
    EvalR env inFn L e v ↔ EvalR env inFn L' e v :=
  ⟨fun h => wkE h hs ha, fun h => wkE h hs fun y hy => (ha y hy).symm⟩

/-! ## The substitution's facts -/

theorem mem_foldl_free {y : String} :
    ∀ (m : SMap) (acc : List String), y ∈ m.foldl (fun acc (p : String × Expr) => acc ++ freeNames [] p.2) acc ↔
      y ∈ acc ∨ ∃ p ∈ m, y ∈ freeNames [] p.2
  | [], acc => by simp
  | p :: m, acc => by
    rw [List.foldl_cons, mem_foldl_free m]
    simp only [List.mem_append, List.mem_cons, exists_eq_or_imp]
    grind

theorem mem_free_of_map {s : Subst} {k : String} {r : Expr} (hm : (k, r) ∈ s.map) {y : String}
    (hy : y ∈ freeNames [] r) : y ∈ s.free := by
  have := (mem_foldl_free (y := y) s.map []).2 (.inr ⟨(k, r), hm, hy⟩)
  simpa [Subst.free] using this

theorem SMap.get?_mem {m : SMap} {k : String} {r : Expr} (h : m.get? k = .some r) : (k, r) ∈ m := by
  simp only [SMap.get?, Option.map_eq_some_iff] at h
  obtain ⟨⟨k', r'⟩, hf, rfl⟩ := h
  have hk := List.find?_some hf
  simp at hk; subst hk
  exact List.mem_of_find?_eq_some hf

/-- A replacement by a name is an enclosing binder's spelling or a name a
replacement mentions. -/
theorem get_name {s : Subst} {y m : String} (h : s.get y = .some (.name m)) :
    m ∈ s.binders.filterMap (·.2) ∨ m ∈ s.free := by
  unfold Subst.get at h
  split at h
  · rename_i fst b hb
    left
    have hmem := List.mem_of_find?_eq_some hb
    cases b with
    | none => simp at h
    | some m' =>
      simp at h; subst h
      exact List.mem_filterMap.2 ⟨_, hmem, rfl⟩
  · right
    simp only [Option.map_eq_some_iff] at h
    obtain ⟨r, hr, hrep⟩ := h
    cases r with
    | var n => simp [replOf] at hrep; subst hrep; exact mem_free_of_map (SMap.get?_mem hr) (by simp [freeNames])
    | _ => simp [replOf] at hrep

/-- A replacement by an expression is one of the map's. -/
theorem get_expr {s : Subst} {y : String} {r : Expr} (h : s.get y = .some (.expr r)) :
    ∃ k, (k, r) ∈ s.map := by
  unfold Subst.get at h
  split at h
  · rename_i _ b _
    cases b <;> simp at h
  · simp only [Option.map_eq_some_iff] at h
    obtain ⟨r', hr, hrep⟩ := h
    cases r' <;> simp [replOf] at hrep <;> (subst hrep; exact ⟨y, SMap.get?_mem hr⟩)

theorem fold_max_le {L : List String} : ∀ {m : Nat} {t : String},
    (t ∈ L → t.length ≤ L.foldl (fun m t => max m t.length) m) ∧ m ≤ L.foldl (fun m t => max m t.length) m := by
  induction L with
  | nil => simp
  | cons a L ih =>
    intro m t
    simp only [List.foldl_cons, List.mem_cons]
    refine ⟨?_, Nat.le_trans (Nat.le_max_left _ _) (ih (t := t)).2⟩
    rintro (rfl | h)
    · exact Nat.le_trans (Nat.le_max_right _ _) (ih (t := t)).2
    · exact (ih (t := t)).1 h

theorem beyond_not_mem (var : String) (taken : List String) : Subst.beyond var taken ∉ taken := by
  intro h
  have hle := (fold_max_le (m := 0)).1 h
  have : (Subst.beyond var taken).length > taken.foldl (fun m t => max m t.length) 0 := by
    simp [Subst.beyond, String.length_append, String.length_ofList]
    omega
  omega

theorem fresh_not_mem (var : String) (taken : List String) : ∀ fuel k, Subst.fresh var taken fuel k ∉ taken
  | 0, _ => beyond_not_mem var taken
  | fuel + 1, k => by
    simp only [Subst.fresh]
    split
    · exact fresh_not_mem var taken fuel (k + 1)
    · rename_i hc
      intro hm
      exact hc (List.contains_iff_mem.mpr hm)

/-- What entering a binder spells it: either itself (no replacement
mentions it and no enclosing binder was renamed to it), or a name the
scope, the replacements and the enclosing binders do not write. -/
theorem enter_spelled (s : Subst) (x : String) (names : List String) :
    let x' := (s.enter x names).1
    x' ∉ s.free ∧ x' ∉ s.binders.filterMap (·.2) ∧ (x' = x ∨ x' ∉ names) := by
  simp only [Subst.enter]
  split
  · rename_i hc
    have := fresh_not_mem x (names ++ s.free ++ s.binders.filterMap (·.2))
      ((names ++ s.free ++ s.binders.filterMap (·.2)).length + 1) 1
    simp only [List.mem_append, not_or] at this
    exact ⟨this.1.2, this.2, .inr this.1.1⟩
  · rename_i hc
    simp only [Bool.or_eq_true, not_or, Bool.not_eq_true] at hc
    refine ⟨by simpa using hc.1, ?_, .inl rfl⟩
    intro hm
    have : s.introduced x = true := by
      simp only [Subst.introduced, List.any_eq_true]
      obtain ⟨b, hb, hb2⟩ := List.mem_filterMap.1 hm
      exact ⟨b, hb, by simp [hb2]⟩
    rw [hc.2] at this; exact absurd this Bool.false_ne_true

theorem enter_binders (s : Subst) (x : String) (names : List String) :
    (s.enter x names).2.binders =
      (x, if (s.enter x names).1 != x then .some (s.enter x names).1 else .none) :: s.binders := rfl

theorem enter_get_renamed (s : Subst) (x : String) (names : List String)
    (h : (s.enter x names).1 ≠ x) :
    (s.enter x names).2.get x = .some (.name (s.enter x names).1) := by
  have hb := enter_binders s x names
  simp only [Subst.get, hb, List.find?_cons, beq_self_eq_true]
  simp [h]

theorem enter_get_kept (s : Subst) (x : String) (names : List String)
    (h : (s.enter x names).1 = x) : (s.enter x names).2.get x = .none := by
  have hb := enter_binders s x names
  simp only [Subst.get, hb, List.find?_cons, beq_self_eq_true]
  simp [h]

theorem enter_get_other (s : Subst) (x : String) (names : List String) {y : String} (hy : y ≠ x) :
    (s.enter x names).2.get y = s.get y := by
  simp only [Subst.enter, Subst.get, List.find?_cons]
  have : (x == y) = false := by simp; exact fun h => hy h.symm
  simp [this]

theorem enter_map (s : Subst) (x : String) (names : List String) : (s.enter x names).2.map = s.map := by
  simp [Subst.enter]

/-! ## Agreement -/

/-- What the substitution makes of name `x`, read in the target. -/
def Reads (e₂ : Env) (Lf : Locals) (s : Subst) (x : String) (v : Value) : Prop :=
  match s.get x with
  | .some (.name m) => Res e₂ Lf m v
  | .some (.expr r) => EvalR e₂ false Lf r v
  | .none => Res e₂ Lf x v

/-- `x` reads in the source exactly what its substitution reads in the target. -/
def Agree (e₁ : Env) (ls : Locals) (e₂ : Env) (Lf : Locals) (s : Subst) (x : String) : Prop :=
  ∀ v, Res e₁ ls x v ↔ Reads e₂ Lf s x v

/-- Every replacement is an expression `Small` holds of. -/
def MapSmall (s : Subst) : Prop := ∀ p ∈ s.map, Small p.2 = true

theorem substVar (e₂ : Env) (Lf : Locals) (s : Subst) (x : String) (v : Value) :
    EvalR e₂ false Lf (substExpr s (.var x)) v ↔ Reads e₂ Lf s x v := by
  simp only [substExpr, Reads]
  split <;> rename_i h <;> simp only [h, var_iff]

/-- Entering a binder keeps agreement: on the binder itself (bound to the
same value on both sides), and on every other name of the scope. -/
theorem agree_enter {e₁ e₂ : Env} {ls Lf : Locals} {s : Subst} {x : String} {w : Value}
    {names R : List String} (hm : MapSmall s)
    (hA : ∀ y ∈ R, y ≠ x → Agree e₁ ls e₂ Lf s y) (hR : ∀ y ∈ R, y = x ∨ y ∈ names) :
    ∀ y ∈ R, Agree e₁ ((x, w) :: ls) e₂ (((s.enter x names).1, w) :: Lf) (s.enter x names).2 y := by
  obtain ⟨hf, hb, hn⟩ := enter_spelled s x names
  intro y hy v
  by_cases hyx : y = x
  · subst hyx
    rw [res_cons_eq]
    by_cases hr : (s.enter y names).1 = y
    · simp only [Reads, enter_get_kept s y names hr]
      rw [hr, res_cons_eq]
    · simp only [Reads, enter_get_renamed s y names hr]
      rw [res_cons_eq]
  · rw [res_cons_ne hyx, hA y hy hyx v]
    simp only [Reads, enter_get_other s x names hyx]
    split
    · rename_i m hg
      have hm' : m ≠ (s.enter x names).1 := by
        rintro rfl
        rcases get_name hg with h | h
        · exact hb h
        · exact hf h
      rw [res_cons_ne hm']
    · rename_i r hg
      obtain ⟨k, hk⟩ := get_expr hg
      refine wk_iff (hm _ hk) ?_
      intro z hz
      have hz' : z ≠ (s.enter x names).1 := fun h => hf (h ▸ mem_free_of_map hk hz)
      rw [lookup_cons_ne hz']
    · rename_i hg
      have hy' : y ≠ (s.enter x names).1 := by
        intro h
        rcases hn with h' | h'
        · exact hyx (h.trans h')
        · rcases hR y hy with h'' | h''
          · exact hyx h''
          · exact h' (h ▸ h'')
      rw [res_cons_ne hy']

/-! ## The fragment -/

mutual
/-- The names an expression calls. -/
def callHeads : Expr → List String
  | .call n args => n :: callHeadsList args
  | .record _ base fields =>
    (match base with | .some b => callHeads b | .none => []) ++ callHeadsFields fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => callHeads o
  | .binary _ a b => callHeads a ++ callHeads b
  | .ternary a b c => callHeads a ++ callHeads b ++ callHeads c
  | .matchOpt s _ a b => callHeads s ++ callHeads a ++ callHeads b
  | .letE _ v b => callHeads v ++ callHeads b
  | .arrow _ b => callHeads b
  | .template parts => callHeadsList parts
  | .var _ | .num _ | .str _ | .bool _ | .none | .emptyList => []
def callHeadsList : List Expr → List String
  | [] => []
  | e :: es => callHeads e ++ callHeadsList es
def callHeadsFields : List (String × Expr) → List String
  | [] => []
  | (_, e) :: es => callHeads e ++ callHeadsFields es
end

mutual
/-- The fragment substitution is proved for: no `pending`/`failed`, every
callback of at most two parameters, and no binder named like a call its
scope makes. -/
def Plain : Expr → Bool
  | .call n args => n != "pending" && n != "failed" && PlainList args
  | .arrow ps b =>
    decide (ps.length ≤ 2) && ps.all (fun p => !(callHeads b).contains p) && Plain b
  | .matchOpt s x a b => Plain s && !(callHeads a).contains x && Plain a && Plain b
  | .letE x v b => Plain v && !(callHeads b).contains x && Plain b
  | .record _ base fields =>
    (match base with | .some b => Plain b | .none => true) && PlainFields fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => Plain o
  | .binary _ a b => Plain a && Plain b
  | .ternary a b c => Plain a && Plain b && Plain c
  | .template parts => PlainList parts
  | .var _ | .num _ | .str _ | .bool _ | .none | .emptyList => true
def PlainList : List Expr → Bool
  | [] => true
  | e :: es => Plain e && PlainList es
def PlainFields : List (String × Expr) → Bool
  | [] => true
  | (_, e) :: es => Plain e && PlainFields es
end

/-! ## Equations of the substitution -/

theorem subst_call {s : Subst} {n : String} {args : List Expr} (h : s.get n = .none) :
    substExpr s (.call n args) = .call n (substList s args) := by
  simp only [substExpr]
  split
  · rfl
  · simp [h]

theorem subst_match (s : Subst) (subj : Expr) (x : String) (a b : Expr) :
    substExpr s (.matchOpt subj x a b) =
      .matchOpt (substExpr s subj) (s.enter x (allNames a)).1
        (substExpr (s.enter x (allNames a)).2 a) (substExpr s b) := by
  simp only [substExpr]

theorem subst_let (s : Subst) (x : String) (v b : Expr) :
    substExpr s (.letE x v b) =
      .letE (s.enter x (allNames b)).1 (substExpr s v) (substExpr (s.enter x (allNames b)).2 b) := by
  simp only [substExpr]

theorem subst_arrow (s : Subst) (ps : List String) (b : Expr) :
    substExpr s (.arrow ps b) =
      .arrow (enterParams b ps s ps).1 (substExpr (enterParams b ps s ps).2 b) := by
  simp only [substExpr]

theorem lookupField_subst (s : Subst) (f : String) :
    ∀ (w : List (String × Expr)), lookupField f (substFields s w) = (lookupField f w).map (substExpr s)
  | [] => rfl
  | (g, e) :: w => by
    simp only [substFields, lookupField]
    split <;> simp [lookupField_subst s f w]

theorem enterParams_map (body : Expr) (all : List String) :
    ∀ (s : Subst) (ps : List String), (enterParams body all s ps).2.map = s.map
  | s, [] => rfl
  | s, p :: ps => by
    simp only [enterParams]
    rw [enterParams_map body all _ ps, enter_map]

theorem enterParams_get (body : Expr) (all : List String) :
    ∀ (s : Subst) (ps : List String) {n : String}, n ∉ ps → (enterParams body all s ps).2.get n = s.get n
  | s, [], _, _ => rfl
  | s, p :: ps, n, hn => by
    simp only [List.mem_cons, not_or] at hn
    simp only [enterParams]
    rw [enterParams_get body all _ ps hn.2, enter_get_other s p _ hn.1]

/-! ## Free variables -/

mutual
/-- The names an expression reads that it does not bind (no call head, no
record shape): the names a substitution must agree on. -/
def fv : Expr → List String
  | .var n => [n]
  | .call _ args => fvList args
  | .record _ base fields => fvOpt base ++ fvFields fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => fv o
  | .binary _ a b => fv a ++ fv b
  | .ternary a b c => fv a ++ fv b ++ fv c
  | .matchOpt s x a b => fv s ++ fv b ++ (fv a).filter (· != x)
  | .letE x v b => fv v ++ (fv b).filter (· != x)
  | .arrow ps b => (fv b).filter (fun y => !ps.contains y)
  | .template parts => fvList parts
  | .num _ | .str _ | .bool _ | .none | .emptyList => []
def fvOpt : Option Expr → List String
  | .some b => fv b
  | .none => []
def fvList : List Expr → List String
  | [] => []
  | e :: es => fv e ++ fvList es
def fvFields : List (String × Expr) → List String
  | [] => []
  | (_, e) :: es => fv e ++ fvFields es
end

mutual
theorem fv_sub_all : ∀ (e : Expr) {y : String}, y ∈ fv e → y ∈ allNames e
  | .var n, y, h => by simpa [fv, allNames] using h
  | .call n args, y, h => by
    simp only [fv] at h; simp only [allNames, List.mem_cons]; exact .inr (fvList_sub_all args h)
  | .record s base fields, y, h => by
    simp only [fv, List.mem_append] at h
    cases base with
    | none =>
      simp only [fvOpt, List.not_mem_nil, false_or] at h
      simp only [allNames, List.mem_cons, List.mem_append]; exact .inr (.inr (fvFields_sub_all fields h))
    | some b =>
      simp only [fvOpt] at h
      simp only [allNames, List.mem_cons, List.mem_append]
      rcases h with h | h
      · exact .inr (.inl (fv_sub_all b h))
      · exact .inr (.inr (fvFields_sub_all fields h))
  | .member o _, y, h | .named _ o, y, h | .typed o _, y, h | .some o, y, h | .unary _ o, y, h => by
    simp only [fv] at h; simp only [allNames]; exact fv_sub_all o h
  | .binary _ a b, y, h => by
    simp only [fv, List.mem_append] at h; simp only [allNames, List.mem_append]
    rcases h with h | h
    · exact .inl (fv_sub_all a h)
    · exact .inr (fv_sub_all b h)
  | .ternary a b c, y, h => by
    simp only [fv, List.mem_append] at h; simp only [allNames, List.mem_append]
    rcases h with (h | h) | h
    · exact .inl (.inl (fv_sub_all a h))
    · exact .inl (.inr (fv_sub_all b h))
    · exact .inr (fv_sub_all c h)
  | .matchOpt s x a b, y, h => by
    simp only [fv, List.mem_append, List.mem_filter] at h; simp only [allNames, List.mem_cons, List.mem_append]
    rcases h with (h | h) | ⟨h, _⟩
    · exact .inr (.inl (.inl (fv_sub_all s h)))
    · exact .inr (.inr (fv_sub_all b h))
    · exact .inr (.inl (.inr (fv_sub_all a h)))
  | .letE x v b, y, h => by
    simp only [fv, List.mem_append, List.mem_filter] at h; simp only [allNames, List.mem_cons, List.mem_append]
    rcases h with h | ⟨h, _⟩
    · exact .inr (.inl (fv_sub_all v h))
    · exact .inr (.inr (fv_sub_all b h))
  | .arrow ps b, y, h => by
    simp only [fv, List.mem_filter] at h; simp only [allNames, List.mem_append]
    exact .inr (fv_sub_all b h.1)
  | .template parts, y, h => by
    simp only [fv] at h; simp only [allNames]; exact fvList_sub_all parts h
  | .num _, y, h | .str _, y, h | .bool _, y, h | .none, y, h | .emptyList, y, h => by simp [fv] at h
theorem fvList_sub_all : ∀ (es : List Expr) {y : String}, y ∈ fvList es → y ∈ allNamesList es
  | [], y, h => by simp [fvList] at h
  | e :: es, y, h => by
    simp only [fvList, List.mem_append] at h; simp only [allNamesList, List.mem_append]
    rcases h with h | h
    · exact .inl (fv_sub_all e h)
    · exact .inr (fvList_sub_all es h)
theorem fvFields_sub_all : ∀ (es : List (String × Expr)) {y : String}, y ∈ fvFields es → y ∈ allNamesFields es
  | [], y, h => by simp [fvFields] at h
  | (_, e) :: es, y, h => by
    simp only [fvFields, List.mem_append] at h; simp only [allNamesFields, List.mem_append]
    rcases h with h | h
    · exact .inl (fv_sub_all e h)
    · exact .inr (fvFields_sub_all es h)
end

theorem fields_fv {k : String} {e : Expr} : ∀ {w : List (String × Expr)}, (k, e) ∈ w →
    ∀ y ∈ fv e, y ∈ fvFields w
  | [], h => by simp at h
  | (g, x) :: w, h => by
    simp only [List.mem_cons, Prod.mk.injEq] at h
    intro y hy
    simp only [fvFields, List.mem_append]
    rcases h with ⟨rfl, rfl⟩ | h
    · exact .inl hy
    · exact .inr (fields_fv h y hy)

/-- A callback's parameters, entered: agreement on the body, the
parameters bound to an item and its index on both sides. -/
theorem agree_params {e₁ e₂ : Env} {ls Lf : Locals} {s : Subst} {ps : List String} {body : Expr}
    (hm : MapSmall s) (hl : ps.length ≤ 2)
    (hA : ∀ y ∈ fv (.arrow ps body), Agree e₁ ls e₂ Lf s y) (x : Value) (i : Nat) :
    ∀ y ∈ fv body,
      Agree e₁ (bindParams ps x i ls) e₂ (bindParams (enterParams body ps s ps).1 x i Lf)
        (enterParams body ps s ps).2 y := by
  match ps, hl with
  | [], _ =>
    intro y hy
    exact hA y (by simp [fv, hy])
  | [p], _ =>
    simp only [enterParams, bindParams]
    exact agree_enter hm (fun y hy hne => hA y (by simp [fv, hy, hne]))
      (fun y hy => .inr (by simp [fv_sub_all body hy]))
  | [p, q], _ =>
    simp only [enterParams, bindParams]
    have h1 := agree_enter (x := p) (w := x) (names := allNames body ++ [p, q])
      (R := (fv body).filter (· != q)) hm
      (fun y hy hne => hA y (by simp only [List.mem_filter, bne_iff_ne, ne_eq] at hy; simp [fv, hy, hne]))
      (fun y hy => .inr (by simp only [List.mem_filter] at hy; simp [fv_sub_all body hy.1]))
    have hm1 : MapSmall (s.enter p (allNames body ++ [p, q])).2 := by
      intro r hr; rw [enter_map] at hr; exact hm r hr
    exact agree_enter hm1 (fun y hy hne => h1 y (by simp [hy, hne]))
      (fun y hy => .inr (by simp [fv_sub_all body hy]))

theorem lookupField_mem : ∀ {w : List (String × Expr)} {f : String} {e : Expr},
    lookupField f w = .some e → ∃ k, (k, e) ∈ w
  | [], _, _, h => by simp [lookupField] at h
  | (g, x) :: w, f, e, h => by
    simp only [lookupField] at h
    split at h
    · cases h; exact ⟨g, by simp⟩
    · obtain ⟨k, hk⟩ := lookupField_mem h; exact ⟨k, by simp [hk]⟩

theorem fields_mem {k : String} {e : Expr} : ∀ {w : List (String × Expr)}, (k, e) ∈ w →
    (∀ y ∈ allNames e, y ∈ allNamesFields w) ∧ (PlainFields w = true → Plain e = true) ∧
      (∀ n ∈ callHeads e, n ∈ callHeadsFields w)
  | [], h => by simp at h
  | (g, x) :: w, h => by
    simp only [List.mem_cons, Prod.mk.injEq] at h
    rcases h with ⟨rfl, rfl⟩ | h
    · exact ⟨fun y hy => by simp [allNamesFields, hy], fun hp => by simp [PlainFields] at hp; exact hp.1,
        fun n hn => by simp [callHeadsFields, hn]⟩
    · obtain ⟨h1, h2, h3⟩ := fields_mem h
      exact ⟨fun y hy => by simp [allNamesFields, h1 y hy], fun hp => by
          simp [PlainFields] at hp; exact h2 hp.2, fun n hn => by simp [callHeadsFields, h3 n hn]⟩

theorem not_contains {l : List String} {x : String} (h : (!l.contains x) = true) : x ∉ l := by
  intro hm; simp at h; exact h hm

theorem plain_cb {f : String} {l : Expr} {ps : List String} {body : Expr}
    (h : Plain (.call f [l, .arrow ps body]) = true) :
    Plain l = true ∧ ps.length ≤ 2 ∧ (∀ p ∈ ps, p ∉ callHeads body) ∧ Plain body = true := by
  simp only [Plain, PlainList, Bool.and_eq_true, Bool.and_true, decide_eq_true_eq,
    List.all_eq_true] at h
  exact ⟨h.2.1, h.2.2.1.1, fun p hp => not_contains (h.2.2.1.2 p hp), h.2.2.2⟩

theorem plain_match {subj : Expr} {x : String} {a b : Expr} (h : Plain (.matchOpt subj x a b) = true) :
    Plain subj = true ∧ x ∉ callHeads a ∧ Plain a = true ∧ Plain b = true := by
  simp only [Plain, Bool.and_eq_true] at h
  exact ⟨h.1.1.1, not_contains h.1.1.2, h.1.2, h.2⟩

theorem plain_let {x : String} {v b : Expr} (h : Plain (.letE x v b) = true) :
    Plain v = true ∧ x ∉ callHeads b ∧ Plain b = true := by
  simp only [Plain, Bool.and_eq_true] at h
  exact ⟨h.1.1, not_contains h.1.2, h.2⟩

theorem sf_var {e₁ e₂ : Env} {ls Lf : Locals} {s : Subst} {x : String} {v : Value}
    (h : EvalR e₁ false ls (.var x) v) (hA : Agree e₁ ls e₂ Lf s x) :
    EvalR e₂ false Lf (substExpr s (.var x)) v :=
  (substVar e₂ Lf s x v).2 ((hA v).1 (var_iff.1 h))

/-! ## Substitution, forwards: a source value is a target value -/

mutual
theorem sfE {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls e v} (h : EvalR e₁ inFn ls e v)
    (hf : inFn = false) {s : Subst} {Lf : Locals} (hm : MapSmall s)
    (hA : ∀ y ∈ fv e, Agree e₁ ls e₂ Lf s y) (hp : Plain e = true)
    (hh : ∀ n ∈ callHeads e, s.get n = .none) : EvalR e₂ false Lf (substExpr s e) v :=
  match h with
  | .num => by simp only [substExpr]; exact .num
  | .str => by simp only [substExpr]; exact .str
  | .bool => by simp only [substExpr]; exact .bool
  | .none => by simp only [substExpr]; exact .none
  | .emptyList => by simp only [substExpr]; exact .emptyList
  | .some h => by
    simp only [substExpr]
    exact .some (sfE hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh))
  | .template h => by
    simp only [substExpr]
    exact .template (sfD hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh))
  | .local hl => sf_var (.local hl) (hA _ (by simp [fv]))
  | .global hl _ hg => sf_var (.global hl rfl hg) (hA _ (by simp [fv]))
  | .member h hi hv => by
    simp only [substExpr]
    exact .member (sfE hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh)) (hs.fieldIndex _ _ ▸ hi) hv
  | .fn hfd ha hb => by
    rename_i name _ _ _
    rw [subst_call (hh name (by simp [callHeads]))]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .fn (hs.fns ▸ hfd)
      (sfL hs ha hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun n hn => hh n (by simp [callHeads, hn])))
      (xferE hs hb rfl)
  | .map hfd hl hmap => by
    rename_i l xs ps body ys
    rw [subst_call (hh "map" (by simp [callHeads]))]
    simp only [substList, subst_arrow]
    obtain ⟨hpl, hlen, hnc, hpb⟩ := plain_cb hp
    have hhb : ∀ n ∈ callHeads body, (enterParams body ps s ps).2.get n = .none := fun n hn => by
      rw [enterParams_get body ps s ps (fun hmem => hnc n hmem hn)]
      exact hh n (by simp [callHeads, callHeadsList, hn])
    have hmb : MapSmall (enterParams body ps s ps).2 := by
      intro r hr; rw [enterParams_map] at hr; exact hm r hr
    exact .map (hs.fns ▸ hfd)
      (sfE hs hl hf hm (fun y hy => hA y (by simp [fv, fvList, hy])) hpl
        (fun n hn => hh n (by simp [callHeads, callHeadsList, hn])))
      (sfM hs hmap hf hmb
        (fun x i => agree_params hm hlen
          (fun y hy => hA y (by simp [fv, fvList] at hy ⊢; grind)) x i)
        hpb hhb)
  | .filter hfd hl hmap => by
    rename_i l xs ps body ys
    rw [subst_call (hh "filter" (by simp [callHeads]))]
    simp only [substList, subst_arrow]
    obtain ⟨hpl, hlen, hnc, hpb⟩ := plain_cb hp
    have hhb : ∀ n ∈ callHeads body, (enterParams body ps s ps).2.get n = .none := fun n hn => by
      rw [enterParams_get body ps s ps (fun hmem => hnc n hmem hn)]
      exact hh n (by simp [callHeads, callHeadsList, hn])
    have hmb : MapSmall (enterParams body ps s ps).2 := by
      intro r hr; rw [enterParams_map] at hr; exact hm r hr
    exact .filter (hs.fns ▸ hfd)
      (sfE hs hl hf hm (fun y hy => hA y (by simp [fv, fvList, hy])) hpl
        (fun n hn => hh n (by simp [callHeads, callHeadsList, hn])))
      (sfF hs hmap hf hmb
        (fun x i => agree_params hm hlen
          (fun y hy => hA y (by simp [fv, fvList] at hy ⊢; grind)) x i)
        hpb hhb)
  | .pendingSettled .. | .pendingOther .. | .failedSettled .. | .failedOther .. => by
    simp [Plain] at hp
  | .stdlib hfd ha hv => by
    rename_i name _ _
    rw [subst_call (hh name (by simp [callHeads]))]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .stdlib (hs.fns ▸ hfd)
      (sfL hs ha hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun n hn => hh n (by simp [callHeads, hn])))
      (hs.stdlib_eq _ _ ▸ hv)
  | .record hd h => by
    simp only [substExpr]
    simp only [Plain, Bool.true_and] at hp
    exact .record (hs.shape _ ▸ hd)
      (sfR hs h hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp
        (fun n hn => hh n (by simp [callHeads, hn])))
  | .recordBase hb hd h => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .recordBase
      (sfE hs hb hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun n hn => hh n (by simp [callHeads, hn])))
      (hs.shape _ ▸ hd)
      (sfR hs h hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun n hn => hh n (by simp [callHeads, hn])))
  | .neg h => by
    simp only [substExpr]
    exact .neg (sfE hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh))
  | .not h => by
    simp only [substExpr]
    exact .not (sfE hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh))
  | .andFalse h => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .andFalse (sfE hs h hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .andTrue h1 h2 => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .andTrue (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
      (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .orTrue h => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .orTrue (sfE hs h hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .orFalse h1 h2 => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .orFalse (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
      (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .binop h1 h2 h3 => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .binop (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
      (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
      (fun n hn => hh n (by simp [callHeads, hn]))) h3
  | .ternaryTrue h1 h2 => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .ternaryTrue (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.1
      (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.2
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .ternaryFalse h1 h2 => by
    simp only [substExpr]
    simp only [Plain, Bool.and_eq_true] at hp
    exact .ternaryFalse (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.1
      (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
      (fun n hn => hh n (by simp [callHeads, hn])))
  | .matchSome h1 h2 => by
    rename_i subj w x a b
    rw [subst_match]
    obtain ⟨hps, hnc, hpa, hpb⟩ := plain_match hp
    have hma : MapSmall (s.enter x (allNames a)).2 := by
      intro r hr; rw [enter_map] at hr; exact hm r hr
    exact .matchSome
      (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hps
        (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hma
        (agree_enter hm (fun y hy hne => hA y (by simp [fv, hy, hne])) (fun y hy => .inr (fv_sub_all _ hy)))
        hpa
        (fun n hn => by
          rw [enter_get_other s x _ (fun he => by subst he; exact hnc hn)]
          exact hh n (by simp [callHeads, hn])))
  | .matchNone h1 h2 => by
    rw [subst_match]
    obtain ⟨hps, _, _, hpb⟩ := plain_match hp
    exact .matchNone
      (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hps
        (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hpb
        (fun n hn => hh n (by simp [callHeads, hn])))
  | .letE h1 h2 => by
    rename_i v0 w x body
    rw [subst_let]
    obtain ⟨hpv, hnc, hpb⟩ := plain_let hp
    have hma : MapSmall (s.enter x (allNames body)).2 := by
      intro r hr; rw [enter_map] at hr; exact hm r hr
    exact .letE
      (sfE hs h1 hf hm (fun y hy => hA y (by simp [fv, fvOpt, hy])) hpv
        (fun n hn => hh n (by simp [callHeads, hn])))
      (sfE hs h2 hf hma
        (agree_enter hm (fun y hy hne => hA y (by simp [fv, hy, hne])) (fun y hy => .inr (fv_sub_all _ hy)))
        hpb
        (fun n hn => by
          rw [enter_get_other s x _ (fun he => by subst he; exact hnc hn)]
          exact hh n (by simp [callHeads, hn])))
  | .typed h => by
    simp only [substExpr]
    exact .typed (sfE hs h hf hm (by simpa [fv] using hA) (by simpa [Plain] using hp)
      (by simpa [callHeads] using hh))
theorem sfL {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls es vs} (h : ListR e₁ inFn ls es vs)
    (hf : inFn = false) {s : Subst} {Lf : Locals} (hm : MapSmall s)
    (hA : ∀ y ∈ fvList es, Agree e₁ ls e₂ Lf s y) (hp : PlainList es = true)
    (hh : ∀ n ∈ callHeadsList es, s.get n = .none) : ListR e₂ false Lf (substList s es) vs :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => by
    simp only [substList]
    simp only [PlainList, Bool.and_eq_true] at hp
    exact .cons (sfE hs h1 hf hm (fun y hy => hA y (by simp [fvList, hy])) hp.1
      (fun n hn => hh n (by simp [callHeadsList, hn])))
      (sfL hs h2 hf hm (fun y hy => hA y (by simp [fvList, hy])) hp.2
      (fun n hn => hh n (by simp [callHeadsList, hn])))
theorem sfD {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls es ss} (h : DisplaysR e₁ inFn ls es ss)
    (hf : inFn = false) {s : Subst} {Lf : Locals} (hm : MapSmall s)
    (hA : ∀ y ∈ fvList es, Agree e₁ ls e₂ Lf s y) (hp : PlainList es = true)
    (hh : ∀ n ∈ callHeadsList es, s.get n = .none) : DisplaysR e₂ false Lf (substList s es) ss :=
  match h with
  | .nil => .nil
  | .cons h1 hd h2 => by
    simp only [substList]
    simp only [PlainList, Bool.and_eq_true] at hp
    exact .cons (sfE hs h1 hf hm (fun y hy => hA y (by simp [fvList, hy])) hp.1
      (fun n hn => hh n (by simp [callHeadsList, hn]))) hd
      (sfD hs h2 hf hm (fun y hy => hA y (by simp [fvList, hy])) hp.2
      (fun n hn => hh n (by simp [callHeadsList, hn])))
theorem sfM {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls ps body xs i ys}
    (h : MapR e₁ inFn ls ps body xs i ys) (hf : inFn = false) {s : Subst} {Lf : Locals}
    {ps' : List String} (hm : MapSmall s)
    (hA : ∀ x i, ∀ y ∈ fv body, Agree e₁ (bindParams ps x i ls) e₂ (bindParams ps' x i Lf) s y)
    (hp : Plain body = true) (hh : ∀ n ∈ callHeads body, s.get n = .none) :
    MapR e₂ false Lf ps' (substExpr s body) xs i ys :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => .cons (sfE hs h1 hf hm (hA _ _) hp hh) (sfM hs h2 hf hm hA hp hh)
theorem sfF {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls ps body xs i ys}
    (h : FilterR e₁ inFn ls ps body xs i ys) (hf : inFn = false) {s : Subst} {Lf : Locals}
    {ps' : List String} (hm : MapSmall s)
    (hA : ∀ x i, ∀ y ∈ fv body, Agree e₁ (bindParams ps x i ls) e₂ (bindParams ps' x i Lf) s y)
    (hp : Plain body = true) (hh : ∀ n ∈ callHeads body, s.get n = .none) :
    FilterR e₂ false Lf ps' (substExpr s body) xs i ys :=
  match h with
  | .nil => .nil
  | .keep h1 h2 => .keep (sfE hs h1 hf hm (hA _ _) hp hh) (sfF hs h2 hf hm hA hp hh)
  | .drop h1 h2 => .drop (sfE hs h1 hf hm (hA _ _) hp hh) (sfF hs h2 hf hm hA hp hh)
theorem sfR {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn ls w b fs i vs} (h : FieldsR e₁ inFn ls w b fs i vs)
    (hf : inFn = false) {s : Subst} {Lf : Locals} (hm : MapSmall s)
    (hA : ∀ y ∈ fvFields w, Agree e₁ ls e₂ Lf s y) (hp : PlainFields w = true)
    (hh : ∀ n ∈ callHeadsFields w, s.get n = .none) : FieldsR e₂ false Lf (substFields s w) b fs i vs :=
  match h with
  | .nil => .nil
  | .written he h1 h2 => by
    obtain ⟨k, hk⟩ := lookupField_mem he
    obtain ⟨m1, m2, m3⟩ := fields_mem hk
    exact .written (by rw [lookupField_subst, he]; rfl)
      (sfE hs h1 hf hm (fun y hy => hA y (fields_fv hk y hy)) (m2 hp) (fun n hn => hh n (m3 n hn)))
      (sfR hs h2 hf hm hA hp hh)
  | .base he hb h2 => .base (by rw [lookupField_subst, he]; rfl) hb (sfR hs h2 hf hm hA hp hh)
end

end Contract.ExpandSubst
