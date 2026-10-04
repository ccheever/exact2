/-
Type soundness for expressions and statements: a well-typed expression,
evaluated where the names it reads hold values of their types, has a value
of its type, or fails only in a way the semantics means to fail — a read
of a derive or resource not settled yet (`pending`), an entry the
semantics leaves out (`unsupported`), or a refusal (`refused`: a row slot
read outside its row, out of fuel). Never a type error and never an
unbound name.

`GoodR P r` says that of a result. Preservation and progress are one
theorem, by induction on the interpreter's fuel.
-/
import Contract.Types
import Contract.Big

namespace Contract

/-- A failure the semantics means: not a type error, not an unbound name. -/
def Legit : Err → Prop
  | .pending | .unsupported _ | .refused _ => True
  | _ => False

/-- A value with property `P`, or a legitimate failure. -/
def GoodR {α} (P : α → Prop) : Result α → Prop
  | .ok a => P a
  | .error e => Legit e

theorem GoodR.bind {α β} {P : α → Prop} {Q : β → Prop} {r : Result α} {f : α → Result β}
    (h : GoodR P r) (hf : ∀ a, P a → GoodR Q (f a)) : GoodR Q (r >>= f) := by
  cases r with
  | ok a => exact hf a h
  | error e => exact h

/-- Values pairwise of types. -/
def ValTyL (p : Program) : List Value → List Ty → Prop
  | [], [] => True
  | v :: vs, t :: ts => ValTy p v t ∧ ValTyL p vs ts
  | _, _ => False

section
variable {p : Program} {v : Value}
theorem ValTy.num_inv (h : ValTy p v .number) : ∃ f, v = .num f := by cases v <;> simp_all [ValTy]
theorem ValTy.str_inv (h : ValTy p v .string) : ∃ s, v = .str s := by cases v <;> simp_all [ValTy]
theorem ValTy.bool_inv (h : ValTy p v .bool) : ∃ b, v = .bool b := by cases v <;> simp_all [ValTy]
theorem ValTy.list_inv {a} (h : ValTy p v (.list a)) : ∃ xs, v = .list xs ∧ ValTys p xs a := by
  cases v <;> simp_all [ValTy]
theorem ValTy.option_inv {a} (h : ValTy p v (.option a)) : v = .none ∨ ∃ w, v = .some w ∧ ValTy p w a := by
  cases v <;> simp_all [ValTy]
theorem ValTy.record_inv {s} (h : ValTy p v (.record s)) :
    ∃ vs sh, v = .record s vs ∧ p.shapes.find? (·.name == s) = .some sh ∧ FieldsTy p vs sh.fields := by
  cases v <;> simp [ValTy] at h
  case record s' vs => obtain ⟨rfl, sh, h1, h2⟩ := h; exact ⟨vs, sh, rfl, h1, h2⟩
theorem ValTyL.nil_inv {vs : List Value} (h : ValTyL p vs []) : vs = [] := by
  cases vs <;> simp_all [ValTyL]
theorem ValTyL.cons_inv {vs : List Value} {t ts} (h : ValTyL p vs (t :: ts)) :
    ∃ w ws, vs = w :: ws ∧ ValTy p w t ∧ ValTyL p ws ts := by
  cases vs <;> simp_all [ValTyL]
  exact ⟨_, _, ⟨rfl, rfl⟩, h⟩
end

theorem display_ok {p : Program} {v : Value} {t : Ty} (h : ValTy p v t) (ht : t.displayable = true) :
    ∃ s, v.display = .ok s := by
  cases t <;> simp [Ty.displayable] at ht
  all_goals first
    | (obtain ⟨_, rfl⟩ := h.num_inv; exact ⟨_, rfl⟩)
    | (obtain ⟨_, rfl⟩ := h.str_inv; exact ⟨_, rfl⟩)
    | (obtain ⟨_, rfl⟩ := h.bool_inv; exact ⟨_, rfl⟩)

theorem mapM_good {α β} {f : α → Result β} : ∀ {xs : List α},
    (∀ x ∈ xs, GoodR (fun _ => True) (f x)) → GoodR (fun _ => True) (xs.mapM f)
  | [], _ => trivial
  | x :: xs, h => by
    simp only [List.mapM_cons]
    exact GoodR.bind (h x (by simp)) fun _ _ =>
      GoodR.bind (mapM_good fun y hy => h y (by simp [hy])) fun _ _ => trivial

theorem ite_pos {α} {c : Prop} [Decidable c] {a b : α} (h : c) : (if c then a else b) = a := by simp [h]
theorem ite_neg {α} {c : Prop} [Decidable c] {a b : α} (h : ¬ c) : (if c then a else b) = b := by simp [h]

/-- The entries the semantics leaves out are refused as unsupported. -/
theorem stdlib_unsupported {env : Env} {p : Program} {name : String} {vs : List Value} {t : Ty}
    {ts : List Ty} (h : unsupportedTy name ts = .some t) : GoodR (ValTy p · t) (stdlib env name vs) := by
  delta unsupportedTy at h
  by_cases hn : name = "formatTime" ∨ name = "formatDate"
  · rcases hn with rfl | rfl <;> simp [stdlib, GoodR, Legit]
  rw [ite_neg hn] at h
  by_cases hn : name = "formatNumber"
  · subst hn; simp [stdlib, GoodR, Legit]
  rw [ite_neg hn] at h
  by_cases hn : name = "frame" ∨ name = "measure"
  · rcases hn with rfl | rfl <;> simp [stdlib, GoodR, Legit]
  rw [ite_neg hn] at h
  simp at h

/-! ## The router's values -/

theorem fieldsTy_of_tys {p : Program} : ∀ {vs : List Value} {fs : List Field},
    ValTyL p vs (fs.map (·.ty)) → FieldsTy p vs fs
  | [], [], _ => trivial
  | v :: vs, f :: fs, h => by
    simp only [List.map_cons, ValTyL] at h
    exact ⟨h.1, fieldsTy_of_tys h.2⟩
  | [], _ :: _, h | _ :: _, [], h => by simp [ValTyL] at h

theorem recordTy_of {p : Program} {s : String} {vs : List Value} {ts : List Ty}
    (hs : shapeTys p s = .some ts) (hv : ValTyL p vs ts) : ValTy p (.record s vs) (.record s) := by
  simp only [shapeTys, Option.map_eq_some_iff] at hs
  obtain ⟨sh, hsh, rfl⟩ := hs
  exact ⟨rfl, sh, hsh, fieldsTy_of_tys hv⟩

theorem shapeTys_of_valTy {p : Program} {v : Value} {s : String} (h : ValTy p v (.record s)) :
    (shapeTys p s).isSome = true := by
  obtain ⟨_, sh, _, hsh, _⟩ := h.record_inv
  simp [shapeTys, hsh]

/-- The router shapes, as `routeShapesOK` says they are. -/
structure RouteShapes (p : Program) : Prop where
  router : shapeTys p "Router" = .some [.string, .list (.record "Tab"), .number]
  tab : shapeTys p "Tab" = .some [.string, .list (.record "Entry")]
  entry : shapeTys p "Entry" = .some [.number, .string, .string, .string, .record "Params"]
  params : shapeTys p "Params" = .some ((Route.paramNames p.routes).map fun _ => .string)

theorem RouteShapes.of {p : Program} (hr : routeShapesOK p = true)
    (hpres : (shapeTys p "Router").isSome = true) : RouteShapes p := by
  simp only [routeShapesOK, Bool.or_eq_true, Bool.not_eq_true', Bool.and_eq_true, decide_eq_true_eq] at hr
  rcases hr with hr | ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩
  · simp_all
  · exact ⟨h1, h2, h3, h4⟩

theorem paramsValue_ty {p : Program} (H : RouteShapes p) (ps : Route.Params) :
    ValTy p (Route.paramsValue p.routes ps) (.record "Params") := by
  refine recordTy_of H.params ?_
  generalize Route.paramNames p.routes = names
  induction names with
  | nil => trivial
  | cons n ns ih => exact ⟨by simp [ValTy], ih⟩

theorem entryValue_ty {p : Program} (H : RouteShapes p) (e : Route.Entry) :
    ValTy p (Route.entryValue p.routes e) (.record "Entry") :=
  recordTy_of H.entry ⟨trivial, trivial, trivial, trivial, paramsValue_ty H _, trivial⟩

theorem entries_ty {p : Program} (H : RouteShapes p) : ∀ (es : List Route.Entry),
    ValTys p (es.map (Route.entryValue p.routes)) (.record "Entry")
  | [] => trivial
  | e :: es => ⟨entryValue_ty H e, entries_ty H es⟩

theorem routerValue_ty {p : Program} (H : RouteShapes p) (r : Route.Router) :
    ValTy p (Route.routerValue p.routes r) (.record "Router") := by
  refine recordTy_of H.router ⟨trivial, ?_, trivial, trivial⟩
  simp only [ValTy]
  generalize r.tabs = tabs
  induction tabs with
  | nil => trivial
  | cons tb tbs ih =>
    exact ⟨recordTy_of H.tab ⟨trivial, entries_ty H tb.stack, trivial⟩, ih⟩

theorem verb_good {p : Program} {v : Value} {f : Route.Router → Route.Verb} (H : RouteShapes p)
    (hv : ValTy p v (.record "Router")) :
    GoodR (ValTy p · (.record "Router")) (Route.verb p.routes v f) := by
  unfold Route.verb
  split
  · next r _ =>
    split
    · exact routerValue_ty H _
    · exact hv
  · trivial

theorem read_good {p : Program} {v : Value} {f : Route.Router → Option Value} {t : Ty}
    (hf : ∀ r w, f r = .some w → ValTy p w t) : GoodR (ValTy p · t) (Route.read p.routes v f) := by
  unfold Route.read
  split
  · next w hw =>
    simp only [Option.bind_eq_some_iff] at hw
    obtain ⟨r, -, hr⟩ := hw
    exact hf r w hr
  · trivial

/-- The router's verbs and reads on values of their types: a value of the
type `routerTy` gives, or a refusal (a value of the shape `Router` that is
not a valid router traps). -/
theorem stdlib_router {env : Env} {p : Program} {name : String} {vs : List Value} {ts : List Ty} {t : Ty}
    (hp : env.prog = p) (hr : routeShapesOK p = true) (hv : ValTyL p vs ts) (h : routerTy name ts = .some t) :
    GoodR (ValTy p · t) (stdlib env name vs) := by
  delta routerTy at h
  have hshape : ∀ {w : Value}, ValTy p w (.record "Router") → RouteShapes p :=
    fun hw => RouteShapes.of hr (shapeTys_of_valTy hw)
  by_cases hn : name = "open" ∨ name = "push" ∨ name = "replace" ∨ name = "select" ∨ name = "go"
  · rw [ite_pos hn] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨_, rfl⟩ := hw'.str_inv
    have H := hshape hw
    subst hp
    rcases hn with rfl | rfl | rfl | rfl | rfl <;> simp only [stdlib] <;> exact verb_good H hw
  rw [ite_neg hn] at h
  by_cases hn : name = "back"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    have H := hshape hw
    subst hp
    simp only [stdlib]; exact verb_good H hw
  rw [ite_neg hn] at h
  by_cases hn : name = "stack"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    have H := hshape hw
    subst hp
    simp only [stdlib]
    exact read_good fun r w hw' => by simp at hw'; subst hw'; exact entries_ty H _
  rw [ite_neg hn] at h
  by_cases hn : name = "top"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    have H := hshape hw
    subst hp
    simp only [stdlib]
    exact read_good fun r w hw' => by
      simp only [Option.map_eq_some_iff] at hw'
      obtain ⟨e, -, rfl⟩ := hw'
      exact entryValue_ty H e
  rw [ite_neg hn] at h
  by_cases hn : name = "depth"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    subst hp
    simp only [stdlib]
    exact read_good fun r w hw' => by simp at hw'; subst hw'; simp [ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "params"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨_, rfl⟩ := hw'.str_inv
    subst hp
    simp only [stdlib]
    refine read_good fun r w hw' => ?_
    simp at hw'; subst hw'
    simp only [ValTy]
    exact ValTys.of_mem fun x hx => by
      simp only [List.mem_map] at hx
      obtain ⟨_, -, rfl⟩ := hx
      simp [ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "searchParam"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next r =>
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨_, rfl⟩ := hw'.str_inv
    simp only [stdlib]
    split <;> simp [GoodR, ValTy, Legit]
  rw [ite_neg hn] at h
  by_cases hn : name = "encodeRouteSegment"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    obtain ⟨_, rfl⟩ := hw.str_inv
    simp only [stdlib]
    split <;> simp [GoodR, ValTy, Legit]
  rw [ite_neg hn] at h
  exact stdlib_unsupported h

theorem mapM_len_good {α β} {f : α → Result β} : ∀ {xs : List α}, (∀ x ∈ xs, ∃ y, f x = .ok y) →
    GoodR (fun ys => ys.length = xs.length) (xs.mapM f)
  | [], _ => rfl
  | x :: xs, h => by
    obtain ⟨y, hy⟩ := h x (by simp)
    simp only [List.mapM_cons, hy]
    exact GoodR.bind (P := fun _ => True) trivial fun _ _ =>
      GoodR.bind (mapM_len_good fun z hz => h z (by simp [hz])) fun ys hys => by
        simp [GoodR, pure, Except.pure, hys]

/-- `path("route", args…)` of a declared route, one string or number per
parameter: a string, or a refusal (a parameter that is empty, `.` or
`..`). -/
theorem pathValue_good {t : Route.Table} {name : String} {route : RouteDecl} {vs : List Value}
    (hroute : t.find? (fun r => r.name == name && !r.notfound) = .some route)
    (hvs : ∀ v ∈ vs, (∃ s, v = .str s) ∨ (∃ f, v = .num f))
    (hlen : vs.length = patternParams route.pattern) :
    GoodR (fun v => ∃ s, v = Value.str s) (Route.pathValue t name vs) := by
  have go : ∀ (segs : List String) (first : Bool) (xs : List String),
      (segs.filter (Route.startsWith · ':')).length ≤ xs.length →
      GoodR (fun _ => True) (Route.pathValue.go first segs xs) := by
    intro segs
    induction segs with
    | nil => intro first xs _; simp [Route.pathValue.go, GoodR]
    | cons seg segs ih =>
      intro first xs hle
      simp only [Route.pathValue.go]
      by_cases hc : Route.startsWith seg ':' = true
      · simp only [List.filter_cons, hc, ite_true, List.length_cons] at hle
        cases xs with
        | nil => simp at hle
        | cons x xs =>
          simp only [List.length_cons] at hle
          simp only [hc, ite_true]
          split
          · exact GoodR.bind (P := fun pv => pv.2 = xs) (by simp only [pure, Except.pure, GoodR])
              fun pv hpv => by
                rw [hpv]; exact GoodR.bind (ih false xs (by omega)) fun _ _ => trivial
          · exact GoodR.bind (P := fun _ => False) (by simp [GoodR, Legit]) fun _ h => h.elim
      · simp only [List.filter_cons, hc, Bool.false_eq_true, ite_false] at hle
        simp only [hc, Bool.false_eq_true, ite_false]
        exact GoodR.bind (P := fun pv => pv.2 = xs) (by simp only [pure, Except.pure, GoodR])
          fun pv hpv => by rw [hpv]; exact GoodR.bind (ih false xs hle) fun _ _ => trivial
  unfold Route.pathValue
  simp only [hroute]
  refine GoodR.bind (P := fun ss => ss.length = vs.length) (mapM_len_good fun v hv => ?_) ?_
  · rcases hvs v hv with ⟨s, rfl⟩ | ⟨f, rfl⟩ <;> exact ⟨_, rfl⟩
  intro ss hl
  have hg := go (Route.split route.pattern '/') true ss (by simp only [patternParams] at hlen; omega)
  revert hg
  cases Route.pathValue.go true (Route.split route.pattern '/') ss with
  | ok s => intro _; exact ⟨s, rfl⟩
  | error e => intro hg; exact hg

/-- A roster call on arguments of the types `rosterTy` takes answers a
value of the type it gives, or is unsupported; never a type error. -/
theorem stdlib_good {env : Env} {p : Program} {name : String} {vs : List Value} {ts : List Ty} {t : Ty}
    (hp : env.prog = p) (hrs : routeShapesOK p = true) (hv : ValTyL p vs ts) (h : rosterTy name ts = .some t) : GoodR (ValTy p · t) (stdlib env name vs) := by
  delta rosterTy at h
  by_cases hn : name = "now"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h; subst h; rw [hv.nil_inv]; simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "length"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h <;> subst h
    all_goals obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    · obtain ⟨_, rfl⟩ := hw.str_inv; simp [stdlib, GoodR, ValTy]
    · obtain ⟨_, rfl, _⟩ := hw.list_inv; simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "isEmpty"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h <;> subst h
    all_goals obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    · obtain ⟨_, rfl⟩ := hw.str_inv; simp [stdlib, GoodR, ValTy]
    · obtain ⟨_, rfl, _⟩ := hw.list_inv; simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "toString"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next u =>
    obtain ⟨hd, rfl⟩ := h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    obtain ⟨s, hs⟩ := display_ok hw hd
    simp [stdlib, hs, GoodR, ValTy, Functor.map, Except.map]
  rw [ite_neg hn] at h
  by_cases hn : name = "floor"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h; subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    obtain ⟨_, rfl⟩ := hw.num_inv; simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "max" ∨ name = "min"
  · rw [ite_pos hn] at h
    split at h <;> simp at h; subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨_, rfl⟩ := hw.num_inv; obtain ⟨_, rfl⟩ := hw'.num_inv
    rcases hn with rfl | rfl <;> simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "first"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h; subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    obtain ⟨xs, rfl, hxs⟩ := hw.list_inv
    cases xs with
    | nil => simp [stdlib, GoodR, ValTy]
    | cons x xs => simp only [ValTys] at hxs; simp [stdlib, GoodR, ValTy, hxs.1]
  rw [ite_neg hn] at h
  by_cases hn : name = "at"
  · subst hn; rw [ite_pos rfl] at h
    split at h <;> simp at h
    next a =>
    subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨xs, rfl, hxs⟩ := hw.list_inv; obtain ⟨_, rfl⟩ := hw'.num_inv
    have hat : ∀ (c : Prop) [Decidable c] (k : Nat), GoodR (fun x => ValTy p x (.option a))
        (if c then Except.ok (match xs[k]? with | .some v => Value.some v | .none => Value.none)
          else (Except.ok Value.none : Result Value)) := by
      intro c _ k
      split
      · split
        · next v hv' => exact hxs.mem v (List.mem_of_getElem? hv')
        · trivial
      · trivial
    simp only [stdlib]
    split <;> exact hat _ _
  rw [ite_neg hn] at h
  by_cases hn : name = "includes" ∨ name = "startsWith" ∨ name = "endsWith"
  · rw [ite_pos hn] at h
    split at h <;> simp at h; subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
    obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
    obtain ⟨_, rfl⟩ := hw.str_inv; obtain ⟨_, rfl⟩ := hw'.str_inv
    rcases hn with rfl | rfl | rfl <;> simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "trim" ∨ name = "encodeURIComponent"
  · rw [ite_pos hn] at h
    split at h <;> simp at h; subst h
    obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv; rw [hws.nil_inv]
    obtain ⟨_, rfl⟩ := hw.str_inv
    rcases hn with rfl | rfl <;> simp [stdlib, GoodR, ValTy]
  rw [ite_neg hn] at h
  by_cases hn : name = "join"
  · subst hn; rw [ite_pos rfl] at h
    split at h
    next a sep =>
      split at h <;> simp at h; subst h
      next hc =>
      simp only [Bool.and_eq_true] at hc
      obtain ⟨w, ws, rfl, hw, hws⟩ := hv.cons_inv
      obtain ⟨w', ws', rfl, hw', hws'⟩ := hws.cons_inv; rw [hws'.nil_inv]
      obtain ⟨xs, rfl, hxs⟩ := hw.list_inv
      have hsep : sep = .string ∨ sep = .unknown := by
        cases sep <;> simp [Ty.le] at hc ⊢
      rcases hsep with rfl | rfl
      · obtain ⟨sv, rfl⟩ := hw'.str_inv
        simp only [stdlib]
        split
        · trivial
        · refine GoodR.bind (P := fun _ => True) ?_ (fun _ _ => trivial)
          apply mapM_good
          intro x hx
          have hx' := hxs.mem x hx
          have hd := hc.1
          cases a <;> simp [Ty.displayable] at hd
          all_goals first
            | exact absurd hx' ValTy.unknown
            | (obtain ⟨_, rfl⟩ := hx'.num_inv; simp [GoodR, Value.display])
            | (obtain ⟨_, rfl⟩ := hx'.str_inv; simp [GoodR, Value.display])
            | (obtain ⟨_, rfl⟩ := hx'.bool_inv; simp [GoodR, Value.display])
      · exact absurd hw' ValTy.unknown
    next => simp at h
  rw [ite_neg hn] at h
  exact stdlib_router hp hrs hv h

/-! ## Scopes and environments -/

/-- The component names an expression reads in `G` have values of their
types in `env`, or reading them fails legitimately (a derive not settled
yet is `pending`, a row slot outside its row is refused). -/
def EnvOK (p : Program) (G : Scope) (env : Env) : Prop :=
  env.prog = p ∧ ∀ x t, lookupTy x G = .some t → GoodR (ValTy p · t) (env.global x)

/-- The locals bound are exactly the names `Γ` types, at their types. -/
def LocalsOK (p : Program) (Γ : Scope) (ls : Locals) : Prop :=
  ∀ x, match lookupTy x Γ, lookup x ls with
    | .some t, .some v => ValTy p v t
    | .none, .none => True
    | _, _ => False

/-- What evaluation needs of the program itself: every `fn` body has its
declared type in a scope of its parameters, and the router shapes are
`Contract.Route`'s. -/
def ProgOK (p : Program) : Prop :=
  (∀ fd ∈ p.fns, ∃ t, HasTy p [] fd.params.reverse fd.body t ∧ t.le fd.ret = true) ∧
    routeShapesOK p = true

/-- Where an evaluation runs: names and locals of their types; a `fn`
body (`inFn`) reads no component name. -/
def Ctx (p : Program) (G : Scope) (env : Env) (inFn : Bool) (Γ : Scope) (ls : Locals) : Prop :=
  EnvOK p G env ∧ (inFn = true → G = []) ∧ LocalsOK p Γ ls

theorem LocalsOK.nil {p : Program} : LocalsOK p [] [] := fun x => by simp [lookupTy, lookup]

theorem LocalsOK.cons {p : Program} {Γ ls x t v} (h : LocalsOK p Γ ls) (hv : ValTy p v t) :
    LocalsOK p ((x, t) :: Γ) ((x, v) :: ls) := fun y => by
  by_cases hxy : (y == x) = true
  · simp only [lookupTy, lookup, ite_pos hxy]; exact hv
  · simp only [lookupTy, lookup, ite_neg hxy]; exact h y

theorem LocalsOK.bind {p : Program} {Γ ls a x} (h : LocalsOK p Γ ls) (hx : ValTy p x a) (ps : List String) (i : Nat) :
    LocalsOK p (bindTy ps a Γ) (bindParams ps x i ls) := by
  match ps with
  | [] => exact h
  | [q] => exact h.cons hx
  | q :: r :: _ => exact (h.cons hx).cons (by simp [ValTy])

theorem LocalsOK.lookup_some {p : Program} {Γ ls x t} (h : LocalsOK p Γ ls) (hx : lookupTy x Γ = .some t) :
    ∃ v, lookup x ls = .some v ∧ ValTy p v t := by
  have := h x
  rw [hx] at this
  cases hv : lookup x ls with
  | none => rw [hv] at this; exact this.elim
  | some v => rw [hv] at this; exact ⟨v, rfl, this⟩

theorem LocalsOK.lookup_none {p : Program} {Γ ls x} (h : LocalsOK p Γ ls) (hx : lookupTy x Γ = .none) :
    lookup x ls = .none := by
  have := h x
  rw [hx] at this
  cases hv : lookup x ls with
  | none => rfl
  | some v => rw [hv] at this; exact this.elim

/-- Names bound pairwise with types: a `fn`'s parameters and arguments. -/
inductive Agree (p : Program) : Locals → Scope → Prop
  | nil : Agree p [] []
  | cons : ValTy p v t → Agree p ls Γ → Agree p ((x, v) :: ls) ((x, t) :: Γ)

theorem Agree.append {p : Program} : ∀ {l₁ g₁ l₂ g₂}, Agree p l₁ g₁ → Agree p l₂ g₂ → Agree p (l₁ ++ l₂) (g₁ ++ g₂)
  | _, _, _, _, .nil, h => h
  | _, _, _, _, .cons hv h₁, h₂ => .cons hv (Agree.append h₁ h₂)

theorem Agree.reverse {p : Program} : ∀ {ls Γ}, Agree p ls Γ → Agree p ls.reverse Γ.reverse
  | _, _, .nil => .nil
  | _, _, .cons hv h => by
    simpa using Agree.append (Agree.reverse h) (.cons hv .nil)

theorem Agree.localsOK {p : Program} : ∀ {ls Γ}, Agree p ls Γ → LocalsOK p Γ ls
  | _, _, .nil => LocalsOK.nil
  | _, _, .cons hv h => (Agree.localsOK h).cons hv

theorem Agree.params {p : Program} : ∀ {params : List (String × Ty)} {vs ts},
    ValTyL p vs ts → Ty.leAll ts (params.map (·.2)) = true →
    Agree p ((params.map (·.1)).zip vs) params
  | [], [], [], _, _ => .nil
  | (x, u) :: params, v :: vs, t :: ts, hv, hl => by
    simp only [ValTyL] at hv
    simp only [List.map_cons, Ty.leAll, Bool.and_eq_true] at hl
    exact .cons (hv.1.mono hl.1) (Agree.params hv.2 hl.2)
  | [], _ :: _, [], hv, _ => by simp [ValTyL] at hv
  | [], _, _ :: _, _, hl => by simp [Ty.leAll] at hl
  | _ :: _, [], [], _, hl => by simp [Ty.leAll] at hl
  | _ :: _, _, [], _, hl => by simp [Ty.leAll] at hl
  | _ :: _, [], _ :: _, hv, _ => by simp [ValTyL] at hv

/-! ## Records -/

theorem NamedTy.lookup {p : Program} {G Γ} : ∀ {written wts}, NamedTy p G Γ written wts → ∀ x,
    (lookupField x written = .none ∧ lookupTy x wts = .none) ∨
    ∃ e t, lookupField x written = .some e ∧ lookupTy x wts = .some t ∧ HasTy p G Γ e t
  | _, _, .nil, _ => .inl ⟨rfl, rfl⟩
  | _, _, .cons he hr, x => by
    simp only [lookupField, lookupTy]
    split
    · exact .inr ⟨_, _, rfl, rfl, he⟩
    · exact NamedTy.lookup hr x

theorem FieldsTy.drop_cons {p : Program} {bs : List Value} {i : Nat} {f : Field} {fs : List Field}
    (h : FieldsTy p (bs.drop i) (f :: fs)) :
    ∃ v, bs[i]? = .some v ∧ ValTy p v f.ty ∧ FieldsTy p (bs.drop (i + 1)) fs := by
  obtain ⟨v, hv, hvt⟩ := FieldsTy.get (i := 0) h rfl
  refine ⟨v, by simpa using hv, hvt, ?_⟩
  have h2 := FieldsTy.drop 1 h
  simpa [List.drop_drop, Nat.add_comm] using h2

/-! ## Expressions -/

theorem GoodR.mono {α} {P Q : α → Prop} {r : Result α} (h : GoodR P r) (hq : ∀ a, P a → Q a) : GoodR Q r := by
  cases r <;> simp_all [GoodR]

theorem binop_good {p : Program} {op : BinOp} {va vb : Value} {ta tb t : Ty}
    (ha : ValTy p va ta) (hb : ValTy p vb tb) (h : binTy op ta tb = .some t) (hand : op ≠ .and) (hor : op ≠ .or) :
    GoodR (ValTy p · t) (binop op va vb) := by
  cases op <;> simp only [binTy] at h
  case add =>
    split at h
    · next hn =>
      simp only [Bool.and_eq_true] at hn
      simp at h; subst h
      obtain ⟨x, rfl⟩ := (ha.mono hn.1).num_inv
      obtain ⟨y, rfl⟩ := (hb.mono hn.2).num_inv
      simp [binop, ValTy, GoodR]
    · split at h
      · next hs =>
        simp only [Bool.and_eq_true] at hs
        simp at h; subst h
        obtain ⟨x, rfl⟩ := (ha.mono hs.1).str_inv
        obtain ⟨y, rfl⟩ := (hb.mono hs.2).str_inv
        simp [binop, ValTy, GoodR]
      · simp at h
  case eq | ne =>
    split at h
    · next hc =>
      simp at h; subst h
      have := equal_isSome va vb ha hb hc
      simp only [binop]
      cases he : Value.equal va vb <;> simp_all [GoodR, ValTy]
    · simp at h
  case and => exact absurd rfl hand
  case or => exact absurd rfl hor
  all_goals
    split at h
    · next hn =>
      simp only [Bool.and_eq_true] at hn
      simp at h; subst h
      obtain ⟨x, rfl⟩ := (ha.mono hn.1).num_inv
      obtain ⟨y, rfl⟩ := (hb.mono hn.2).num_inv
      simp [binop, ValTy, GoodR]
    · simp at h

theorem ListTy.length {p : Program} {G Γ} : ∀ {es ts}, ListTy p G Γ es ts → es.length = ts.length
  | _, _, .nil => rfl
  | _, _, .cons _ h => by simp [ListTy.length h]

theorem ValTyL.length {p : Program} : ∀ {vs : List Value} {ts : List Ty}, ValTyL p vs ts → vs.length = ts.length
  | [], [], _ => rfl
  | _ :: _, _ :: _, h => by simp only [ValTyL] at h; simp [ValTyL.length h.2]
  | [], _ :: _, h | _ :: _, [], h => by simp [ValTyL] at h

theorem ValTyL.mem {p : Program} : ∀ {vs : List Value} {ts : List Ty} {P : Ty → Prop},
    ValTyL p vs ts → (∀ t ∈ ts, P t) → ∀ v ∈ vs, ∃ t, P t ∧ ValTy p v t
  | [], _, _, _, _, _, hv => by simp at hv
  | v :: vs, t :: ts, P, h, hP, w, hw => by
    simp only [ValTyL] at h
    simp only [List.mem_cons] at hw
    rcases hw with rfl | hw
    · exact ⟨t, hP t (by simp), h.1⟩
    · exact ValTyL.mem h.2 (fun u hu => hP u (by simp [hu])) w hw
  | _ :: _, [], _, h, _, _, _ => by simp [ValTyL] at h

/-- `path("route", args…)` on arguments of its types. -/
theorem stdlib_path {env : Env} {p : Program} {rn : String} {route : RouteDecl}
    {vs : List Value} {ts : List Ty} (hp : env.prog = p)
    (hr : pathRoute p rn = .some route) (hv : ValTyL p vs ts)
    (hts : ∀ t ∈ ts, (t.le .string || t.le .number) = true) (hlen : vs.length = patternParams route.pattern) :
    GoodR (ValTy p · .string) (stdlib env "path" (.str rn :: vs)) := by
  simp only [stdlib, hp]
  refine (pathValue_good (route := route) hr ?_ hlen).mono fun w ⟨s, hs⟩ => by subst hs; simp [ValTy]
  intro v hv'
  obtain ⟨t, ht, hvt⟩ := ValTyL.mem hv hts v hv'
  simp only [Bool.or_eq_true] at ht
  rcases ht with ht | ht
  · exact .inl (hvt.mono ht).str_inv
  · exact .inr (hvt.mono ht).num_inv

theorem ty_sound_aux {p : Program} (hfn : ProgOK p) : ∀ n,
    (∀ {G env inFn Γ ls e t}, Ctx p G env inFn Γ ls → HasTy p G Γ e t →
      GoodR (ValTy p · t) (eval n env inFn ls e)) ∧
    (∀ {G env inFn Γ ls es ts}, Ctx p G env inFn Γ ls → ListTy p G Γ es ts →
      GoodR (ValTyL p · ts) (evalList n env inFn ls es)) ∧
    (∀ {G env inFn Γ ls es ts}, Ctx p G env inFn Γ ls → ListTy p G Γ es ts →
      (∀ t ∈ ts, t.displayable = true) → GoodR (fun _ => True) (evalDisplays n env inFn ls es)) ∧
    (∀ {G env inFn Γ ls ps body a b xs i}, Ctx p G env inFn Γ ls → HasTy p G (bindTy ps a Γ) body b →
      ValTys p xs a → GoodR (ValTys p · b) (evalMap n env inFn ls ps body xs i)) ∧
    (∀ {G env inFn Γ ls ps body a b xs i}, Ctx p G env inFn Γ ls → HasTy p G (bindTy ps a Γ) body b →
      b.le .bool = true → ValTys p xs a → GoodR (ValTys p · a) (evalFilter n env inFn ls ps body xs i)) ∧
    (∀ {G env inFn Γ ls written wts base fs i}, Ctx p G env inFn Γ ls → NamedTy p G Γ written wts →
      fieldsOk fs wts base.isSome = true → (∀ bs, base = .some bs → FieldsTy p (bs.drop i) fs) →
      GoodR (FieldsTy p · fs) (evalFields n env inFn ls written base fs i))
  | 0 => by
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ <;> intros <;>
      simp [eval, evalList, evalDisplays, evalMap, evalFilter, evalFields, GoodR, outOfFuel, Legit]
  | n + 1 => by
    obtain ⟨ihE, ihL, ihD, ihM, ihF, ihR⟩ := ty_sound_aux hfn n
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
    · intro G env inFn Γ ls e t hc h
      have hc' := hc
      obtain ⟨⟨hp, hG⟩, hin, hl⟩ := hc'
      cases h with
      | num => simp [eval, GoodR, ValTy]
      | str => simp [eval, GoodR, ValTy]
      | bool => simp [eval, GoodR, ValTy]
      | none => simp [eval, GoodR, ValTy]
      | emptyList => simp [eval, GoodR, ValTy, ValTys]
      | some h =>
        simp only [eval]
        exact GoodR.bind (ihE hc h) fun v hv => by simpa [GoodR, ValTy] using hv
      | template h hd =>
        simp only [eval]
        exact GoodR.bind (ihD hc h hd) fun _ _ => by simp [GoodR, ValTy]
      | «local» hx =>
        obtain ⟨v, hv, hvt⟩ := hl.lookup_some hx
        simp [eval, hv, GoodR, hvt]
      | global hx hg =>
        simp only [eval, hl.lookup_none hx]
        cases inFn with
        | true => rw [hin rfl] at hg; simp [lookupTy] at hg
        | false => exact hG _ _ hg
      | member h hs hi hf =>
        simp only [eval]
        refine GoodR.bind (ihE hc h) fun v hv => ?_
        obtain ⟨vs, sh, rfl, hsh, hfs⟩ := hv.record_inv
        rw [hs] at hsh; cases hsh
        obtain ⟨w, hw, hwt⟩ := FieldsTy.get hfs hf
        simp [Env.fieldIndex, Env.shape, hp, hs, hi, hw, GoodR, hwt]
      | fn hfd hargs hle =>
        simp only [eval, hp, hfd]
        refine GoodR.bind (ihL hc hargs) fun vs hvs => ?_
        obtain ⟨tb, hb, hbl⟩ := hfn.1 _ (List.mem_of_find?_eq_some hfd)
        have hloc := (Agree.params hvs hle).reverse.localsOK
        exact (ihE ⟨⟨hp, fun x t h => by simp [lookupTy] at h⟩, fun _ => rfl, hloc⟩ hb).mono
          fun v hv => hv.mono hbl
      | map hfd _ hlist hbody =>
        simp only [eval, hp, hfd]
        refine GoodR.bind (ihE hc hlist) fun v hv => ?_
        obtain ⟨xs, rfl, hxs⟩ := hv.list_inv
        exact GoodR.bind (P := fun ys => ys = xs) rfl fun _ h => by
          subst h
          exact GoodR.bind (ihM hc hbody hxs) fun ys hys => by simpa [GoodR, ValTy] using hys
      | filter hfd _ hlist hbody hb =>
        simp only [eval, hp, hfd]
        refine GoodR.bind (ihE hc hlist) fun v hv => ?_
        obtain ⟨xs, rfl, hxs⟩ := hv.list_inv
        exact GoodR.bind (P := fun ys => ys = xs) rfl fun _ h => by
          subst h
          exact GoodR.bind (ihF hc hbody hb hxs) fun ys hys => by simpa [GoodR, ValTy] using hys
      | pending hfd _ =>
        simp only [eval, hp, hfd]
        split
        · cases lookup _ env.resources <;> simp [GoodR, Legit, ValTy]
        · simp [GoodR, ValTy]
      | failed hfd _ =>
        simp only [eval, hp, hfd]
        split
        · cases lookup _ env.resources <;> simp [GoodR, Legit, ValTy]
        · simp [GoodR, ValTy]
      | roster hfd hargs hr =>
        simp only [eval, hp, hfd]
        split
        · cases hargs with | cons _ h2 => cases h2 with | cons h3 _ => cases h3
        · cases hargs with | cons _ h2 => cases h2 with | cons h3 _ => cases h3
        · cases hargs with | cons _ h2 => cases h2 with | nil =>
            delta rosterTy routerTy unsupportedTy at hr; simp at hr
        · cases hargs with | cons _ h2 => cases h2 with | nil =>
            delta rosterTy routerTy unsupportedTy at hr; simp at hr
        · exact GoodR.bind (ihL hc hargs) fun vs hvs => stdlib_good hp hfn.2 hvs hr
      | path hfd hr hargs hts hlen =>
        rename_i rn route args ts
        simp only [eval, hp, hfd]
        split
        all_goals try (simp_all; done)
        have hv0 := ihL (es := .str rn :: args) hc (.cons .str hargs)
        cases hres : evalList n env inFn ls (.str rn :: args) with
        | error e => simp only [hres] at hv0 ⊢; exact hv0
        | ok vs =>
          rw [hres] at hv0
          cases evalList_sound hres with
          | cons hE _ =>
            cases hE
            have hv1 : ValTyL p _ _ := hv0
            simp only [ValTyL] at hv1
            exact stdlib_path hp hr hv1.2 hts
              (by rw [ValTyL.length hv1.2, ← ListTy.length hargs, hlen])
      | record hs hw hok _ =>
        simp only [eval]
        refine GoodR.bind (P := fun b => b = Option.none) rfl fun b hb => ?_
        subst hb
        simp only [Env.shape, hp, hs, Option.elim]
        refine GoodR.bind (P := fun d => d = _) rfl fun d hd => ?_
        subst hd
        exact GoodR.bind (ihR (base := Option.none) (i := 0) hc hw hok (fun bs h => by cases h)) fun vs hvs => by
          simpa [GoodR, ValTy, hs] using hvs
      | recordBase hb hle hs hw hok _ =>
        simp only [eval]
        refine GoodR.bind (ihE hc hb) fun v hv => ?_
        obtain ⟨vs, sh', rfl, hsh, hfs⟩ := (hv.mono hle).record_inv
        rw [hs] at hsh; cases hsh
        refine GoodR.bind (P := fun b => b = Option.some vs) rfl fun b hb' => ?_
        subst hb'
        simp only [Env.shape, hp, hs, Option.elim]
        refine GoodR.bind (P := fun d => d = _) rfl fun d hd => ?_
        subst hd
        exact GoodR.bind (ihR (base := Option.some vs) (i := 0) hc hw hok (by intro bs' h'; cases h'; simpa using hfs))
          fun vs hvs => by simpa [GoodR, ValTy, hs] using hvs
      | neg h hle =>
        simp only [eval]
        refine GoodR.bind (ihE hc h) fun v hv => ?_
        obtain ⟨x, rfl⟩ := (hv.mono hle).num_inv
        simp [GoodR, ValTy]
      | «not» h hle =>
        simp only [eval]
        refine GoodR.bind (ihE hc h) fun v hv => ?_
        obtain ⟨x, rfl⟩ := (hv.mono hle).bool_inv
        simp [GoodR, ValTy]
      | @binary _ _ _ _ _ op _ ha hb hbin =>
        cases op
        case and | or =>
          simp only [binTy] at hbin
          split at hbin
          · next hbb =>
            simp at hbin; subst hbin
            simp only [Bool.and_eq_true] at hbb
            simp only [eval]
            refine GoodR.bind (ihE hc ha) fun v hv => ?_
            obtain ⟨x, rfl⟩ := (hv.mono hbb.1).bool_inv
            cases x
            all_goals first
              | (simp [GoodR, ValTy]; done)
              | exact (ihE hc hb).mono fun w hw => hw.mono hbb.2
          · simp at hbin
        all_goals
          rw [eval_binary_strict (by simp) (by simp)]
          exact GoodR.bind (ihE hc ha) fun va hva =>
            GoodR.bind (ihE hc hb) fun vb hvb =>
              binop_good hva hvb hbin (by simp) (by simp)
      | ternary hcnd hle ha hb hu =>
        simp only [eval]
        refine GoodR.bind (ihE hc hcnd) fun v hv => ?_
        obtain ⟨x, rfl⟩ := (hv.mono hle).bool_inv
        have hu' := Ty.unify_le hu
        cases x
        · exact (ihE hc hb).mono fun w hw => hw.mono hu'.2
        · exact (ihE hc ha).mono fun w hw => hw.mono hu'.1
      | matchOpt hs ha hb hu =>
        simp only [eval]
        refine GoodR.bind (ihE hc hs) fun v hv => ?_
        have hu' := Ty.unify_le hu
        rcases hv.option_inv with rfl | ⟨w, rfl, hw⟩
        · exact (ihE hc hb).mono fun w hw => hw.mono hu'.2
        · exact (ihE ⟨⟨hp, hG⟩, hin, hl.cons hw⟩ ha).mono fun w hw => hw.mono hu'.1
      | letE hv hb =>
        simp only [eval]
        exact GoodR.bind (ihE hc hv) fun w hw => ihE ⟨⟨hp, hG⟩, hin, hl.cons hw⟩ hb
      | typed h hu =>
        simp only [eval]
        exact (ihE hc h).mono fun w hw => hw.mono (Ty.unify_le hu).2
    · intro G env inFn Γ ls es ts hc h
      cases h with
      | nil => simp [evalList, GoodR, ValTyL]
      | cons he hes =>
        simp only [evalList]
        exact GoodR.bind (ihE hc he) fun v hv => GoodR.bind (ihL hc hes) fun vs hvs => by
          simp [GoodR, ValTyL, hv, hvs]
    · intro G env inFn Γ ls es ts hc h hd
      cases h with
      | nil => simp [evalDisplays, GoodR]
      | cons he hes =>
        simp only [evalDisplays]
        refine GoodR.bind (ihE hc he) fun v hv => ?_
        obtain ⟨s, hs⟩ := display_ok hv (hd _ (by simp))
        rw [hs]
        exact GoodR.bind (P := fun x => x = s) rfl fun _ h => by
          subst h; exact GoodR.bind (ihD hc hes fun t ht => hd t (by simp [ht])) fun _ _ => trivial
    · intro G env inFn Γ ls ps body a b xs i hc hb hxs
      cases xs with
      | nil => simp [evalMap, GoodR, ValTys]
      | cons x xs =>
        simp only [ValTys] at hxs
        simp only [evalMap]
        obtain ⟨he, hin, hl⟩ := hc
        exact GoodR.bind (ihE ⟨he, hin, hl.bind hxs.1 ps i⟩ hb) fun y hy =>
          GoodR.bind (ihM ⟨he, hin, hl⟩ hb hxs.2) fun ys hys => by simp [GoodR, ValTys, hy, hys]
    · intro G env inFn Γ ls ps body a b xs i hc hb hbl hxs
      cases xs with
      | nil => simp [evalFilter, GoodR, ValTys]
      | cons x xs =>
        simp only [ValTys] at hxs
        simp only [evalFilter]
        obtain ⟨he, hin, hl⟩ := hc
        refine GoodR.bind (ihE ⟨he, hin, hl.bind hxs.1 ps i⟩ hb) fun k hk => ?_
        refine GoodR.bind (ihF ⟨he, hin, hl⟩ hb hbl hxs.2) fun ys hys => ?_
        obtain ⟨kb, rfl⟩ := (hk.mono hbl).bool_inv
        cases kb <;> simp [GoodR, ValTys, hys, hxs.1]
    · intro G env inFn Γ ls written wts base fs i hc hw hok hbase
      cases fs with
      | nil => simp [evalFields, GoodR, FieldsTy]
      | cons f fs =>
        simp only [fieldsOk, List.all_cons, Bool.and_eq_true] at hok
        obtain ⟨hf, hok⟩ := hok
        simp only [evalFields]
        have hb1 : ∀ bs, base = .some bs →
            ∃ v, bs[i]? = .some v ∧ ValTy p v f.ty ∧ FieldsTy p (bs.drop (i + 1)) fs :=
          fun bs h => FieldsTy.drop_cons (hbase bs h)
        have hb2 : ∀ bs, base = .some bs → FieldsTy p (bs.drop (i + 1)) fs :=
          fun bs h => by obtain ⟨_, _, _, h3⟩ := hb1 bs h; exact h3
        have hrest : ∀ v, ValTy p v f.ty →
            GoodR (FieldsTy p · (f :: fs))
              (do let vs ← evalFields n env inFn ls written base fs (i + 1); .ok (v :: vs)) :=
          fun v hv => GoodR.bind (ihR hc hw (by simpa [fieldsOk] using hok) hb2) fun vs hvs => by
            simp [GoodR, FieldsTy, hv, hvs]
        rcases hw.lookup f.name with ⟨hnone, htnone⟩ | ⟨e, te, he, hte, het⟩
        · rw [htnone] at hf
          simp only [hnone]
          cases hbase' : base with
          | none => rw [hbase'] at hf; simp at hf
          | some bs =>
            obtain ⟨v, hv, hvt, _⟩ := hb1 bs hbase'
            rw [hbase'] at hrest
            simp only [hv, Option.elim]
            exact GoodR.bind (P := fun w => w = v) rfl fun w hw' => by rw [hw']; exact hrest v hvt
        · rw [hte] at hf
          simp only [he]
          exact GoodR.bind (ihE hc het) fun v hv => hrest v (hv.mono hf)

/-- **Expression soundness** (preservation and progress). A well-typed
expression evaluated where its names and locals hold values of their types
has a value of its type, or fails legitimately: never a type error, never
an unbound name. -/
theorem eval_sound_ty {p : Program} (hfn : ProgOK p) {n G env Γ ls e t}
    (henv : EnvOK p G env) (hl : LocalsOK p Γ ls) (h : HasTy p G Γ e t) :
    GoodR (ValTy p · t) (eval n env false ls e) :=
  (ty_sound_aux hfn n).1 ⟨henv, by simp, hl⟩ h

theorem evalList_sound_ty {p : Program} (hfn : ProgOK p) {n G env Γ ls es ts}
    (henv : EnvOK p G env) (hl : LocalsOK p Γ ls) (h : ListTy p G Γ es ts) :
    GoodR (ValTyL p · ts) (evalList n env false ls es) :=
  (ty_sound_aux hfn n).2.1 ⟨henv, by simp, hl⟩ h

/-! ## Statements -/

/-- What an action body has asked for is well typed: every write holds a
value of its slot's type, and every send targets a mutation. -/
def FxOK (p : Program) (fx : Effects) : Prop :=
  (∀ w ∈ fx.writes ++ fx.rowWrites, ValTy p w.2 (slotTy p w.1)) ∧ (∀ s ∈ fx.sends, isMutation p s.1 = true)

theorem FxOK.empty {p : Program} : FxOK p {} := ⟨by simp, by simp⟩

theorem isSlot_cases {p : Program} {x : String} (h : isSlot p x = true) :
    isRootState p x = true ∨ isRowState p x = true := by
  simp only [isSlot, isMutation, Bool.or_eq_true, List.any_eq_true, beq_iff_eq] at h
  simp only [isRootState, isRowState, Bool.or_eq_true, List.any_eq_true, Bool.and_eq_true, beq_iff_eq]
  rcases h with ⟨s, hs, rfl⟩ | ⟨m, hm, rfl⟩
  · cases ho : s.owner with
    | none => exact .inl (.inl ⟨s, hs, rfl, by simp [ho]⟩)
    | some o => exact .inr ⟨s, hs, rfl, by simp [ho]⟩
  · exact .inl (.inr ⟨m, hm, rfl⟩)

theorem exec_sound_aux {p : Program} (hfn : ProgOK p) {G : Scope} : ∀ n {env Γ ls ss fx},
    EnvOK p G env → LocalsOK p Γ ls → StmtsTy p G Γ ss → FxOK p fx →
    GoodR (FxOK p) (exec n env ls ss fx)
  | 0, _, _, _, _, _, _, _, _, _ => by simp [exec, GoodR, outOfFuel, Legit]
  | n + 1, env, Γ, ls, ss, fx, henv, hl, hs, hfx => by
    have hp := henv.1
    cases ss with
    | nil => simpa [exec, GoodR] using hfx
    | cons s rest =>
      cases s with
      | letS x e =>
        simp only [StmtsTy] at hs
        obtain ⟨t, he, hr⟩ := hs
        simp only [exec]
        exact GoodR.bind (eval_sound_ty hfn henv hl he) fun v hv =>
          exec_sound_aux hfn n henv (hl.cons hv) hr hfx
      | assign x e =>
        simp only [StmtsTy] at hs
        obtain ⟨hslot, ⟨t, he, hle⟩, hr⟩ := hs
        simp only [exec]
        refine GoodR.bind (eval_sound_ty hfn henv hl he) fun v hv => ?_
        have hvx := hv.mono hle
        rw [hp]
        split
        · next hroot =>
          refine GoodR.bind (P := fun fx' => FxOK p fx') ?_ fun fx' h' => exec_sound_aux hfn n henv hl hr h'
          refine ⟨fun w hw => ?_, hfx.2⟩
          simp only [List.mem_append, List.mem_singleton] at hw
          rcases hw with (hw | rfl) | hw
          · exact hfx.1 w (List.mem_append_left _ hw)
          · exact hvx
          · exact hfx.1 w (List.mem_append_right _ hw)
        · next hroot =>
          split
          · split
            · refine GoodR.bind (P := fun fx' => FxOK p fx') ?_ fun fx' h' => exec_sound_aux hfn n henv hl hr h'
              refine ⟨fun w hw => ?_, hfx.2⟩
              simp only [List.mem_append, List.mem_singleton] at hw
              rcases hw with hw | hw | rfl
              · exact hfx.1 w (List.mem_append_left _ hw)
              · exact hfx.1 w (List.mem_append_right _ hw)
              · exact hvx
            · exact GoodR.bind (P := fun _ => False) trivial (fun _ h => h.elim)
          · next hrow =>
            rcases isSlot_cases hslot with h | h
            · simp_all
            · simp_all
      | command name args =>
        simp only [StmtsTy] at hs
        obtain ⟨⟨ts, hargs⟩, hr⟩ := hs
        simp only [exec]
        exact GoodR.bind (evalList_sound_ty hfn henv hl hargs) fun vs _ =>
          exec_sound_aux hfn n henv hl hr ⟨hfx.1, hfx.2⟩
      | send x src args =>
        simp only [StmtsTy] at hs
        obtain ⟨hm, ⟨ts, hargs⟩, hr⟩ := hs
        simp only [exec]
        refine GoodR.bind (evalList_sound_ty hfn henv hl hargs) fun vs _ =>
          exec_sound_aux hfn n henv hl hr ⟨hfx.1, fun s hs' => ?_⟩
        simp only [List.mem_append, List.mem_singleton] at hs'
        rcases hs' with hs' | rfl
        · exact hfx.2 s hs'
        · exact hm
      | refresh x =>
        simp only [StmtsTy] at hs
        obtain ⟨_, hr⟩ := hs
        simp only [exec]
        exact exec_sound_aux hfn n henv hl hr ⟨hfx.1, hfx.2⟩
      | ifS c thn els =>
        simp only [StmtsTy] at hs
        obtain ⟨⟨t, hc, hle⟩, hthn, hels, hr⟩ := hs
        simp only [exec]
        refine GoodR.bind (eval_sound_ty hfn henv hl hc) fun v hv => ?_
        obtain ⟨b, rfl⟩ := (hv.mono hle).bool_inv
        cases b
        · exact GoodR.bind (exec_sound_aux hfn n henv hl hels hfx) fun fx' h' => exec_sound_aux hfn n henv hl hr h'
        · exact GoodR.bind (exec_sound_aux hfn n henv hl hthn hfx) fun fx' h' => exec_sound_aux hfn n henv hl hr h'
      | matchS subj x sm nn =>
        simp only [StmtsTy] at hs
        obtain ⟨⟨a, hsub, hsm⟩, hnn, hr⟩ := hs
        simp only [exec]
        refine GoodR.bind (eval_sound_ty hfn henv hl hsub) fun v hv => ?_
        rcases hv.option_inv with rfl | ⟨w, rfl, hw⟩
        · exact GoodR.bind (exec_sound_aux hfn n henv hl hnn hfx) fun fx' h' => exec_sound_aux hfn n henv hl hr h'
        · exact GoodR.bind (exec_sound_aux hfn n henv (hl.cons hw) hsm hfx) fun fx' h' =>
            exec_sound_aux hfn n henv hl hr h'

/-- **Statement soundness.** A well-typed block run where its names and
locals hold values of their types asks only for writes of values of the
target slots' types (root and row writes alike) and sends to mutations, or
fails legitimately: never a type error, never an unbound name. -/
theorem exec_sound_ty {p : Program} (hfn : ProgOK p) {n G env Γ ls ss}
    (henv : EnvOK p G env) (hl : LocalsOK p Γ ls) (h : StmtsTy p G Γ ss) :
    GoodR (FxOK p) (exec n env ls ss {}) :=
  exec_sound_aux hfn n henv hl h FxOK.empty

end Contract
