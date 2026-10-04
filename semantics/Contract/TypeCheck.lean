/-
The type checker as a program, and the proof that it is sound for the
judgments of `Contract.Types`: `check p = true → WellTyped p`.

`infer` computes the type `infer` in contract/types/src/lib.rs would, rule
for rule (the judgments are syntax-directed, so the checker is their
reading from left to right). `check` runs it over every body of a program
in the scope that body is evaluated in. The differential tester
(`difftest types`) runs `check` on the programs the Rust compiler emits
and on ill-typed mutants of them.
-/
import Contract.Types

namespace Contract

/-- `path("route", args…)`, given the arguments' types. -/
def pathTy (p : Program) (args : List Expr) (ts? : Option (List Ty)) : Option Ty :=
  match args, ts? with
  | .str rn :: rest, .some (_ :: ts) =>
    match pathRoute p rn with
    | .some route =>
      if ts.all (fun t => t.le .string || t.le .number) && rest.length == patternParams route.pattern
      then .some .string else .none
    | .none => .none
  | _, _ => .none

mutual

/-- The type of `e`, or `none` when the judgments give it none. -/
def infer (p : Program) (G : Scope) : Scope → Expr → Option Ty
  | _, .num _ => .some .number
  | _, .str _ => .some .string
  | _, .bool _ => .some .bool
  | _, .none => .some (.option .unknown)
  | _, .emptyList => .some (.list .unknown)
  | Γ, .some e => (infer p G Γ e).map .option
  | Γ, .template parts =>
    match inferList p G Γ parts with
    | .some ts => if ts.all Ty.displayable then .some .string else .none
    | .none => .none
  | Γ, .var x =>
    match lookupTy x Γ with
    | .some t => .some t
    | .none => lookupTy x G
  | Γ, .member e f =>
    match infer p G Γ e with
    | .some (.record s) =>
      match p.shapes.find? (·.name == s) with
      | .some sh =>
        match sh.fields.findIdx? (·.name == f) with
        | .some i => (sh.fields[i]?).map (·.ty)
        | .none => .none
      | .none => .none
    | _ => .none
  | Γ, .call name args => inferCall p G Γ name args (inferList p G Γ args)
  | Γ, .record s .none written =>
    match inferNamed p G Γ written with
    | .some wts =>
      match p.shapes.find? (·.name == s) with
      | .some sh =>
        if namesOk sh.fields wts && fieldsOk sh.fields wts false then .some (.record s) else .none
      | .none => .none
    | .none => .none
  | Γ, .record s (.some e) written =>
    match inferNamed p G Γ written with
    | .some wts =>
      match infer p G Γ e with
      | .some tb =>
        match p.shapes.find? (·.name == s) with
        | .some sh =>
          if namesOk sh.fields wts && tb.le (.record s) && fieldsOk sh.fields wts true then
            .some (.record s)
          else .none
        | .none => .none
      | .none => .none
    | .none => .none
  | Γ, .unary .neg e =>
    match infer p G Γ e with
    | .some t => if t.le .number then .some .number else .none
    | .none => .none
  | Γ, .unary .not e =>
    match infer p G Γ e with
    | .some t => if t.le .bool then .some .bool else .none
    | .none => .none
  | Γ, .binary op a b =>
    match infer p G Γ a with
    | .some ta =>
      match infer p G Γ b with
      | .some tb => binTy op ta tb
      | .none => .none
    | .none => .none
  | Γ, .ternary c a b =>
    match infer p G Γ c with
    | .some tc =>
      match infer p G Γ a with
      | .some ta =>
        match infer p G Γ b with
        | .some tb => if tc.le .bool then Ty.unify ta tb else .none
        | .none => .none
      | .none => .none
    | .none => .none
  | Γ, .matchOpt s x sm nn =>
    match infer p G Γ s with
    | .some (.option a) =>
      match infer p G ((x, a) :: Γ) sm with
      | .some ta =>
        match infer p G Γ nn with
        | .some tb => Ty.unify ta tb
        | .none => .none
      | .none => .none
    | _ => .none
  | _, .arrow _ _ => .none
  | Γ, .letE x v body =>
    match infer p G Γ v with
    | .some t => infer p G ((x, t) :: Γ) body
    | .none => .none
  | _, .named _ _ => .none
  | Γ, .typed e ty =>
    match infer p G Γ e with
    | .some t => Ty.unify ty t
    | .none => .none

/-- A call, given its arguments' types (`ts?`): a `fn`, `map`/`filter`
with a callback, `pending`/`failed` of a name, or a roster entry. -/
def inferCall (p : Program) (G : Scope) : Scope → String → List Expr → Option (List Ty) → Option Ty
  | Γ, name, [l, .arrow ps body], ts? =>
    match p.fns.find? (·.name == name) with
    | .some fd =>
      match ts? with
      | .some ts => if Ty.leAll ts (fd.params.map (·.2)) then .some fd.ret else .none
      | .none => .none
    | .none =>
      if name = "map" then
        if ps.length ≤ 2 then
          match infer p G Γ l with
          | .some (.list a) => (infer p G (bindTy ps a Γ) body).map .list
          | _ => .none
        else .none
      else if name = "filter" then
        if ps.length ≤ 2 then
          match infer p G Γ l with
          | .some (.list a) =>
            match infer p G (bindTy ps a Γ) body with
            | .some b => if b.le .bool then .some (.list a) else .none
            | .none => .none
          | _ => .none
        else .none
      else
        match ts? with
        | .some ts => rosterTy name ts
        | .none => .none
  | Γ, name, args, ts? =>
    match p.fns.find? (·.name == name) with
    | .some fd =>
      match ts? with
      | .some ts => if Ty.leAll ts (fd.params.map (·.2)) then .some fd.ret else .none
      | .none => .none
    | .none =>
      match args with
      | [.var x] =>
        if name = "pending" then
          (if (isResource p x || isMutation p x) && (lookupTy x Γ).isNone && (lookupTy x G).isSome
           then .some .bool else .none)
        else if name = "failed" then
          (if isResource p x && (lookupTy x Γ).isNone && (lookupTy x G).isSome then .some .bool else .none)
        else
          match ts? with
          | .some ts => rosterTy name ts
          | .none => .none
      | _ =>
        if name = "path" then pathTy p args ts?
        else
          match ts? with
          | .some ts => rosterTy name ts
          | .none => .none

def inferList (p : Program) (G : Scope) : Scope → List Expr → Option (List Ty)
  | _, [] => .some []
  | Γ, e :: es =>
    match infer p G Γ e with
    | .some t =>
      match inferList p G Γ es with
      | .some ts => .some (t :: ts)
      | .none => .none
    | .none => .none

def inferNamed (p : Program) (G : Scope) : Scope → List (String × Expr) → Option (List (String × Ty))
  | _, [] => .some []
  | Γ, f :: rest =>
    match inferField p G Γ f with
    | .some w =>
      match inferNamed p G Γ rest with
      | .some wts => .some (w :: wts)
      | .none => .none
    | .none => .none

def inferField (p : Program) (G : Scope) : Scope → String × Expr → Option (String × Ty)
  | Γ, (g, e) => (infer p G Γ e).map (g, ·)

end

/-- `e` has a type at most `t`. -/
def inferLe (p : Program) (G Γ : Scope) (e : Expr) (t : Ty) : Bool :=
  match infer p G Γ e with
  | .some u => u.le t
  | .none => false

/-- `e` has a displayable type. -/
def inferShows (p : Program) (G Γ : Scope) (e : Expr) : Bool :=
  match infer p G Γ e with
  | .some u => u.displayable
  | .none => false

def checkStmts (p : Program) (G : Scope) : Scope → List Stmt → Bool
  | _, [] => true
  | Γ, .letS x e :: rest =>
    match infer p G Γ e with
    | .some t => checkStmts p G ((x, t) :: Γ) rest
    | .none => false
  | Γ, .assign x e :: rest => isSlot p x && inferLe p G Γ e (slotTy p x) && checkStmts p G Γ rest
  | Γ, .command _ args :: rest => (inferList p G Γ args).isSome && checkStmts p G Γ rest
  | Γ, .send x _ args :: rest =>
    isMutation p x && (inferList p G Γ args).isSome && checkStmts p G Γ rest
  | Γ, .refresh x :: rest => isResource p x && checkStmts p G Γ rest
  | Γ, .ifS c thn els :: rest =>
    inferLe p G Γ c .bool && checkStmts p G Γ thn && checkStmts p G Γ els && checkStmts p G Γ rest
  | Γ, .matchS s x sm nn :: rest =>
    (match infer p G Γ s with
      | .some (.option a) => checkStmts p G ((x, a) :: Γ) sm
      | _ => false) && checkStmts p G Γ nn && checkStmts p G Γ rest
  | Γ, .call a args :: rest =>
    (match p.actions.find? (·.name == a), inferList p G Γ args with
      | .some ad, .some ts => Ty.leAll ts (ad.params.map (·.2))
      | _, _ => false) && checkStmts p G Γ rest

def checkArm (p : Program) (G Γ : Scope) (owner : Nat × Nat) : Bool :=
  p.states.all fun st => !decide (st.owner = .some owner) || inferLe p G Γ st.init st.ty

def checkHandler (p : Program) (G Γ : Scope) (h : String × String × List Expr) : Bool :=
  match p.actions.find? (·.name == h.2.1), inferList p G Γ h.2.2 with
  | .some a, .some ts => Ty.lePrefix ts (a.params.map (·.2))
  | _, _ => false

def checkElement (p : Program) (G Γ : Scope) (tag : String) (pos : List Expr)
    (props : List (String × Expr)) (hs : List (String × String × List Expr)) : Bool :=
  (!(tag == "text" || tag == "tspan" || tag == "option") ||
    match pos with
    | e :: _ => inferShows p G Γ e
    | [] => true) &&
  (match lookupField "testId" props with
    | .some e => inferShows p G Γ e
    | .none => true) &&
  hs.all (checkHandler p G Γ)

def checkNodes (p : Program) (G : Scope) : Scope → List Node → Bool
  | _, [] => true
  | Γ, .element tag pos props hs children :: rest =>
    checkElement p G Γ tag pos props hs && checkNodes p G Γ children && checkNodes p G Γ rest
  | Γ, .when tag c thn els :: rest =>
    inferLe p G Γ c .bool && checkArm p G Γ (tag, 0) && checkArm p G Γ (tag, 1) &&
      checkNodes p G Γ thn && checkNodes p G Γ els && checkNodes p G Γ rest
  | Γ, .each tag x ix l key body :: rest =>
    (match infer p G Γ l with
      | .some (.list a) =>
        a.complete && inferShows p G (eachScope x ix a Γ) key &&
          checkArm p G (eachScope x ix a Γ) (tag, 0) && checkNodes p G (eachScope x ix a Γ) body
      | _ => false) && checkNodes p G Γ rest
  | Γ, .matchN tag s x sm nn :: rest =>
    (match infer p G Γ s with
      | .some (.option a) => checkArm p G ((x, a) :: Γ) (tag, 0) && checkNodes p G ((x, a) :: Γ) sm
      | _ => false) && checkArm p G Γ (tag, 1) && checkNodes p G Γ nn && checkNodes p G Γ rest

def checkShapes (p : Program) : Bool := p.shapes.all fun sh => sh.fields.all (·.ty.complete)

def checkTypes (p : Program) : Bool :=
  p.states.all (·.ty.complete) && p.derives.all (·.ty.complete) &&
    p.resources.all (·.ty.complete) && p.mutations.all (·.ty.complete) &&
    p.actions.all (fun a => a.params.all (·.2.complete))

def checkFn (p : Program) (fd : FnDecl) : Bool := inferLe p [] fd.params.reverse fd.body fd.ret

/-- The `i`th state's initializer, in the scope it runs in: a root slot's
at boot, a late one's after boot settlement. An arm's states are checked
with the view, in the arm's scope. -/
def checkState (p : Program) (i : Nat) : Bool :=
  match p.states[i]? with
  | .some st =>
    match st.owner, st.late with
    | .none, false => decide (p.router = .some st.name) || inferLe p (rootScope p i) [] st.init st.ty
    | .none, true => inferLe p (lateScope p i) [] st.init st.ty
    | .some _, _ => true
  | .none => true

def checkDerive (p : Program) (d : DeriveDecl) : Bool := inferLe p (settleScope p) [] d.body d.ty
def checkResource (p : Program) (r : ResourceDecl) : Bool := (inferList p (settleScope p) [] r.args).isSome
def checkAction (p : Program) (a : ActionDecl) : Bool := checkStmts p (compScope p) a.params.reverse a.body
/-- A task's action exists and takes no parameters. -/
def taskAction (p : Program) (t : TaskDecl) : Bool :=
  match p.actions.find? (·.name == t.action) with
  | .some a => a.params.isEmpty
  | .none => false
def checkTask (p : Program) (t : TaskDecl) : Bool :=
  inferLe p (compScope p) [] t.ms .number && taskAction p t && (t.ms matches .num _)

/-- The checker: every part well typed. -/
def check (p : Program) : Bool :=
  checkShapes p && distinct (compNames p) && checkTypes p && routeShapesOK p && routerSlotOK p &&
    p.fns.all (checkFn p) &&
    (List.range p.states.length).all (checkState p) && p.derives.all (checkDerive p) &&
    p.resources.all (checkResource p) && p.actions.all (checkAction p) && p.tasks.all (checkTask p) &&
    checkNodes p (compScope p) [] p.view

/-- Each part of a program and the checker's verdict on it. -/
def checkParts (p : Program) : List (String × Bool) :=
  [("shapes", checkShapes p), ("names", distinct (compNames p)), ("types", checkTypes p),
   ("route shapes", routeShapesOK p), ("router slot", routerSlotOK p)] ++
  p.fns.map (fun fd => (s!"fn {fd.name}", checkFn p fd)) ++
  (List.range p.states.length).map (fun i =>
    (s!"state {((p.states[i]?).map (·.name)).getD ""}", checkState p i)) ++
  p.derives.map (fun d => (s!"derive {d.name}", checkDerive p d)) ++
  p.resources.map (fun r => (s!"resource {r.name}", checkResource p r)) ++
  p.actions.map (fun a => (s!"action {a.name}", checkAction p a)) ++
  p.tasks.map (fun t => (s!"task {t.name}", checkTask p t)) ++
  [("view", checkNodes p (compScope p) [] p.view)]

/-- The first part the checker refuses, if any (a diagnostic). -/
def checkFailure (p : Program) : Option String := ((checkParts p).find? (!·.2)).map (·.1)

/-! ## Soundness of the checker -/

/-- A call whose arguments are not `(list, callback)`: a `fn`, `pending` or
`failed` of a name, or a roster entry. -/
theorem inferCall_rest {p : Program} {G Γ : Scope} {name : String} {args : List Expr}
    {ts? : Option (List Ty)} {t : Ty} (hne : ∀ l ps body, args ≠ [l, .arrow ps body])
    (hts : ∀ ts, ts? = .some ts → ListTy p G Γ args ts) (h : inferCall p G Γ name args ts? = .some t) :
    HasTy p G Γ (.call name args) t := by
  unfold inferCall at h
  split at h
  · next l ps body => exact absurd rfl (hne l ps body)
  · split at h
    · next fd hfd =>
      split at h
      · next ts =>
        split at h
        · next hle => simp at h; subst h; exact .fn hfd (hts ts rfl) hle
        · simp at h
      · simp at h
    · next hfd =>
      split at h
      · next x =>
        split at h
        · next hn =>
          subst hn
          split at h
          · next hr =>
            simp only [Bool.and_eq_true, Option.isNone_iff_eq_none] at hr
            simp at h; subst h; exact .pending hfd hr.1.1 hr.1.2 hr.2
          · simp at h
        · split at h
          · next hn =>
            subst hn
            split at h
            · next hr =>
              simp only [Bool.and_eq_true, Option.isNone_iff_eq_none] at hr
              simp at h; subst h; exact .failed hfd hr.1.1 hr.1.2 hr.2
            · simp at h
          · split at h
            · next ts => exact .roster hfd (hts ts rfl) h
            · simp at h
      · split at h
        · next hn =>
          subst hn
          simp only [pathTy] at h
          split at h
          · next rn rest t0 ts =>
            split at h
            · next route hr =>
              split at h
              · next hok =>
                simp only [Bool.and_eq_true, List.all_eq_true, beq_iff_eq] at hok
                simp at h; subst h
                have hl := hts _ rfl
                cases hl with
                | cons _ hrest => exact .path hfd hr hrest hok.1 hok.2
              · simp at h
            · simp at h
          · simp at h
        · split at h
          · next ts => exact .roster hfd (hts ts rfl) h
          · simp at h

mutual

theorem infer_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (e : Expr) {t : Ty},
    infer p G Γ e = .some t → HasTy p G Γ e t
  | Γ, .num _, t, h => by simp [infer] at h; subst h; exact .num
  | Γ, .str _, t, h => by simp [infer] at h; subst h; exact .str
  | Γ, .bool _, t, h => by simp [infer] at h; subst h; exact .bool
  | Γ, .none, t, h => by simp [infer] at h; subst h; exact .none
  | Γ, .emptyList, t, h => by simp [infer] at h; subst h; exact .emptyList
  | Γ, .some e, t, h => by
    simp only [infer, Option.map_eq_some_iff] at h
    obtain ⟨u, hu, rfl⟩ := h
    exact .some (infer_sound e hu)
  | Γ, .template parts, t, h => by
    simp only [infer] at h
    split at h
    · next ts hts =>
      split at h
      · next hd => simp at h; subst h; exact .template (inferList_sound parts hts) (by simpa using hd)
      · simp at h
    · simp at h
  | Γ, .var x, t, h => by
    simp only [infer] at h
    split at h
    · next u hu => simp at h; subst h; exact .local hu
    · next hu => exact .global hu h
  | Γ, .member e f, t, h => by
    simp only [infer] at h
    split at h
    · next s hs =>
      split at h
      · next sh hsh =>
        split at h
        · next i hi =>
          simp only [Option.map_eq_some_iff] at h
          obtain ⟨fld, hfld, rfl⟩ := h
          exact .member (infer_sound e hs) hsh hi hfld
        · simp at h
      · simp at h
    · simp at h
  | Γ, .call name args, t, h => by
    simp only [infer] at h
    exact inferCall_sound name args (fun ts hts => inferList_sound args hts) h
  | Γ, .record s .none written, t, h => by
    simp only [infer] at h
    split at h
    · next wts hw =>
      split at h
      · next sh hsh =>
        split at h
        · next hok =>
          simp only [Bool.and_eq_true] at hok
          simp at h; subst h; exact .record hsh (inferNamed_sound written hw) hok.2 hok.1
        · simp at h
      · simp at h
    · simp at h
  | Γ, .record s (.some e) written, t, h => by
    simp only [infer] at h
    split at h
    · next wts hw =>
      split at h
      · next tb htb =>
        split at h
        · next sh hsh =>
          split at h
          · next hok =>
            simp only [Bool.and_eq_true] at hok
            simp at h; subst h
            exact .recordBase (infer_sound e htb) hok.1.2 hsh (inferNamed_sound written hw) hok.2 hok.1.1
          · simp at h
        · simp at h
      · simp at h
    · simp at h
  | Γ, .unary op e, t, h => by
    cases op <;> simp only [infer] at h <;> split at h
    · next u hu =>
      split at h
      · next hle => simp at h; subst h; exact .neg (infer_sound e hu) hle
      · simp at h
    · simp at h
    · next u hu =>
      split at h
      · next hle => simp at h; subst h; exact .not (infer_sound e hu) hle
      · simp at h
    · simp at h
  | Γ, .binary op a b, t, h => by
    simp only [infer] at h
    split at h
    · next ta ha =>
      split at h
      · next tb hb => exact .binary (infer_sound a ha) (infer_sound b hb) h
      · simp at h
    · simp at h
  | Γ, .ternary c a b, t, h => by
    simp only [infer] at h
    split at h
    · next tc hc =>
      split at h
      · next ta ha =>
        split at h
        · next tb hb =>
          split at h
          · next hle => exact .ternary (infer_sound c hc) hle (infer_sound a ha) (infer_sound b hb) h
          · simp at h
        · simp at h
      · simp at h
    · simp at h
  | Γ, .matchOpt s x sm nn, t, h => by
    simp only [infer] at h
    split at h
    · next a hs =>
      split at h
      · next ta ha =>
        split at h
        · next tb hb => exact .matchOpt (infer_sound s hs) (infer_sound sm ha) (infer_sound nn hb) h
        · simp at h
      · simp at h
    · simp at h
  | Γ, .arrow _ _, t, h => by simp [infer] at h
  | Γ, .letE x v body, t, h => by
    simp only [infer] at h
    split at h
    · next u hu => exact .letE (infer_sound v hu) (infer_sound body h)
    · simp at h
  | Γ, .named _ _, t, h => by simp [infer] at h
  | Γ, .typed e ty, t, h => by
    simp only [infer] at h
    split at h
    · next u hu => exact .typed (infer_sound e hu) h
    · simp at h

theorem inferCall_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (name : String) (args : List Expr)
    {ts? : Option (List Ty)} {t : Ty}, (∀ ts, ts? = .some ts → ListTy p G Γ args ts) →
    inferCall p G Γ name args ts? = .some t → HasTy p G Γ (.call name args) t
  | Γ, name, [l, m], ts?, t, hts, h => by
    cases m with
    | arrow ps body =>
      simp only [inferCall] at h
      split at h
      · next fd hfd =>
        split at h
        · next ts =>
          split at h
          · next hle => simp at h; subst h; exact .fn hfd (hts ts rfl) hle
          · simp at h
        · simp at h
      · next hfd =>
        split at h
        · next hn =>
          subst hn
          split at h
          · next hps =>
            split at h
            · next a ha =>
              simp only [Option.map_eq_some_iff] at h
              obtain ⟨b, hb, rfl⟩ := h
              exact .map hfd hps (infer_sound l ha) (infer_sound body hb)
            · simp at h
          · simp at h
        · split at h
          · next hn =>
            subst hn
            split at h
            · next hps =>
              split at h
              · next a ha =>
                split at h
                · next b hb =>
                  split at h
                  · next hbl =>
                    simp at h; subst h
                    exact .filter hfd hps (infer_sound l ha) (infer_sound body hb) hbl
                  · simp at h
                · simp at h
              · simp at h
            · simp at h
          · split at h
            · next ts => exact .roster hfd (hts ts rfl) h
            · simp at h
    | _ => exact inferCall_rest (by simp) hts h
  | Γ, name, [], ts?, t, hts, h => inferCall_rest (by simp) hts h
  | Γ, name, [_], ts?, t, hts, h => inferCall_rest (by simp) hts h
  | Γ, name, _ :: _ :: _ :: _, ts?, t, hts, h => inferCall_rest (by simp) hts h

theorem inferList_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (es : List Expr) {ts : List Ty},
    inferList p G Γ es = .some ts → ListTy p G Γ es ts
  | Γ, [], ts, h => by simp [inferList] at h; subst h; exact .nil
  | Γ, e :: es, ts, h => by
    simp only [inferList] at h
    split at h
    · next t he =>
      split at h
      · next ts' hes => simp at h; subst h; exact .cons (infer_sound e he) (inferList_sound es hes)
      · simp at h
    · simp at h

theorem inferNamed_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (written : List (String × Expr))
    {wts : List (String × Ty)}, inferNamed p G Γ written = .some wts → NamedTy p G Γ written wts
  | Γ, [], wts, h => by simp [inferNamed] at h; subst h; exact .nil
  | Γ, (g, e) :: rest, wts, h => by
    simp only [inferNamed] at h
    split at h
    · next w hw =>
      split at h
      · next wts' hr =>
        simp at h; subst h
        simp only [inferField, Option.map_eq_some_iff] at hw
        obtain ⟨t, he, rfl⟩ := hw
        exact .cons (infer_sound e he) (inferNamed_sound rest hr)
      · simp at h
    · simp at h

end

theorem inferLe_sound {p : Program} {G Γ e t} (h : inferLe p G Γ e t = true) :
    ∃ u, HasTy p G Γ e u ∧ u.le t = true := by
  simp only [inferLe] at h
  split at h
  · next u hu => exact ⟨u, infer_sound e hu, h⟩
  · simp at h

theorem inferShows_sound {p : Program} {G Γ e} (h : inferShows p G Γ e = true) :
    ∃ u, HasTy p G Γ e u ∧ u.displayable = true := by
  simp only [inferShows] at h
  split at h
  · next u hu => exact ⟨u, infer_sound e hu, h⟩
  · simp at h

theorem checkStmts_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (ss : List Stmt),
    checkStmts p G Γ ss = true → StmtsTy p G Γ ss
  | _, [], _ => by simp [StmtsTy]
  | Γ, .letS x e :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts] at h
    split at h
    · next t ht => exact ⟨t, infer_sound e ht, checkStmts_sound rest h⟩
    · simp at h
  | Γ, .assign x e :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true] at h
    exact ⟨h.1.1, inferLe_sound h.1.2, checkStmts_sound rest h.2⟩
  | Γ, .command _ args :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true, Option.isSome_iff_exists] at h
    obtain ⟨⟨ts, hts⟩, hr⟩ := h
    exact ⟨⟨ts, inferList_sound args hts⟩, checkStmts_sound rest hr⟩
  | Γ, .send x _ args :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true, Option.isSome_iff_exists] at h
    obtain ⟨⟨hm, ts, hts⟩, hr⟩ := h
    exact ⟨hm, ⟨ts, inferList_sound args hts⟩, checkStmts_sound rest hr⟩
  | Γ, .refresh x :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true] at h
    exact ⟨h.1, checkStmts_sound rest h.2⟩
  | Γ, .ifS c thn els :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true] at h
    obtain ⟨⟨⟨hc, ht⟩, he⟩, hr⟩ := h
    exact ⟨inferLe_sound hc, checkStmts_sound thn ht, checkStmts_sound els he, checkStmts_sound rest hr⟩
  | Γ, .matchS s x sm nn :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true] at h
    obtain ⟨⟨hs, hn⟩, hr⟩ := h
    refine ⟨?_, checkStmts_sound nn hn, checkStmts_sound rest hr⟩
    split at hs
    · next a ha => exact ⟨a, infer_sound s ha, checkStmts_sound sm hs⟩
    · simp at hs
  | Γ, .call a args :: rest, h => by
    simp only [StmtsTy]
    simp only [checkStmts, Bool.and_eq_true] at h
    obtain ⟨hc, hr⟩ := h
    refine ⟨?_, checkStmts_sound rest hr⟩
    split at hc
    · next ad ts had hts => exact ⟨ad, had, ts, inferList_sound args hts, hc⟩
    · simp at hc

theorem checkArm_sound {p : Program} {G Γ owner} (h : checkArm p G Γ owner = true) : ArmInits p G Γ owner := by
  intro st hst ho
  simp only [checkArm, List.all_eq_true] at h
  have := h st hst
  simp only [ho, decide_true, Bool.not_true, Bool.false_or] at this
  exact inferLe_sound this

theorem checkElement_sound {p : Program} {G Γ tag pos props hs}
    (h : checkElement p G Γ tag pos props hs = true) : ElementTy p G Γ tag pos props hs := by
  simp only [checkElement, Bool.and_eq_true] at h
  obtain ⟨⟨htext, hid⟩, hh⟩ := h
  refine ⟨fun ht e he => ?_, fun e he => ?_, fun hd hd' => ?_⟩
  · simp only [ht, Bool.not_true, Bool.false_or] at htext
    cases pos with
    | nil => simp at he
    | cons e' _ => simp at he; subst he; exact inferShows_sound htext
  · simp only [Option.mem_def] at he
    rw [he] at hid
    exact inferShows_sound hid
  · simp only [List.all_eq_true] at hh
    have := hh hd hd'
    simp only [checkHandler] at this
    split at this
    · next a ts ha hts => exact ⟨a, ha, ts, inferList_sound _ hts, this⟩
    · simp at this

theorem checkNodes_sound {p : Program} {G : Scope} : ∀ {Γ : Scope} (ns : List Node),
    checkNodes p G Γ ns = true → NodesTy p G Γ ns
  | _, [], _ => by simp [NodesTy]
  | Γ, .element tag pos props hs children :: rest, h => by
    simp only [NodesTy]
    simp only [checkNodes, Bool.and_eq_true] at h
    exact ⟨checkElement_sound h.1.1, checkNodes_sound children h.1.2, checkNodes_sound rest h.2⟩
  | Γ, .when tag c thn els :: rest, h => by
    simp only [NodesTy]
    simp only [checkNodes, Bool.and_eq_true] at h
    obtain ⟨⟨⟨⟨⟨hc, h0⟩, h1⟩, ht⟩, he⟩, hr⟩ := h
    exact ⟨inferLe_sound hc, checkArm_sound h0, checkArm_sound h1, checkNodes_sound thn ht,
      checkNodes_sound els he, checkNodes_sound rest hr⟩
  | Γ, .each tag x ix l key body :: rest, h => by
    simp only [NodesTy]
    simp only [checkNodes, Bool.and_eq_true] at h
    obtain ⟨hl, hr⟩ := h
    refine ⟨?_, checkNodes_sound rest hr⟩
    split at hl
    · next a ha =>
      simp only [Bool.and_eq_true] at hl
      obtain ⟨⟨⟨hc, hk⟩, harm⟩, hb⟩ := hl
      exact ⟨a, infer_sound l ha, hc, inferShows_sound hk, checkArm_sound harm, checkNodes_sound body hb⟩
    · simp at hl
  | Γ, .matchN tag s x sm nn :: rest, h => by
    simp only [NodesTy]
    simp only [checkNodes, Bool.and_eq_true] at h
    obtain ⟨⟨⟨hs, h1⟩, hn⟩, hr⟩ := h
    refine ⟨?_, checkArm_sound h1, checkNodes_sound nn hn, checkNodes_sound rest hr⟩
    split at hs
    · next a ha =>
      simp only [Bool.and_eq_true] at hs
      exact ⟨a, infer_sound s ha, checkArm_sound hs.1, checkNodes_sound sm hs.2⟩
    · simp at hs

/-- **The checker is sound**: a program it accepts is well typed. -/
theorem check_sound {p : Program} (h : check p = true) : WellTyped p := by
  simp only [check, Bool.and_eq_true, List.all_eq_true] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨hsh, hnames⟩, htypes⟩, hrs⟩, hrslot⟩, hfns⟩, hstates⟩, hderives⟩, hres⟩, hacts⟩,
    htasks⟩, hview⟩ := h
  simp only [checkTypes, Bool.and_eq_true, List.all_eq_true] at htypes
  obtain ⟨⟨⟨⟨hst, hdt⟩, hrt⟩, hmt⟩, hat⟩ := htypes
  have hstate : ∀ i st, p.states[i]? = .some st → checkState p i = true := fun i st hi =>
    hstates i (List.mem_range.mpr (List.getElem?_eq_some_iff.mp hi).1)
  refine {
    shapes := ?_, names := hnames, routeShapes := hrs, routerSlot := hrslot, states := hst, derivesComplete := hdt, resourcesComplete := hrt,
    mutations := hmt, params := ?_, fns := ?_, rootInits := ?_, lateInits := ?_, derives := ?_,
    resources := ?_, actions := ?_, tasks := ?_, taskActions := ?_, taskLiterals := ?_, view := checkNodes_sound _ hview }
  · intro sh hs f hf
    simp only [checkShapes, List.all_eq_true] at hsh
    exact hsh sh hs f hf
  · intro a ha q hq
    exact hat a ha q hq
  · intro fd hfd; exact inferLe_sound (hfns fd hfd)
  · intro i st hi ho hl hr
    have := hstate i st hi
    simp only [checkState, hi, ho, hl, hr, decide_false, Bool.false_or] at this
    exact inferLe_sound this
  · intro i st hi ho hl
    have := hstate i st hi
    simp only [checkState, hi, ho, hl] at this
    exact inferLe_sound this
  · intro d hd; exact inferLe_sound (hderives d hd)
  · intro r hr
    have := hres r hr
    simp only [checkResource, Option.isSome_iff_exists] at this
    obtain ⟨ts, hts⟩ := this
    exact ⟨ts, inferList_sound _ hts⟩
  · intro a ha; exact checkStmts_sound _ (hacts a ha)
  · intro t ht
    simp only [checkTask, Bool.and_eq_true] at htasks
    exact inferLe_sound (htasks t ht).1.1
  · intro t ht
    simp only [checkTask, Bool.and_eq_true] at htasks
    have := (htasks t ht).1.2
    simp only [taskAction] at this
    split at this
    · next a ha => exact ⟨a, ha, by simpa using this⟩
    · simp at this
  · intro t ht
    simp only [checkTask, Bool.and_eq_true] at htasks
    have := (htasks t ht).2
    split at this
    · next b hb => exact ⟨b, hb⟩
    · simp at this

end Contract
