/-
The type system of Contract, as judgments over the embedding.

These mirror the Rust checker (contract/types/src: `infer` in lib.rs,
records.rs, lists.rs, actions.rs, component.rs, checks.rs) for the
language the semantics gives a meaning to. A judgment is read in two
scopes: `G`, the component-level names an expression may read (states,
derives, resources, mutations — or fewer, for an initializer that runs
before some of them exist), and `Γ`, the locals (action parameters,
`let`s, `match` bindings, `each` items, callback parameters), innermost
first. A local shadows a component name, as `Env.global` is consulted only
when `lookup` finds no local.

`?` (`Ty.unknown`) is the checker's open type: `none` is an `option<?>`,
`[]` a `list<?>`, and the arms of a `?:` or `match`, an ascription, an
assignment and an argument meet through `Ty.unify`/`Ty.le` (`Ty::unify`,
`can_unify`). Where the checker defers an operand that is still `?` (an
arithmetic or comparison on it), the judgment accepts `?` too: no value
has type `?` (`Contract.ValTy`), so such an operand never produces one.

What the judgments leave to the Rust checker (it refuses more): `let`
shadowing and reassignment rules, host command signatures (a command's
arguments need only be well typed), presentation attributes (only a
`text`'s text, `testId` and handlers are typed), placeholders and
`t(...)`, and cycles among derives and `fn`s (settlement and the fuel bound
deal with those, not typing).
-/
import Contract.ValTy

namespace Contract

/-- Names and their types, innermost first. -/
abbrev Scope := List (String × Ty)

def lookupTy (x : String) : Scope → Option Ty
  | [] => .none
  | (y, t) :: rest => if x == y then .some t else lookupTy x rest

/-- A component's own scope, in the checker's order (`component_scope`):
states, derives, resources, then mutations as `option<T>`. -/
def compScope (p : Program) : Scope :=
  p.states.map (fun s => (s.name, s.ty)) ++ p.derives.map (fun d => (d.name, d.ty)) ++
  p.resources.map (fun r => (r.name, r.ty)) ++ p.mutations.map (fun m => (m.name, Ty.option m.ty))

/-- A callback's parameters bound to an item of `item` and its index
(`bindParams`). -/
def bindTy (ps : List String) (item : Ty) (Γ : Scope) : Scope :=
  match ps with
  | [] => Γ
  | [x] => (x, item) :: Γ
  | x :: i :: _ => (i, .number) :: (x, item) :: Γ

/-- An `each` row's scope: its item, then its index when named. -/
def eachScope (x : String) (ix : Option String) (item : Ty) (Γ : Scope) : Scope :=
  match ix with
  | .some i => (i, .number) :: (x, item) :: Γ
  | .none => (x, item) :: Γ

def isResource (p : Program) (x : String) : Bool := p.resources.any (·.name == x)
def isMutation (p : Program) (x : String) : Bool := p.mutations.any (·.name == x)
def isSlot (p : Program) (x : String) : Bool := p.states.any (·.name == x) || isMutation p x

/-- `ts` pairwise at most `us`, as many as `us` (a `fn`'s arguments). -/
def Ty.leAll : List Ty → List Ty → Bool
  | [], [] => true
  | t :: ts, u :: us => t.le u && leAll ts us
  | _, _ => false

/-- `ts` pairwise at most a prefix of `us` (a handler's curried arguments;
the rest, and any payload, come when the event does). -/
def Ty.lePrefix : List Ty → List Ty → Bool
  | [], _ => true
  | t :: ts, u :: us => t.le u && lePrefix ts us
  | _ :: _, [] => false

/-- The roster entries whose result is a string or a refusal on any
arguments (the formats and `t`), and those the semantics refuses as
unsupported (geometry), at the types the roster spells. `t` is typed as
`contract lean` writes it: the locale slot, the key, then each
placeholder's name and its value as `toString` prints it. -/
def unsupportedTy (name : String) (ts : List Ty) : Option Ty :=
  if name = "formatTime" ∨ name = "formatDate" then
    match ts with | [.number, .number, .string] => .some .string | _ => .none
  else if name = "formatNumber" then match ts with | [.number, .string] => .some .string | _ => .none
  else if name = "t" then
    match ts with
    | .string :: .string :: rest => if rest.all (· == .string) then .some .string else .none
    | _ => .none
  else if name = "frame" ∨ name = "measure" then
    match ts with | [.string] => .some (.record "Geometry") | _ => .none
  else if name = "toLowerCase" then match ts with | [.string] => .some .string | _ => .none
  else if name = "elementFromPoint" then
    match ts with | [.number, .number] => .some (.option .string) | _ => .none
  else .none

/-- The router's verbs and reads (LLP 1038, `Contract.Route`), at the
roster's types. -/
def routerTy (name : String) (ts : List Ty) : Option Ty :=
  if name = "open" ∨ name = "push" ∨ name = "replace" ∨ name = "select" ∨ name = "go" then
    match ts with
    | [.record r, .string] => if r = "Router" then .some (.record "Router") else .none
    | _ => .none
  else if name = "back" then
    match ts with | [.record r] => if r = "Router" then .some (.record "Router") else .none | _ => .none
  else if name = "stack" then
    match ts with | [.record r] => if r = "Router" then .some (.list (.record "Entry")) else .none | _ => .none
  else if name = "top" then
    match ts with | [.record r] => if r = "Router" then .some (.record "Entry") else .none | _ => .none
  else if name = "depth" then
    match ts with | [.record r] => if r = "Router" then .some .number else .none | _ => .none
  else if name = "params" then
    match ts with
    | [.record r, .string] => if r = "Router" then .some (.list .string) else .none
    | _ => .none
  else if name = "searchParam" then
    match ts with | [.record r, .string] => if r = "Entry" then .some .string else .none | _ => .none
  else if name = "encodeRouteSegment" then match ts with | [.string] => .some .string | _ => .none
  else unsupportedTy name ts

/-- The roster (plan/tables/format.json `stdlib`) as types: the result of a
call with arguments of these types, or `none` when the call is refused.
`any` is held to what the runner reads (`roster_accepts`). The entries the
semantics refuses as unsupported (formats, routes, geometry) are typed as
the roster spells them: their calls never produce a value here. `map`,
`filter`, `pending`, `failed` and `path` are typed by rules of their own. -/
def rosterTy (name : String) (ts : List Ty) : Option Ty :=
  if name = "now" then match ts with | [] => .some .number | _ => .none
  else if name = "length" then match ts with | [.string] | [.list _] => .some .number | _ => .none
  else if name = "isEmpty" then match ts with | [.string] | [.list _] => .some .bool | _ => .none
  else if name = "toString" then
    match ts with | [t] => if t.displayable then .some .string else .none | _ => .none
  else if name = "floor" then match ts with | [.number] => .some .number | _ => .none
  else if name = "ceil" ∨ name = "round" then match ts with | [.number] => .some .number | _ => .none
  else if name = "parseNumber" then match ts with | [.string] => .some (.option .number) | _ => .none
  else if name = "calendarDiff" then
    match ts with | [.string, .string, .string] => .some (.option .number) | _ => .none
  else if name = "max" ∨ name = "min" then
    match ts with | [.number, .number] => .some .number | _ => .none
  else if name = "first" then match ts with | [.list a] => .some (.option a) | _ => .none
  else if name = "at" then match ts with | [.list a, .number] => .some (.option a) | _ => .none
  else if name = "startsWith" ∨ name = "endsWith" then
    match ts with | [.string, .string] => .some .bool | _ => .none
  else if name = "trim" ∨ name = "encodeURIComponent" then
    match ts with | [.string] => .some .string | _ => .none
  else if name = "join" then
    match ts with
    | [.list a, s] =>
      if (a.displayable || decide (a = .unknown)) && s.le .string then .some .string else .none
    | _ => .none
  else if name = "slice" then
    match ts with
    | [.string, .number, .number] => .some .string
    | [.list a, .number, .number] => .some (.list a)
    | _ => .none
  else if name = "replaceAll" then
    match ts with | [.string, .string, .string] => .some .string | _ => .none
  /- LLP 1088 §9.1: `includes` finds text in text or, by SameValueZero, a
  string, number or bool in a list; `concat` joins two lists of one item
  type. -/
  else if name = "includes" then
    match ts with
    | [.string, .string] => .some .bool
    | [.list a, x] => match Ty.unify a x with | .some u => if u.displayable then .some .bool else .none | .none => .none
    | _ => .none
  else if name = "concat" then
    match ts with | [.list a, .list b] => (Ty.unify a b).map .list | _ => .none
  /- LLP 1088 §9.1 (2026-10-04 note): `indexOf` takes what `includes` takes
  and answers a position; `split` cuts text into a list of text. -/
  else if name = "indexOf" then
    match ts with
    | [.string, .string] => .some .number
    | [.list a, x] => match Ty.unify a x with | .some u => if u.displayable then .some .number else .none | .none => .none
    | _ => .none
  else if name = "split" then
    match ts with | [.string, .string] => .some (.list .string) | _ => .none
  else routerTy name ts

/-- A binary operator's result on operands of these types (`infer`'s
`Binary`): `+` and comparisons on numbers or strings, arithmetic on
numbers, `==`/`!=` on unifiable types, `and`/`or` on bools. -/
def binTy (op : BinOp) (a b : Ty) : Option Ty :=
  let nums := a.le .number && b.le .number
  match op with
  | .add =>
    if nums then .some .number
    else if a.le .string && b.le .string then .some .string else .none
  | .sub | .mul | .div | .rem => if nums then .some .number else .none
  | .lt | .le | .gt | .ge =>
    if nums then .some .bool
    else if a.le .string && b.le .string then .some .bool else .none
  | .eq | .ne => if a.compat b then .some .bool else .none
  | .and | .or => if a.le .bool && b.le .bool then .some .bool else .none

/-- A use of data source `src` with arguments of types `ts`, answering
`res`, against the source's one signature (`record_source`): as many
arguments, each meeting its parameter, the answer meeting the source's. -/
def sourceOk (p : Program) (src : String) (ts : List Ty) (res : Ty) : Bool :=
  match p.sources.find? (·.1 == src) with
  | .none => true
  | .some (_, ps, r) =>
    ps.length == ts.length && (ts.zip ps).all (fun (t, u) => t.compat u) && res.compat r

/-- A mutation's answer type (`unknown` for a name that is none). -/
def mutationTy (p : Program) (x : String) : Ty :=
  ((p.mutations.find? (·.name == x)).map (·.ty)).getD .unknown

/-- A shape's field types, by name. -/
def shapeTys (p : Program) (s : String) : Option (List Ty) :=
  (p.shapes.find? (·.name == s)).map (·.fields.map (·.ty))

/-- The compiler's router shapes, when the program has them (a program
without routes may declare an `Entry` of its own), are the
values `Contract.Route` builds: `Router(tab, tabs, next)`, `Tab(name,
stack)`, `Entry(id, name, url, tab, params)`, and `Params` with a string
per parameter of the table (`Route.paramNames`). -/
def routeShapesOK (p : Program) : Bool :=
  !(shapeTys p "Router").isSome ||
    (decide (shapeTys p "Router" = .some [.string, .list (.record "Tab"), .number]) &&
     decide (shapeTys p "Tab" = .some [.string, .list (.record "Entry")]) &&
     decide (shapeTys p "Entry" = .some [.number, .string, .string, .string, .record "Params"]) &&
     decide (shapeTys p "Params" = .some ((Route.paramNames p.routes).map fun _ => .string)))

/-- The router slot, when the program has one, is a state of type `Router`
(boot starts it at the launch of `/`, never its initializer). -/
def routerSlotOK (p : Program) : Bool :=
  match p.router with
  | .none => true
  | .some x => p.states.any (fun s => s.name == x && decide (s.ty = .record "Router") && s.owner.isNone && !s.late) &&
      (shapeTys p "Router").isSome

/-- The route `path(name, …)` names: a declared row, not the notfound one. -/
def pathRoute (p : Program) (name : String) : Option RouteDecl :=
  p.routes.find? (fun r => r.name == name && !r.notfound)

/-- How many parameters a pattern has. -/
def patternParams (pattern : String) : Nat :=
  ((Route.split pattern '/').filter (Route.startsWith · ':')).length

/-- No name twice. -/
def distinct : List String → Bool
  | [] => true
  | x :: xs => !xs.contains x && distinct xs

/-- A record's written fields against its shape: each declared field is
written at a type at most its own, or copied from a base. -/
def fieldsOk (fs : List Field) (written : List (String × Ty)) (hasBase : Bool) : Bool :=
  fs.all fun f =>
    match lookupTy f.name written with
    | .some t => t.le f.ty
    | .none => hasBase

/-- Every written field is the shape's, and none is written twice
(`type-record-unknown-field`, `type-record-duplicate`). -/
def namesOk (fs : List Field) (written : List (String × Ty)) : Bool :=
  written.all (fun (g, _) => fs.any (·.name == g)) && distinct (written.map (·.1))

mutual

/-- `e` has type `τ` reading component names in `G` and locals in `Γ`. -/
inductive HasTy (p : Program) (G : Scope) : Scope → Expr → Ty → Prop
  | num : HasTy p G Γ (.num b) .number
  | str : HasTy p G Γ (.str s) .string
  | bool : HasTy p G Γ (.bool b) .bool
  | none : HasTy p G Γ .none (.option .unknown)
  /-- `[a, b]`: the items' types meet (`[]` is a `list<?>`). -/
  | list : ListTy p G Γ items ts → Ty.unifyAll ts = .some t → HasTy p G Γ (.list items) (.list t)
  | some : HasTy p G Γ e t → HasTy p G Γ (.some e) (.option t)
  /-- Each part a number, bool or string. -/
  | template : ListTy p G Γ parts ts → (∀ t ∈ ts, t.displayable = true) →
      HasTy p G Γ (.template parts) .string
  | local : lookupTy x Γ = .some t → HasTy p G Γ (.var x) t
  | global : lookupTy x Γ = .none → lookupTy x G = .some t → HasTy p G Γ (.var x) t
  /-- A field by name in the shape the record is of. -/
  | member : HasTy p G Γ e (.record s) → p.shapes.find? (·.name == s) = .some sh →
      sh.fields.findIdx? (·.name == f) = .some i → sh.fields[i]? = .some fld →
      HasTy p G Γ (.member e f) fld.ty
  /-- A `fn` (it shadows the roster): every argument at most its parameter. -/
  | fn : p.fns.find? (·.name == name) = .some fd → ListTy p G Γ args ts →
      Ty.leAll ts (fd.params.map (·.2)) = true → HasTy p G Γ (.call name args) fd.ret
  | map : p.fns.find? (·.name == "map") = .none → ps.length ≤ 2 → HasTy p G Γ l (.list a) →
      HasTy p G (bindTy ps a Γ) body b → HasTy p G Γ (.call "map" [l, .arrow ps body]) (.list b)
  | filter : p.fns.find? (·.name == "filter") = .none → ps.length ≤ 2 → HasTy p G Γ l (.list a) →
      HasTy p G (bindTy ps a Γ) body b → b.le .bool = true →
      HasTy p G Γ (.call "filter" [l, .arrow ps body]) (.list a)
  /-- `pending(x)` names a resource or a mutation in scope (the checker's
  `scope.lookup`: no local shadows it, and the component scope has it). -/
  | pending : p.fns.find? (·.name == "pending") = .none → (isResource p x || isMutation p x) = true →
      lookupTy x Γ = .none → (lookupTy x G).isSome = true →
      HasTy p G Γ (.call "pending" [.var x]) .bool
  /-- `failed(x)` names a resource in scope. -/
  | failed : p.fns.find? (·.name == "failed") = .none → isResource p x = true →
      lookupTy x Γ = .none → (lookupTy x G).isSome = true →
      HasTy p G Γ (.call "failed" [.var x]) .bool
  | roster : p.fns.find? (·.name == name) = .none → ListTy p G Γ args ts → rosterTy name ts = .some t →
      HasTy p G Γ (.call name args) t
  /-- `path("route", args…)` (as the compiler expands it): a declared
  route, one argument per parameter, each a string or a number. -/
  | path : p.fns.find? (·.name == "path") = .none → pathRoute p rn = .some route →
      ListTy p G Γ args ts → (∀ t ∈ ts, (t.le .string || t.le .number) = true) →
      args.length = patternParams route.pattern →
      HasTy p G Γ (.call "path" (.str rn :: args)) .string
  /-- `Shape(field=value, …)`: every field written. -/
  | record : p.shapes.find? (·.name == s) = .some sh → NamedTy p G Γ written wts →
      fieldsOk sh.fields wts false = true → namesOk sh.fields wts = true →
      HasTy p G Γ (.record s .none written) (.record s)
  /-- `Shape(base, field=value, …)`: the rest copied from a `Shape`. -/
  | recordBase : HasTy p G Γ e tb → tb.le (.record s) = true → p.shapes.find? (·.name == s) = .some sh →
      NamedTy p G Γ written wts → fieldsOk sh.fields wts true = true → namesOk sh.fields wts = true →
      HasTy p G Γ (.record s (.some e) written) (.record s)
  | neg : HasTy p G Γ e t → t.le .number = true → HasTy p G Γ (.unary .neg e) .number
  | not : HasTy p G Γ e t → t.le .bool = true → HasTy p G Γ (.unary .not e) .bool
  | binary : HasTy p G Γ a ta → HasTy p G Γ b tb → binTy op ta tb = .some t → HasTy p G Γ (.binary op a b) t
  /-- The arms meet (`Ty::unify`). -/
  | ternary : HasTy p G Γ c tc → tc.le .bool = true → HasTy p G Γ a ta → HasTy p G Γ b tb →
      Ty.unify ta tb = .some t → HasTy p G Γ (.ternary c a b) t
  | matchOpt : HasTy p G Γ s (.option a) → HasTy p G ((x, a) :: Γ) sm ta → HasTy p G Γ nn tb →
      Ty.unify ta tb = .some t → HasTy p G Γ (.matchOpt s x sm nn) t
  | letE : HasTy p G Γ v t → HasTy p G ((x, t) :: Γ) body u → HasTy p G Γ (.letE x v body) u
  /-- An ascription: the declared type, met with the expression's (`ascribe`). -/
  | typed : HasTy p G Γ e t → Ty.unify ty t = .some u → HasTy p G Γ (.typed e ty) u

/-- Expressions and their types, pairwise. -/
inductive ListTy (p : Program) (G : Scope) : Scope → List Expr → List Ty → Prop
  | nil : ListTy p G Γ [] []
  | cons : HasTy p G Γ e t → ListTy p G Γ es ts → ListTy p G Γ (e :: es) (t :: ts)

/-- A record's written fields and their types. -/
inductive NamedTy (p : Program) (G : Scope) : Scope → List (String × Expr) → List (String × Ty) → Prop
  | nil : NamedTy p G Γ [] []
  | cons : HasTy p G Γ e t → NamedTy p G Γ rest wts → NamedTy p G Γ ((g, e) :: rest) ((g, t) :: wts)

end

/-! ## Statements -/

/-- An action body is well typed. Assignments are to states and mutations
at types at most theirs (`slotTy`); a `send` targets a mutation and a
`refresh` a resource; `if` tests a bool and `match` an option. A `let`
scopes over the rest of its block. A call names an action and passes
each of its parameters a value of its type. -/
def StmtsTy (p : Program) (G : Scope) : Scope → List Stmt → Prop
  | _, [] => True
  | Γ, .letS x e :: rest => ∃ t, HasTy p G Γ e t ∧ StmtsTy p G ((x, t) :: Γ) rest
  | Γ, .assign x e :: rest =>
    isSlot p x = true ∧ (∃ t, HasTy p G Γ e t ∧ t.le (slotTy p x) = true) ∧ StmtsTy p G Γ rest
  | Γ, .command _ args :: rest => (∃ ts, ListTy p G Γ args ts) ∧ StmtsTy p G Γ rest
  | Γ, .send x src args :: rest =>
    isMutation p x = true ∧ (∃ ts, ListTy p G Γ args ts ∧ sourceOk p src ts (mutationTy p x) = true) ∧
      StmtsTy p G Γ rest
  | Γ, .refresh x :: rest => isResource p x = true ∧ StmtsTy p G Γ rest
  | Γ, .ifS c thn els :: rest =>
    (∃ t, HasTy p G Γ c t ∧ t.le .bool = true) ∧ StmtsTy p G Γ thn ∧ StmtsTy p G Γ els ∧
      StmtsTy p G Γ rest
  | Γ, .matchS s x sm nn :: rest =>
    (∃ a, HasTy p G Γ s (.option a) ∧ StmtsTy p G ((x, a) :: Γ) sm) ∧ StmtsTy p G Γ nn ∧
      StmtsTy p G Γ rest
  /- A call (LLP 1089 D9) names an action, its arguments each at most its
  parameter's type; the callee's body is typed as the action it is. -/
  | Γ, .call a args :: rest =>
    (∃ ad, p.actions.find? (·.name == a) = .some ad ∧
      ∃ ts, ListTy p G Γ args ts ∧ Ty.leAll ts (ad.params.map (·.2)) = true) ∧ StmtsTy p G Γ rest

/-! ## Views -/

/-- The states an arm instance owns are initialized in the arm's scope. -/
def ArmInits (p : Program) (G Γ : Scope) (owner : Nat × Nat) : Prop :=
  ∀ st ∈ p.states, st.owner = .some owner → ∃ t, HasTy p G Γ st.init t ∧ t.le st.ty = true

/-- What a rendered element evaluates: a `text`'s text and a `testId`
are displayable; a handler names an action, its curried arguments at most
the action's leading parameters. -/
def ElementTy (p : Program) (G Γ : Scope) (tag : String) (pos : List Expr)
    (props : List (String × Expr)) (hs : List (String × String × List Expr)) : Prop :=
  (tag == "text" || tag == "tspan" || tag == "option" →
    ∀ e ∈ pos.head?, ∃ t, HasTy p G Γ e t ∧ t.displayable = true) ∧
  (∀ e ∈ lookupField "testId" props, ∃ t, HasTy p G Γ e t ∧ t.displayable = true) ∧
  (∀ h ∈ hs, ∃ a, p.actions.find? (·.name == h.2.1) = .some a ∧
    ∃ ts, ListTy p G Γ h.2.2 ts ∧ Ty.lePrefix ts (a.params.map (·.2)) = true)

def NodesTy (p : Program) (G : Scope) : Scope → List Node → Prop
  | _, [] => True
  | Γ, .element tag pos props hs children :: rest =>
    ElementTy p G Γ tag pos props hs ∧ NodesTy p G Γ children ∧ NodesTy p G Γ rest
  | Γ, .when tag c thn els :: rest =>
    (∃ t, HasTy p G Γ c t ∧ t.le .bool = true) ∧ ArmInits p G Γ (tag, 0) ∧ ArmInits p G Γ (tag, 1) ∧
      NodesTy p G Γ thn ∧ NodesTy p G Γ els ∧ NodesTy p G Γ rest
  | Γ, .each tag x ix l key body :: rest =>
    (∃ a, HasTy p G Γ l (.list a) ∧ a.complete = true ∧
      (∃ k, HasTy p G (eachScope x ix a Γ) key k ∧ k.displayable = true) ∧
      ArmInits p G (eachScope x ix a Γ) (tag, 0) ∧ NodesTy p G (eachScope x ix a Γ) body) ∧
      NodesTy p G Γ rest
  | Γ, .matchN tag s x sm nn :: rest =>
    (∃ a, HasTy p G Γ s (.option a) ∧ ArmInits p G ((x, a) :: Γ) (tag, 0) ∧
      NodesTy p G ((x, a) :: Γ) sm) ∧ ArmInits p G Γ (tag, 1) ∧ NodesTy p G Γ nn ∧ NodesTy p G Γ rest

/-! ## Programs -/

/-- What a root initializer sees at boot (the router slot's is never
evaluated): the root states before it that are initialized at boot (a late slot holds `()` until boot settlement is
done, so it is not readable yet). -/
def rootScope (p : Program) (i : Nat) : Scope :=
  ((p.states.take i).filter fun s => s.owner.isNone && !s.late).map fun s => (s.name, s.ty)

/-- What a late initializer sees: every root state initialized at boot and
the late ones before it, the settled derives and resources, the
mutations. -/
def lateScope (p : Program) (i : Nat) : Scope :=
  ((p.states.zipIdx.filter fun (s, j) => s.owner.isNone && (!s.late || decide (j < i))).map
    fun (s, _) => (s.name, s.ty)) ++
  p.derives.map (fun d => (d.name, d.ty)) ++ p.resources.map (fun r => (r.name, r.ty)) ++
  p.mutations.map (fun m => (m.name, Ty.option m.ty))

/-- A state lifted from a child component: a late root slot or one an arm
owns. Its name (`x#N`) cannot be written in the root's source. -/
def lifted (p : Program) (x : String) : Bool :=
  p.states.any fun s => s.name == x && (s.late || s.owner.isSome)

/-- What a derive's body or a resource's argument reads: the root's own
names — no lifted state (the expander substitutes a child's derive where
it is read, and a resource lives in the root). So settlement, which boot
runs before the late slots are initialized, reads none of them. -/
def settleScope (p : Program) : Scope := (compScope p).filter fun q => !lifted p q.1

/-- The names a component declares. -/
def compNames (p : Program) : List String :=
  p.states.map (·.name) ++ p.derives.map (·.name) ++ p.resources.map (·.name) ++
  p.mutations.map (·.name)

/-- A well-typed program. Every declared type is complete (no `?`: the
checker refuses a type it cannot infer), component names are distinct,
and every body has its declared type in the scope it is evaluated in. -/
structure WellTyped (p : Program) : Prop where
  shapes : ShapesComplete p
  names : distinct (compNames p) = true
  states : ∀ st ∈ p.states, st.ty.complete = true
  derivesComplete : ∀ d ∈ p.derives, d.ty.complete = true
  resourcesComplete : ∀ r ∈ p.resources, r.ty.complete = true
  mutations : ∀ m ∈ p.mutations, m.ty.complete = true
  params : ∀ a ∈ p.actions, ∀ q ∈ a.params, q.2.complete = true
  /-- A `fn` body sees its parameters alone. -/
  fns : ∀ fd ∈ p.fns, ∃ t, HasTy p [] fd.params.reverse fd.body t ∧ t.le fd.ret = true
  /-- The router shapes are `Contract.Route`'s, and the router slot a
  `Router` state. -/
  routeShapes : routeShapesOK p = true
  routerSlot : routerSlotOK p = true
  rootInits : ∀ i st, p.states[i]? = .some st → st.owner = .none → st.late = false →
    p.router ≠ .some st.name → ∃ t, HasTy p (rootScope p i) [] st.init t ∧ t.le st.ty = true
  lateInits : ∀ i st, p.states[i]? = .some st → st.owner = .none → st.late = true →
    ∃ t, HasTy p (lateScope p i) [] st.init t ∧ t.le st.ty = true
  derives : ∀ d ∈ p.derives, ∃ t, HasTy p (settleScope p) [] d.body t ∧ t.le d.ty = true
  /-- A resource's arguments type, and meet its source's one signature. -/
  resources : ∀ r ∈ p.resources, ∃ ts, ListTy p (settleScope p) [] r.args ts ∧
    sourceOk p r.source ts r.ty = true
  actions : ∀ a ∈ p.actions, StmtsTy p (compScope p) a.params.reverse a.body
  tasks : ∀ t ∈ p.tasks, ∃ u, HasTy p (compScope p) [] t.ms u ∧ u.le .number = true
  /-- A task names an action, which takes no parameters (the analyzer's
  `analyze-unknown-action`, `analyze-handler-arity`): a timer passes none. -/
  taskActions : ∀ t ∈ p.tasks, ∃ a, p.actions.find? (·.name == t.action) = .some a ∧ a.params = []
  /-- A task's interval is a number literal (the compiler's
  `lower-timer-literal`). -/
  taskLiterals : ∀ t ∈ p.tasks, ∃ b, t.ms = .num b
  /-- A gated task's gate and key are well typed (the compiler's
  `type-task-gate`, `type-task-key`; LLP 1092 D9). -/
  taskGates : ∀ t ∈ p.tasks, ∀ e ∈ t.gate.toList ++ t.key.toList, ∃ u, HasTy p (compScope p) [] e u
  /-- A mutation's `then` names an action that takes no parameters: the
  clock runs it with none. -/
  thenActions : ∀ m ∈ p.mutations, ∀ a, m.andThen = .some a →
    ∃ ad, p.actions.find? (·.name == a) = .some ad ∧ ad.params = []
  view : NodesTy p (compScope p) [] p.view

end Contract
