/-
The semantics of expressions and statements as inductive big-step
relations, and the proof that the interpreters of `Contract.Eval` and
`Contract.Runtime` compute exactly them.

The relations are success-only: `EvalR env inFn ls e v` says `e` has the
value `v`; an expression that fails (a type error, an unbound name, a
pending read) is one with no derivation. Fuel is the interpreter's, not the
language's: the relations have none, and the theorems at the end say the
interpreter at enough fuel answers a value exactly when a derivation does,
and that more fuel never changes an answer.

Each rule names the interpreter's case it mirrors; the side conditions are
the ones the interpreter tests, in the order it tests them.
-/
import Contract.Runtime

namespace Contract

/-! ## Facts about `Except` -/

@[simp] theorem Except.bind_ok_iff {ε α β} {x : Except ε α} {f : α → Except ε β} {v : β} :
    (x >>= f) = .ok v ↔ ∃ a, x = .ok a ∧ f a = .ok v := by
  cases x <;> simp [bind, Except.bind]

@[simp] theorem Except.pure_ok_iff {ε α} {a v : α} : (pure a : Except ε α) = .ok v ↔ a = v := by
  simp [pure, Except.pure]

@[simp] theorem Except.ok_ok_iff {ε α} {a v : α} : (Except.ok a : Except ε α) = .ok v ↔ a = v := by
  constructor
  · intro h; cases h; rfl
  · intro h; rw [h]

@[simp] theorem Except.error_ok_iff {ε α} {e : ε} {v : α} : (Except.error e : Except ε α) = .ok v ↔ False := by
  simp

theorem Option.elim_err_ok {ε α} {o : Option α} {e : ε} {v : α} :
    o.elim (Except.error e) Except.ok = Except.ok v ↔ o = .some v := by
  cases o <;> simp

theorem Option.elim_err_const {ε α β} {o : Option α} {e : ε} {w v : β} :
    o.elim (Except.error e) (fun _ => Except.ok w) = Except.ok v ↔ (o.isSome ∧ w = v) := by
  cases o <;> simp

theorem Value.asList_ok {v : Value} {xs} : v.asList = .ok xs ↔ v = .list xs := by
  cases v <;> simp [Value.asList]

/-! ## Expressions -/

mutual

/-- `e` evaluates to `v`, `inFn` hiding the component's names. -/
inductive EvalR (env : Env) : Bool → Locals → Expr → Value → Prop
  | num : EvalR env inFn ls (.num b) (.num (Float.ofBits b))
  | str : EvalR env inFn ls (.str s) (.str s)
  | bool : EvalR env inFn ls (.bool b) (.bool b)
  | none : EvalR env inFn ls .none .none
  | emptyList : EvalR env inFn ls .emptyList (.list [])
  | some : EvalR env inFn ls e v → EvalR env inFn ls (.some e) (.some v)
  /-- Each part displayed, then concatenated. -/
  | template : DisplaysR env inFn ls parts ss → EvalR env inFn ls (.template parts) (.str (String.join ss))
  /-- A local shadows everything. -/
  | local : lookup x ls = .some v → EvalR env inFn ls (.var x) v
  /-- Outside a `fn`, an unbound local is a component-level name. -/
  | global : lookup x ls = .none → inFn = false → env.global x = .ok v → EvalR env inFn ls (.var x) v
  /-- A field by its index in the shape the record was built as. -/
  | member : EvalR env inFn ls e (.record s fs) → env.fieldIndex s f = .some i → fs[i]? = .some v →
      EvalR env inFn ls (.member e f) v
  /-- A `fn` (which shadows the roster): the arguments, then the body in a
  scope of the parameters alone. -/
  | fn : env.prog.fns.find? (·.name == name) = .some fd → ListR env inFn ls args vs →
      EvalR env true ((fd.params.map (·.1)).zip vs).reverse fd.body v →
      EvalR env inFn ls (.call name args) v
  | map : env.prog.fns.find? (·.name == "map") = .none → EvalR env inFn ls l (.list xs) →
      MapR env inFn ls ps body xs 0 ys → EvalR env inFn ls (.call "map" [l, .arrow ps body]) (.list ys)
  | filter : env.prog.fns.find? (·.name == "filter") = .none → EvalR env inFn ls l (.list xs) →
      FilterR env inFn ls ps body xs 0 ys → EvalR env inFn ls (.call "filter" [l, .arrow ps body]) (.list ys)
  /-- `pending(r)` of a settled resource, or of a name that is not one. -/
  | pendingSettled : env.prog.fns.find? (·.name == "pending") = .none →
      env.prog.resources.any (·.name == x) = true → (lookup x env.resources).isSome →
      EvalR env inFn ls (.call "pending" [.var x]) (.bool false)
  | pendingOther : env.prog.fns.find? (·.name == "pending") = .none →
      env.prog.resources.any (·.name == x) = false →
      EvalR env inFn ls (.call "pending" [.var x]) (.bool false)
  /-- `failed(r)`, likewise: a source here never fails. -/
  | failedSettled : env.prog.fns.find? (·.name == "failed") = .none →
      env.prog.resources.any (·.name == x) = true → (lookup x env.resources).isSome →
      EvalR env inFn ls (.call "failed" [.var x]) (.bool false)
  | failedOther : env.prog.fns.find? (·.name == "failed") = .none →
      env.prog.resources.any (·.name == x) = false →
      EvalR env inFn ls (.call "failed" [.var x]) (.bool false)
  /-- Any other roster entry, on its evaluated arguments. (`stdlib` refuses
  the names of the forms above, so this never overlaps them.) -/
  | stdlib : env.prog.fns.find? (·.name == name) = .none → ListR env inFn ls args vs →
      stdlib env name vs = .ok v → EvalR env inFn ls (.call name args) v
  | record : env.shape shape = .some decl → FieldsR env inFn ls fields .none decl.fields 0 vs →
      EvalR env inFn ls (.record shape .none fields) (.record shape vs)
  /-- With a base: its fields fill those not written. -/
  | recordBase : EvalR env inFn ls e (.record s' bs) → env.shape shape = .some decl →
      FieldsR env inFn ls fields (.some bs) decl.fields 0 vs →
      EvalR env inFn ls (.record shape (.some e) fields) (.record shape vs)
  | neg : EvalR env inFn ls e (.num x) → EvalR env inFn ls (.unary .neg e) (.num (-x))
  | not : EvalR env inFn ls e (.bool b) → EvalR env inFn ls (.unary .not e) (.bool !b)
  /-- `and`/`or` short-circuit: the right operand is evaluated only when
  the left does not decide. -/
  | andFalse : EvalR env inFn ls a (.bool false) → EvalR env inFn ls (.binary .and a b) (.bool false)
  | andTrue : EvalR env inFn ls a (.bool true) → EvalR env inFn ls b v → EvalR env inFn ls (.binary .and a b) v
  | orTrue : EvalR env inFn ls a (.bool true) → EvalR env inFn ls (.binary .or a b) (.bool true)
  | orFalse : EvalR env inFn ls a (.bool false) → EvalR env inFn ls b v → EvalR env inFn ls (.binary .or a b) v
  /-- Any other operator, strictly. (`binop` refuses `and`/`or`.) -/
  | binop : EvalR env inFn ls a va → EvalR env inFn ls b vb → binop op va vb = .ok v →
      EvalR env inFn ls (.binary op a b) v
  | ternaryTrue : EvalR env inFn ls c (.bool true) → EvalR env inFn ls a v → EvalR env inFn ls (.ternary c a b) v
  | ternaryFalse : EvalR env inFn ls c (.bool false) → EvalR env inFn ls b v → EvalR env inFn ls (.ternary c a b) v
  | matchSome : EvalR env inFn ls s (.some w) → EvalR env inFn ((x, w) :: ls) a v →
      EvalR env inFn ls (.matchOpt s x a b) v
  | matchNone : EvalR env inFn ls s .none → EvalR env inFn ls b v → EvalR env inFn ls (.matchOpt s x a b) v
  | letE : EvalR env inFn ls e w → EvalR env inFn ((x, w) :: ls) body v → EvalR env inFn ls (.letE x e body) v
  /-- A type ascription is its expression's value. -/
  | typed : EvalR env inFn ls e v → EvalR env inFn ls (.typed e ty) v

/-- A list of expressions, left to right. -/
inductive ListR (env : Env) : Bool → Locals → List Expr → List Value → Prop
  | nil : ListR env inFn ls [] []
  | cons : EvalR env inFn ls e v → ListR env inFn ls es vs → ListR env inFn ls (e :: es) (v :: vs)

/-- A template's parts, each displayed. -/
inductive DisplaysR (env : Env) : Bool → Locals → List Expr → List String → Prop
  | nil : DisplaysR env inFn ls [] []
  | cons : EvalR env inFn ls e v → v.display = .ok s → DisplaysR env inFn ls es ss →
      DisplaysR env inFn ls (e :: es) (s :: ss)

/-- `map`'s callback on each item from index `i`. -/
inductive MapR (env : Env) : Bool → Locals → List String → Expr → List Value → Nat → List Value → Prop
  | nil : MapR env inFn ls ps body [] i []
  | cons : EvalR env inFn (bindParams ps x i ls) body y → MapR env inFn ls ps body xs (i + 1) ys →
      MapR env inFn ls ps body (x :: xs) i (y :: ys)

/-- `filter`'s callback on each item from index `i`: kept on `true`. -/
inductive FilterR (env : Env) : Bool → Locals → List String → Expr → List Value → Nat → List Value → Prop
  | nil : FilterR env inFn ls ps body [] i []
  | keep : EvalR env inFn (bindParams ps x i ls) body (.bool true) → FilterR env inFn ls ps body xs (i + 1) ys →
      FilterR env inFn ls ps body (x :: xs) i (x :: ys)
  | drop : EvalR env inFn (bindParams ps x i ls) body (.bool false) → FilterR env inFn ls ps body xs (i + 1) ys →
      FilterR env inFn ls ps body (x :: xs) i ys

/-- A record's fields from the `i`th declared: the one written, else the
base's at the same index. -/
inductive FieldsR (env : Env) : Bool → Locals → List (String × Expr) → Option (List Value) →
    List Field → Nat → List Value → Prop
  | nil : FieldsR env inFn ls written base [] i []
  | written : lookupField f.name written = .some e → EvalR env inFn ls e v →
      FieldsR env inFn ls written base fs (i + 1) vs → FieldsR env inFn ls written base (f :: fs) i (v :: vs)
  | base : lookupField f.name written = .none → bs[i]? = .some v →
      FieldsR env inFn ls written (.some bs) fs (i + 1) vs →
      FieldsR env inFn ls written (.some bs) (f :: fs) i (v :: vs)

end

/-! ## Statements -/

def Effects.write (fx : Effects) (t : String) (v : Value) : Effects := { fx with writes := fx.writes ++ [(t, v)] }
def Effects.rowWrite (fx : Effects) (t : String) (v : Value) : Effects :=
  { fx with rowWrites := fx.rowWrites ++ [(t, v)] }
def Effects.command (fx : Effects) (n : String) (vs : List Value) : Effects :=
  { fx with commands := fx.commands ++ [(n, vs)] }
def Effects.send (fx : Effects) (t src : String) (vs : List Value) : Effects :=
  { fx with sends := fx.sends ++ [(t, src, vs)] }
def Effects.refresh (fx : Effects) (t : String) : Effects := { fx with refreshes := fx.refreshes ++ [t] }

/-- A block runs from accumulated effects `fx` to `fx'`. Every expression
is read against the one `env`: a write is only recorded, so no statement
sees an earlier one's write. -/
inductive ExecR (env : Env) : Locals → List Stmt → Effects → Effects → Prop
  | nil : ExecR env ls [] fx fx
  /-- A `let` scopes over the rest of its block. -/
  | letS : EvalR env false ls e v → ExecR env ((x, v) :: ls) rest fx fx' →
      ExecR env ls (.letS x e :: rest) fx fx'
  /-- A write to a root slot (a state or a mutation) is appended. -/
  | assignRoot : EvalR env false ls e v → isRootState env.prog t = true →
      ExecR env ls rest (fx.write t v) fx' → ExecR env ls (.assign t e :: rest) fx fx'
  /-- A write to a row slot, only inside a row that has it. -/
  | assignRow : EvalR env false ls e v → isRootState env.prog t = false → isRowState env.prog t = true →
      (lookup t env.rows).isSome → ExecR env ls rest (fx.rowWrite t v) fx' →
      ExecR env ls (.assign t e :: rest) fx fx'
  | command : ListR env false ls args vs → ExecR env ls rest (fx.command n vs) fx' →
      ExecR env ls (.command n args :: rest) fx fx'
  | send : ListR env false ls args vs → ExecR env ls rest (fx.send t src vs) fx' →
      ExecR env ls (.send t src args :: rest) fx fx'
  | refresh : ExecR env ls rest (fx.refresh t) fx' → ExecR env ls (.refresh t :: rest) fx fx'
  /-- A branch is a block of its own: its `let`s end with it. -/
  | ifTrue : EvalR env false ls c (.bool true) → ExecR env ls thn fx fx₁ → ExecR env ls rest fx₁ fx' →
      ExecR env ls (.ifS c thn els :: rest) fx fx'
  | ifFalse : EvalR env false ls c (.bool false) → ExecR env ls els fx fx₁ → ExecR env ls rest fx₁ fx' →
      ExecR env ls (.ifS c thn els :: rest) fx fx'
  | matchSome : EvalR env false ls s (.some w) → ExecR env ((x, w) :: ls) sm fx fx₁ →
      ExecR env ls rest fx₁ fx' → ExecR env ls (.matchS s x sm nn :: rest) fx fx'
  | matchNone : EvalR env false ls s .none → ExecR env ls nn fx fx₁ → ExecR env ls rest fx₁ fx' →
      ExecR env ls (.matchS s x sm nn :: rest) fx fx'

/-! ## The interpreter is sound -/

theorem stdlib_map {env : Env} {vs v} : stdlib env "map" vs ≠ .ok v := by
  unfold stdlib; split <;> simp_all
theorem stdlib_filter {env : Env} {vs v} : stdlib env "filter" vs ≠ .ok v := by
  unfold stdlib; split <;> simp_all
theorem stdlib_pending {env : Env} {vs v} : stdlib env "pending" vs ≠ .ok v := by
  unfold stdlib; split <;> simp_all
theorem stdlib_failed {env : Env} {vs v} : stdlib env "failed" vs ≠ .ok v := by
  unfold stdlib; split <;> simp_all

theorem binop_and {a b v} : binop .and a b ≠ .ok v := by simp [binop]
theorem binop_or {a b v} : binop .or a b ≠ .ok v := by simp [binop]

/-- The strict operators, as one equation. -/
theorem eval_binary_strict {n env inFn ls a b} {op : BinOp} (hand : op ≠ .and) (hor : op ≠ .or) :
    eval (n + 1) env inFn ls (.binary op a b) =
      (do let va ← eval n env inFn ls a; let vb ← eval n env inFn ls b; binop op va vb) := by
  cases op <;> first | rfl | (exact absurd rfl hand) | (exact absurd rfl hor)

theorem sound_aux : ∀ n,
    (∀ {env inFn ls e v}, eval n env inFn ls e = .ok v → EvalR env inFn ls e v) ∧
    (∀ {env inFn ls es vs}, evalList n env inFn ls es = .ok vs → ListR env inFn ls es vs) ∧
    (∀ {env inFn ls es ss}, evalDisplays n env inFn ls es = .ok ss → DisplaysR env inFn ls es ss) ∧
    (∀ {env inFn ls ps body xs i ys}, evalMap n env inFn ls ps body xs i = .ok ys →
      MapR env inFn ls ps body xs i ys) ∧
    (∀ {env inFn ls ps body xs i ys}, evalFilter n env inFn ls ps body xs i = .ok ys →
      FilterR env inFn ls ps body xs i ys) ∧
    (∀ {env inFn ls w b fs i vs}, evalFields n env inFn ls w b fs i = .ok vs →
      FieldsR env inFn ls w b fs i vs)
  | 0 => by
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ <;> intros <;>
      simp_all [eval, evalList, evalDisplays, evalMap, evalFilter, evalFields]
  | n + 1 => by
    obtain ⟨ihE, ihL, ihD, ihM, ihF, ihR⟩ := sound_aux n
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
    · intro env inFn ls e v h
      cases e with
      | num => simp [eval] at h; subst h; exact .num
      | str => simp [eval] at h; subst h; exact .str
      | bool => simp [eval] at h; subst h; exact .bool
      | none => simp [eval] at h; subst h; exact .none
      | emptyList => simp [eval] at h; subst h; exact .emptyList
      | some e =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h; simp at h2; subst h2; exact .some (ihE h1)
      | template parts =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h; simp at h2; subst h2; exact .template (ihD h1)
      | var x =>
        simp only [eval] at h
        split at h
        next w hl => simp at h; subst h; exact .local hl
        next hl =>
          split at h
          next => simp at h
          next hf => exact .global hl (by simpa using hf) h
      | member e f =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨a, h1, h2⟩ := h
        split at h2
        next s fs =>
          split at h2
          next i hi => exact .member (ihE h1) hi (Option.elim_err_ok.mp h2)
          next => simp at h2
        next => simp at h2
      | call name args =>
        simp only [eval] at h
        split at h
        next fd hfd =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨vs, h1, h2⟩ := h
          exact .fn hfd (ihL h1) (ihE h2)
        next hfd =>
          split at h
          next l ps body =>
            simp only [Except.bind_ok_iff, Value.asList_ok] at h
            obtain ⟨a, h1, xs, rfl, ys, h3, h4⟩ := h
            simp at h4; subst h4
            exact .map hfd (ihE h1) (ihM h3)
          next l ps body =>
            simp only [Except.bind_ok_iff, Value.asList_ok] at h
            obtain ⟨a, h1, xs, rfl, ys, h3, h4⟩ := h
            simp at h4; subst h4
            exact EvalR.filter hfd (ihE h1) (ihF h3)
          next x =>
            split at h
            next hr =>
              obtain ⟨hs, rfl⟩ := Option.elim_err_const.mp h
              exact .pendingSettled hfd hr hs
            next hr => simp at h; subst h; exact .pendingOther hfd (by simpa using hr)
          next x =>
            split at h
            next hr =>
              obtain ⟨hs, rfl⟩ := Option.elim_err_const.mp h
              exact .failedSettled hfd hr hs
            next hr => simp at h; subst h; exact .failedOther hfd (by simpa using hr)
          next =>
            simp only [Except.bind_ok_iff] at h
            obtain ⟨vs, h1, h2⟩ := h
            exact .stdlib hfd (ihL h1) h2
      | record shape base fields =>
        simp only [eval] at h
        split at h
        next =>
          simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h
          obtain ⟨_, rfl, decl, hd, vs, h1, h2⟩ := h
          simp at h2; subst h2
          exact .record (Option.elim_err_ok.mp hd) (ihR h1)
        next e =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨a, h1, h2⟩ := h
          split at h2
          next s' bs =>
            simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2
            obtain ⟨_, rfl, decl, hd, vs, h3, h4⟩ := h2
            simp at h4; subst h4
            exact .recordBase (ihE h1) (Option.elim_err_ok.mp hd) (ihR h3)
          next => simp at h2
      | unary op e =>
        cases op <;> simp only [eval, Except.bind_ok_iff] at h <;> obtain ⟨a, h1, h2⟩ := h <;>
          split at h2 <;> simp at h2 <;> subst h2
        · exact .neg (ihE h1)
        · exact .not (ihE h1)
      | binary op a b =>
        cases op
        case and =>
          simp only [eval, Except.bind_ok_iff] at h
          obtain ⟨w, h1, h2⟩ := h
          split at h2
          next => simp at h2; subst h2; exact .andFalse (ihE h1)
          next => exact .andTrue (ihE h1) (ihE h2)
          next => simp at h2
        case or =>
          simp only [eval, Except.bind_ok_iff] at h
          obtain ⟨w, h1, h2⟩ := h
          split at h2
          next => simp at h2; subst h2; exact .orTrue (ihE h1)
          next => exact .orFalse (ihE h1) (ihE h2)
          next => simp at h2
        all_goals
          rw [eval_binary_strict (by simp) (by simp)] at h
          simp only [Except.bind_ok_iff] at h
          obtain ⟨va, h1, vb, h2, h3⟩ := h
          exact .binop (ihE h1) (ihE h2) h3
      | ternary c a b =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next => exact .ternaryTrue (ihE h1) (ihE h2)
        next => exact .ternaryFalse (ihE h1) (ihE h2)
        next => simp at h2
      | matchOpt s x a b =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next => exact .matchSome (ihE h1) (ihE h2)
        next => exact .matchNone (ihE h1) (ihE h2)
        next => simp at h2
      | arrow => simp [eval] at h
      | letE x e body =>
        simp only [eval, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        exact .letE (ihE h1) (ihE h2)
      | named => simp [eval] at h
      | typed e ty =>
        simp only [eval] at h
        exact .typed (ihE h)
    · intro env inFn ls es vs h
      cases es with
      | nil => simp [evalList] at h; subst h; exact .nil
      | cons e es =>
        simp only [evalList, Except.bind_ok_iff] at h
        obtain ⟨v, h1, vs', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1) (ihL h2)
    · intro env inFn ls es ss h
      cases es with
      | nil => simp [evalDisplays] at h; subst h; exact .nil
      | cons e es =>
        simp only [evalDisplays, Except.bind_ok_iff] at h
        obtain ⟨v, h1, s, hs, ss', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1) hs (ihD h2)
    · intro env inFn ls ps body xs i ys h
      cases xs with
      | nil => simp [evalMap] at h; subst h; exact .nil
      | cons x xs =>
        simp only [evalMap, Except.bind_ok_iff] at h
        obtain ⟨y, h1, ys', h2, h3⟩ := h
        simp at h3; subst h3; exact .cons (ihE h1) (ihM h2)
    · intro env inFn ls ps body xs i ys h
      cases xs with
      | nil => simp [evalFilter] at h; subst h; exact .nil
      | cons x xs =>
        simp only [evalFilter, Except.bind_ok_iff] at h
        obtain ⟨k, h1, ys', h2, h3⟩ := h
        split at h3
        next => simp at h3; subst h3; exact .keep (ihE h1) (ihF h2)
        next => simp at h3; subst h3; exact .drop (ihE h1) (ihF h2)
        next => simp at h3
    · intro env inFn ls w b fs i vs h
      cases fs with
      | nil => simp [evalFields] at h; subst h; exact .nil
      | cons f fs =>
        simp only [evalFields] at h
        split at h
        next e he =>
          simp only [Except.bind_ok_iff] at h
          obtain ⟨v, h1, vs', h2, h3⟩ := h
          simp at h3; subst h3
          exact .written he (ihE h1) (ihR h2)
        next he =>
          split at h
          next bs =>
            simp only [Except.bind_ok_iff] at h
            obtain ⟨v, h1, vs', h2, h3⟩ := h
            simp at h3; subst h3
            exact .base he (Option.elim_err_ok.mp h1) (ihR h2)
          next => simp at h

/-! ## More fuel never changes an answer -/

theorem mono_aux : ∀ n,
    (∀ {m env inFn ls e v}, n ≤ m → eval n env inFn ls e = .ok v → eval m env inFn ls e = .ok v) ∧
    (∀ {m env inFn ls es vs}, n ≤ m → evalList n env inFn ls es = .ok vs →
      evalList m env inFn ls es = .ok vs) ∧
    (∀ {m env inFn ls es ss}, n ≤ m → evalDisplays n env inFn ls es = .ok ss →
      evalDisplays m env inFn ls es = .ok ss) ∧
    (∀ {m env inFn ls ps body xs i ys}, n ≤ m → evalMap n env inFn ls ps body xs i = .ok ys →
      evalMap m env inFn ls ps body xs i = .ok ys) ∧
    (∀ {m env inFn ls ps body xs i ys}, n ≤ m → evalFilter n env inFn ls ps body xs i = .ok ys →
      evalFilter m env inFn ls ps body xs i = .ok ys) ∧
    (∀ {m env inFn ls w b fs i vs}, n ≤ m → evalFields n env inFn ls w b fs i = .ok vs →
      evalFields m env inFn ls w b fs i = .ok vs)
  | 0 => by
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ <;> intros <;>
      simp_all [eval, evalList, evalDisplays, evalMap, evalFilter, evalFields]
  | n + 1 => by
    obtain ⟨ihE, ihL, ihD, ihM, ihF, ihR⟩ := mono_aux n
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
    · intro m env inFn ls e v hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases e with
      | num | str | bool | none | emptyList | var | arrow | named => exact h
      | some e | template e =>
        simp only [eval, Except.bind_ok_iff] at h ⊢
        obtain ⟨a, h1, h2⟩ := h
        first | exact ⟨a, ihE hm h1, h2⟩ | exact ⟨a, ihD hm h1, h2⟩
      | member e f =>
        simp only [eval, Except.bind_ok_iff] at h ⊢
        obtain ⟨a, h1, h2⟩ := h
        exact ⟨a, ihE hm h1, h2⟩
      | call name args =>
        simp only [eval] at h ⊢
        split at h
        next fd hfd =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨vs, h1, h2⟩ := h
          exact ⟨vs, ihL hm h1, ihE hm h2⟩
        next hfd =>
          split at h
          next l ps body =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨a, h1, xs, h2, ys, h3, h4⟩ := h
            exact ⟨a, ihE hm h1, xs, h2, ys, ihM hm h3, h4⟩
          next l ps body =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨a, h1, xs, h2, ys, h3, h4⟩ := h
            exact ⟨a, ihE hm h1, xs, h2, ys, ihF hm h3, h4⟩
          next => exact h
          next => exact h
          next =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨vs, h1, h2⟩ := h
            exact ⟨vs, ihL hm h1, h2⟩
      | record shape base fields =>
        simp only [eval] at h ⊢
        split at h
        next =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨b, h0, decl, hd, vs, h1, h2⟩ := h
          exact ⟨b, h0, decl, hd, vs, ihR hm h1, h2⟩
        next e =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨a, h1, h2⟩ := h
          refine ⟨a, ihE hm h1, ?_⟩
          split at h2
          next =>
            simp only [Except.bind_ok_iff] at h2 ⊢
            obtain ⟨b, h0, decl, hd, vs, h3, h4⟩ := h2
            exact ⟨b, h0, decl, hd, vs, ihR hm h3, h4⟩
          next => simp at h2
      | unary op e =>
        cases op <;> simp only [eval, Except.bind_ok_iff] at h ⊢ <;> obtain ⟨a, h1, h2⟩ := h <;>
          exact ⟨a, ihE hm h1, h2⟩
      | binary op a b =>
        cases op
        case and | or =>
          simp only [eval, Except.bind_ok_iff] at h ⊢
          obtain ⟨w, h1, h2⟩ := h
          refine ⟨w, ihE hm h1, ?_⟩
          split at h2
          next => exact h2
          next => exact ihE hm h2
          next => simp at h2
        all_goals
          rw [eval_binary_strict (by simp) (by simp)] at h ⊢
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨va, h1, vb, h2, h3⟩ := h
          exact ⟨va, ihE hm h1, vb, ihE hm h2, h3⟩
      | ternary c a b | matchOpt c x a b =>
        simp only [eval, Except.bind_ok_iff] at h ⊢
        obtain ⟨w, h1, h2⟩ := h
        refine ⟨w, ihE hm h1, ?_⟩
        split at h2
        next => exact ihE hm h2
        next => exact ihE hm h2
        next => simp at h2
      | letE x e body =>
        simp only [eval, Except.bind_ok_iff] at h ⊢
        obtain ⟨w, h1, h2⟩ := h
        exact ⟨w, ihE hm h1, ihE hm h2⟩
      | typed e ty =>
        simp only [eval] at h ⊢
        exact ihE hm h
    · intro m env inFn ls es vs hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases es with
      | nil => exact h
      | cons e es =>
        simp only [evalList, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, vs', h2, h3⟩ := h
        exact ⟨v, ihE hm h1, vs', ihL hm h2, h3⟩
    · intro m env inFn ls es ss hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases es with
      | nil => exact h
      | cons e es =>
        simp only [evalDisplays, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, s, hs, ss', h2, h3⟩ := h
        exact ⟨v, ihE hm h1, s, hs, ss', ihD hm h2, h3⟩
    · intro m env inFn ls ps body xs i ys hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases xs with
      | nil => exact h
      | cons x xs =>
        simp only [evalMap, Except.bind_ok_iff] at h ⊢
        obtain ⟨y, h1, ys', h2, h3⟩ := h
        exact ⟨y, ihE hm h1, ys', ihM hm h2, h3⟩
    · intro m env inFn ls ps body xs i ys hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases xs with
      | nil => exact h
      | cons x xs =>
        simp only [evalFilter, Except.bind_ok_iff] at h ⊢
        obtain ⟨k, h1, ys', h2, h3⟩ := h
        exact ⟨k, ihE hm h1, ys', ihF hm h2, h3⟩
    · intro m env inFn ls w b fs i vs hm h
      obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
      have hm : n ≤ m := by omega
      cases fs with
      | nil => exact h
      | cons f fs =>
        simp only [evalFields] at h ⊢
        split at h
        next e he =>
          simp only [Except.bind_ok_iff] at h ⊢
          obtain ⟨v, h1, vs', h2, h3⟩ := h
          exact ⟨v, ihE hm h1, vs', ihR hm h2, h3⟩
        next he =>
          split at h
          next bs =>
            simp only [Except.bind_ok_iff] at h ⊢
            obtain ⟨v, h1, vs', h2, h3⟩ := h
            exact ⟨v, h1, vs', ihR hm h2, h3⟩
          next => simp at h

theorem eval_mono {n m env inFn ls e v} (hm : n ≤ m) (h : eval n env inFn ls e = .ok v) :
    eval m env inFn ls e = .ok v := (mono_aux n).1 hm h
theorem evalList_mono {n m env inFn ls es vs} (hm : n ≤ m) (h : evalList n env inFn ls es = .ok vs) :
    evalList m env inFn ls es = .ok vs := (mono_aux n).2.1 hm h
theorem evalDisplays_mono {n m env inFn ls es ss} (hm : n ≤ m)
    (h : evalDisplays n env inFn ls es = .ok ss) : evalDisplays m env inFn ls es = .ok ss :=
  (mono_aux n).2.2.1 hm h
theorem evalMap_mono {n m env inFn ls ps body xs i ys} (hm : n ≤ m)
    (h : evalMap n env inFn ls ps body xs i = .ok ys) : evalMap m env inFn ls ps body xs i = .ok ys :=
  (mono_aux n).2.2.2.1 hm h
theorem evalFilter_mono {n m env inFn ls ps body xs i ys} (hm : n ≤ m)
    (h : evalFilter n env inFn ls ps body xs i = .ok ys) :
    evalFilter m env inFn ls ps body xs i = .ok ys :=
  (mono_aux n).2.2.2.2.1 hm h
theorem evalFields_mono {n m env inFn ls w b fs i vs} (hm : n ≤ m)
    (h : evalFields n env inFn ls w b fs i = .ok vs) : evalFields m env inFn ls w b fs i = .ok vs :=
  (mono_aux n).2.2.2.2.2 hm h

/-! ## The interpreter is complete -/

theorem eval_binop_intro {n env inFn ls a b va vb v} {op : BinOp}
    (h1 : eval n env inFn ls a = .ok va) (h2 : eval n env inFn ls b = .ok vb)
    (h3 : binop op va vb = .ok v) : eval (n + 1) env inFn ls (.binary op a b) = .ok v := by
  cases op
  case and => exact absurd h3 binop_and
  case or => exact absurd h3 binop_or
  all_goals
    rw [eval_binary_strict (by simp) (by simp)]
    simp only [Except.bind_ok_iff]
    exact ⟨_, h1, _, h2, h3⟩

theorem eval_stdlib_intro {n env inFn ls name args vs v}
    (hfd : env.prog.fns.find? (·.name == name) = .none) (h1 : evalList n env inFn ls args = .ok vs)
    (h2 : stdlib env name vs = .ok v) : eval (n + 1) env inFn ls (.call name args) = .ok v := by
  simp only [eval, hfd]
  split
  next => exact absurd h2 stdlib_map
  next => exact absurd h2 stdlib_filter
  next => exact absurd h2 stdlib_pending
  next => exact absurd h2 stdlib_failed
  next => simp only [Except.bind_ok_iff]; exact ⟨_, h1, h2⟩

local macro "lift" h:term : term =>
  `(by first
    | exact eval_mono (by omega) $h | exact evalList_mono (by omega) $h
    | exact evalDisplays_mono (by omega) $h | exact evalMap_mono (by omega) $h
    | exact evalFilter_mono (by omega) $h | exact evalFields_mono (by omega) $h)

mutual

theorem EvalR.complete {env inFn ls e v} :
    EvalR env inFn ls e v → ∃ n, eval n env inFn ls e = .ok v
  | .num | .str | .bool | .none | .emptyList => ⟨1, rfl⟩
  | .some h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, h, rfl⟩⟩
  | .template h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, h, rfl⟩⟩
  | .local hl => ⟨1, by simp [eval, hl]⟩
  | .global hl hf hg => ⟨1, by subst hf; simpa [eval, hl] using hg⟩
  | .member h hi hv => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, h, by simp [hi, hv]⟩⟩
  | .fn hfd h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [eval, hfd, Except.bind_ok_iff]; exact ⟨_, lift h1, lift h2⟩⟩
  | .map hfd h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [eval, hfd, Except.bind_ok_iff]; exact ⟨_, lift h1, _, rfl, _, lift h2, rfl⟩⟩
  | .filter hfd h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [eval, hfd, Except.bind_ok_iff]; exact ⟨_, lift h1, _, rfl, _, lift h2, rfl⟩⟩
  | .pendingSettled hfd hr hs => ⟨1, by simp [eval, hfd, hr, Option.elim_err_const, hs]⟩
  | .pendingOther hfd hr => ⟨1, by simp [eval, hfd, hr]⟩
  | .failedSettled hfd hr hs => ⟨1, by simp [eval, hfd, hr, Option.elim_err_const, hs]⟩
  | .failedOther hfd hr => ⟨1, by simp [eval, hfd, hr]⟩
  | .stdlib hfd h1 h2 => by
    obtain ⟨n, h1⟩ := h1.complete
    exact ⟨n + 1, eval_stdlib_intro hfd h1 h2⟩
  | .record hd h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by
      simp only [eval, Except.bind_ok_iff]; exact ⟨_, rfl, _, by simp [hd], _, h, rfl⟩⟩
  | .recordBase h1 hd h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [eval, Except.bind_ok_iff]
      exact ⟨_, lift h1, by simp only [Except.bind_ok_iff]; exact ⟨_, rfl, _, by rw [hd]; rfl, _, lift h2, rfl⟩⟩⟩
  | .neg h | .not h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, h, rfl⟩⟩
  | .andFalse h | .orTrue h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, h, rfl⟩⟩
  | .andTrue h1 h2 | .orFalse h1 h2 | .ternaryTrue h1 h2 | .ternaryFalse h1 h2
  | .matchSome h1 h2 | .matchNone h1 h2 | .letE h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by simp only [eval, Except.bind_ok_iff]; exact ⟨_, lift h1, lift h2⟩⟩
  | .typed h => by
    obtain ⟨n, h⟩ := h.complete
    exact ⟨n + 1, by simp only [eval]; exact h⟩
  | .binop h1 h2 h3 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, eval_binop_intro (lift h1) (lift h2) h3⟩

theorem ListR.complete {env inFn ls es vs} :
    ListR env inFn ls es vs → ∃ n, evalList n env inFn ls es = .ok vs
  | .nil => ⟨1, rfl⟩
  | .cons h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by simp only [evalList, Except.bind_ok_iff]; exact ⟨_, lift h1, _, lift h2, rfl⟩⟩

theorem DisplaysR.complete {env inFn ls es ss} :
    DisplaysR env inFn ls es ss → ∃ n, evalDisplays n env inFn ls es = .ok ss
  | .nil => ⟨1, rfl⟩
  | .cons h1 hs h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [evalDisplays, Except.bind_ok_iff]; exact ⟨_, lift h1, _, hs, _, lift h2, rfl⟩⟩

theorem MapR.complete {env inFn ls ps body xs i ys} :
    MapR env inFn ls ps body xs i ys → ∃ n, evalMap n env inFn ls ps body xs i = .ok ys
  | .nil => ⟨1, rfl⟩
  | .cons h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by simp only [evalMap, Except.bind_ok_iff]; exact ⟨_, lift h1, _, lift h2, rfl⟩⟩

theorem FilterR.complete {env inFn ls ps body xs i ys} :
    FilterR env inFn ls ps body xs i ys → ∃ n, evalFilter n env inFn ls ps body xs i = .ok ys
  | .nil => ⟨1, rfl⟩
  | .keep h1 h2 | .drop h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by simp only [evalFilter, Except.bind_ok_iff]; exact ⟨_, lift h1, _, lift h2, rfl⟩⟩

theorem FieldsR.complete {env inFn ls w b fs i vs} :
    FieldsR env inFn ls w b fs i vs → ∃ n, evalFields n env inFn ls w b fs i = .ok vs
  | .nil => ⟨1, rfl⟩
  | .written he h1 h2 => by
    obtain ⟨n1, h1⟩ := h1.complete
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨max n1 n2 + 1, by
      simp only [evalFields, he, Except.bind_ok_iff]; exact ⟨_, lift h1, _, lift h2, rfl⟩⟩
  | .base he hb h2 => by
    obtain ⟨n2, h2⟩ := h2.complete
    exact ⟨n2 + 1, by
      simp only [evalFields, he, Except.bind_ok_iff]; exact ⟨_, by simp [hb], _, h2, rfl⟩⟩

end

/-! ## Statements: the interpreter is sound and complete -/

/-- The interpreter's answers are derivations. -/
theorem eval_sound {n env inFn ls e v} (h : eval n env inFn ls e = .ok v) : EvalR env inFn ls e v :=
  (sound_aux n).1 h
theorem evalList_sound {n env inFn ls es vs} (h : evalList n env inFn ls es = .ok vs) :
    ListR env inFn ls es vs :=
  (sound_aux n).2.1 h

theorem exec_sound : ∀ {n env ls ss fx fx'}, exec n env ls ss fx = .ok fx' → ExecR env ls ss fx fx'
  | 0, _, _, _, _, _, h => by simp [exec] at h
  | n + 1, env, ls, ss, fx, fx', h => by
    cases ss with
    | nil => simp [exec] at h; subst h; exact .nil
    | cons s rest =>
      cases s with
      | letS x e =>
        simp only [exec, Except.bind_ok_iff] at h
        obtain ⟨v, h1, h2⟩ := h
        exact .letS (eval_sound h1) (exec_sound h2)
      | assign t e =>
        simp only [exec, Except.bind_ok_iff] at h
        obtain ⟨v, h1, h2⟩ := h
        split at h2
        next hr =>
          simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2
          obtain ⟨_, rfl, h3⟩ := h2
          exact .assignRoot (eval_sound h1) hr (exec_sound h3)
        next hr =>
          split at h2
          next hw =>
            split at h2
            next hs =>
              simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2
              obtain ⟨_, rfl, h3⟩ := h2
              exact .assignRow (eval_sound h1) (by simpa using hr) hw hs (exec_sound h3)
            next => simp at h2
          next => simp at h2
      | command n args | send t src args =>
        simp only [exec, Except.bind_ok_iff] at h
        obtain ⟨vs, h1, h2⟩ := h
        first
          | exact .command (evalList_sound h1) (exec_sound h2)
          | exact .send (evalList_sound h1) (exec_sound h2)
      | refresh t =>
        simp only [exec] at h
        exact .refresh (exec_sound h)
      | ifS c thn els =>
        simp only [exec, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next =>
          simp only [Except.bind_ok_iff] at h2; obtain ⟨fx₁, h2, h3⟩ := h2
          exact .ifTrue (eval_sound h1) (exec_sound h2) (exec_sound h3)
        next =>
          simp only [Except.bind_ok_iff] at h2; obtain ⟨fx₁, h2, h3⟩ := h2
          exact .ifFalse (eval_sound h1) (exec_sound h2) (exec_sound h3)
        next => simp at h2
      | matchS subj x sm nn =>
        simp only [exec, Except.bind_ok_iff] at h
        obtain ⟨w, h1, h2⟩ := h
        split at h2
        next =>
          simp only [Except.bind_ok_iff] at h2; obtain ⟨fx₁, h2, h3⟩ := h2
          exact .matchSome (eval_sound h1) (exec_sound h2) (exec_sound h3)
        next =>
          simp only [Except.bind_ok_iff] at h2; obtain ⟨fx₁, h2, h3⟩ := h2
          exact .matchNone (eval_sound h1) (exec_sound h2) (exec_sound h3)
        next => simp at h2

theorem exec_mono : ∀ {n m env ls ss fx fx'}, n ≤ m → exec n env ls ss fx = .ok fx' →
    exec m env ls ss fx = .ok fx'
  | 0, _, _, _, _, _, _, _, h => by simp [exec] at h
  | n + 1, m, env, ls, ss, fx, fx', hm, h => by
    obtain ⟨m, rfl⟩ : ∃ m', m = m' + 1 := ⟨m - 1, by omega⟩
    have hm : n ≤ m := by omega
    cases ss with
    | nil => exact h
    | cons s rest =>
      cases s with
      | letS x e =>
        simp only [exec, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, h2⟩ := h
        exact ⟨v, eval_mono hm h1, exec_mono hm h2⟩
      | assign t e =>
        simp only [exec, Except.bind_ok_iff] at h ⊢
        obtain ⟨v, h1, h2⟩ := h
        refine ⟨v, eval_mono hm h1, ?_⟩
        split at h2
        next hr =>
          rw [ite_eq_left hr]
          simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2 ⊢
          obtain ⟨a, rfl, h3⟩ := h2; exact ⟨_, rfl, exec_mono hm h3⟩
        next hr =>
          rw [ite_eq_right hr]
          split at h2
          next hw =>
            rw [ite_eq_left hw]
            split at h2
            next hs =>
              rw [ite_eq_left hs]
              simp only [Except.bind_ok_iff, Except.pure_ok_iff] at h2 ⊢
              obtain ⟨a, rfl, h3⟩ := h2; exact ⟨_, rfl, exec_mono hm h3⟩
            next => simp at h2
          next => simp at h2
      | command n args | send t src args =>
        simp only [exec, Except.bind_ok_iff] at h ⊢
        obtain ⟨vs, h1, h2⟩ := h
        exact ⟨vs, evalList_mono hm h1, exec_mono hm h2⟩
      | refresh t =>
        simp only [exec] at h ⊢
        exact exec_mono hm h
      | ifS c thn els | matchS c x thn els =>
        simp only [exec, Except.bind_ok_iff] at h ⊢
        obtain ⟨w, h1, h2⟩ := h
        refine ⟨w, eval_mono hm h1, ?_⟩
        split at h2
        next =>
          simp only [Except.bind_ok_iff] at h2 ⊢
          obtain ⟨a, h2, h3⟩ := h2; exact ⟨a, exec_mono hm h2, exec_mono hm h3⟩
        next =>
          simp only [Except.bind_ok_iff] at h2 ⊢
          obtain ⟨a, h2, h3⟩ := h2; exact ⟨a, exec_mono hm h2, exec_mono hm h3⟩
        next => simp at h2

theorem ExecR.complete {env ls ss fx fx'} (h : ExecR env ls ss fx fx') :
    ∃ n, exec n env ls ss fx = .ok fx' := by
  induction h with
  | nil => exact ⟨1, rfl⟩
  | letS he _ ih =>
    obtain ⟨n1, h1⟩ := he.complete
    obtain ⟨n2, h2⟩ := ih
    exact ⟨max n1 n2 + 1, by
      simp only [exec, Except.bind_ok_iff]
      exact ⟨_, eval_mono (by omega) h1, exec_mono (by omega) h2⟩⟩
  | assignRoot he hr _ ih =>
    obtain ⟨n1, h1⟩ := he.complete
    obtain ⟨n2, h2⟩ := ih
    exact ⟨max n1 n2 + 1, by
      simp only [exec, Except.bind_ok_iff]
      refine ⟨_, eval_mono (by omega) h1, ?_⟩
      rw [ite_eq_left hr]; simp only [Except.bind_ok_iff, Except.pure_ok_iff]
      exact ⟨_, rfl, exec_mono (by omega) h2⟩⟩
  | assignRow he hr hw hs _ ih =>
    obtain ⟨n1, h1⟩ := he.complete
    obtain ⟨n2, h2⟩ := ih
    exact ⟨max n1 n2 + 1, by
      simp only [exec, Except.bind_ok_iff]
      refine ⟨_, eval_mono (by omega) h1, ?_⟩
      rw [ite_eq_right (by simp [hr]), ite_eq_left hw, ite_eq_left hs]; simp only [Except.bind_ok_iff, Except.pure_ok_iff]
      exact ⟨_, rfl, exec_mono (by omega) h2⟩⟩
  | command he _ ih | send he _ ih =>
    obtain ⟨n1, h1⟩ := he.complete
    obtain ⟨n2, h2⟩ := ih
    exact ⟨max n1 n2 + 1, by
      simp only [exec, Except.bind_ok_iff]
      exact ⟨_, evalList_mono (by omega) h1, exec_mono (by omega) h2⟩⟩
  | refresh _ ih =>
    obtain ⟨n, h⟩ := ih
    exact ⟨n + 1, by simp only [exec]; exact h⟩
  | ifTrue he _ _ ih₁ ih₂ | ifFalse he _ _ ih₁ ih₂ | matchSome he _ _ ih₁ ih₂
  | matchNone he _ _ ih₁ ih₂ =>
    obtain ⟨n0, h0⟩ := he.complete
    obtain ⟨n1, h1⟩ := ih₁
    obtain ⟨n2, h2⟩ := ih₂
    exact ⟨max n0 (max n1 n2) + 1, by
      simp only [exec, Except.bind_ok_iff]
      refine ⟨_, eval_mono (by omega) h0, ?_⟩
      simp only [Except.bind_ok_iff]
      exact ⟨_, exec_mono (by omega) h1, exec_mono (by omega) h2⟩⟩

/-! ## The theorems -/

/-- Every derivation is an answer of the interpreter at some fuel. -/
theorem eval_complete {env inFn ls e v} : EvalR env inFn ls e v → ∃ n, eval n env inFn ls e = .ok v :=
  EvalR.complete

theorem eval_iff {env inFn ls e v} : EvalR env inFn ls e v ↔ ∃ n, eval n env inFn ls e = .ok v :=
  ⟨EvalR.complete, fun ⟨_, h⟩ => eval_sound h⟩

theorem exec_complete {env ls ss fx fx'} : ExecR env ls ss fx fx' → ∃ n, exec n env ls ss fx = .ok fx' :=
  ExecR.complete

theorem exec_iff {env ls ss fx fx'} : ExecR env ls ss fx fx' ↔ ∃ n, exec n env ls ss fx = .ok fx' :=
  ⟨ExecR.complete, fun ⟨_, h⟩ => exec_sound h⟩

/-- An expression has at most one value. -/
theorem EvalR.det {env inFn ls e v₁ v₂} (h₁ : EvalR env inFn ls e v₁) (h₂ : EvalR env inFn ls e v₂) :
    v₁ = v₂ := by
  obtain ⟨n₁, h₁⟩ := h₁.complete
  obtain ⟨n₂, h₂⟩ := h₂.complete
  have := (eval_mono (Nat.le_max_left n₁ n₂) h₁).symm.trans (eval_mono (Nat.le_max_right n₁ n₂) h₂)
  simpa using this

theorem ListR.det {env inFn ls es vs₁ vs₂} (h₁ : ListR env inFn ls es vs₁) (h₂ : ListR env inFn ls es vs₂) :
    vs₁ = vs₂ := by
  obtain ⟨n₁, h₁⟩ := h₁.complete
  obtain ⟨n₂, h₂⟩ := h₂.complete
  have := (evalList_mono (Nat.le_max_left n₁ n₂) h₁).symm.trans (evalList_mono (Nat.le_max_right n₁ n₂) h₂)
  simpa using this

/-- A block has at most one outcome. -/
theorem ExecR.det {env ls ss fx fx₁ fx₂} (h₁ : ExecR env ls ss fx fx₁) (h₂ : ExecR env ls ss fx fx₂) :
    fx₁ = fx₂ := by
  obtain ⟨n₁, h₁⟩ := h₁.complete
  obtain ⟨n₂, h₂⟩ := h₂.complete
  have := (exec_mono (Nat.le_max_left n₁ n₂) h₁).symm.trans (exec_mono (Nat.le_max_right n₁ n₂) h₂)
  simpa using this

/-- Whatever fuel the interpreter answers at, it answers the same at any
other fuel at which it answers. -/
theorem eval_fuel_indep {n m env inFn ls e v w} (h₁ : eval n env inFn ls e = .ok v)
    (h₂ : eval m env inFn ls e = .ok w) : v = w :=
  (eval_sound h₁).det (eval_sound h₂)

end Contract
