/-
The abstract syntax of Contract, as a deep embedding.

A program here is what the compiler's front end accepts: the root component
after component expansion (`contract_syntax::inline::expand`), with every
used component's declarations lifted into it and every `fn`, shape and
style beside it. `contract lean <file>` (contract/cli/src/lean.rs) emits one
of these for any program the type checker accepts.

Names are kept as names: a variable is resolved by the semantics' own
lexical scoping (`Contract.Eval`), never by an index the compiler chose, so
the semantics is of the language and not of its lowering.
-/
namespace Contract

/-- A written or inferred type. `record s` is a declared shape. -/
inductive Ty where
  | number
  | bool
  | string
  | unit
  | option (t : Ty)
  | list (t : Ty)
  | record (shape : String)
  /-- A type the checker left open (the element type of a bare `[]`). -/
  | unknown
  deriving Repr, BEq, Inhabited, DecidableEq

inductive UnOp where
  | neg
  | not
  deriving Repr, BEq, Inhabited, DecidableEq

inductive BinOp where
  | add | sub | mul | div | rem
  | eq | ne | lt | le | gt | ge
  | and | or
  deriving Repr, BEq, Inhabited, DecidableEq

/-- An expression. Numbers are carried as their IEEE-754 bits so that the
embedding is exact. -/
inductive Expr where
  | num (bits : UInt64)
  | str (s : String)
  | bool (b : Bool)
  | none
  /-- `[a, b, c]`, its items left to right; `[]` is `list []` (LLP 1088
  §9.1). -/
  | list (items : List Expr)
  | some (e : Expr)
  /-- A template string: each part is printed with `toString` and the
  results concatenated. A literal part is a `str`. -/
  | template (parts : List Expr)
  | var (name : String)
  | member (e : Expr) (field : String)
  /-- `name(args)`: a `fn`, else a roster entry (`map`/`filter` take an
  `arrow` as their second argument). -/
  | call (name : String) (args : List Expr)
  /-- `Shape(base?, field: value, …)`: a record, its unnamed fields copied
  from `base`. Fields are in the order written. -/
  | record (shape : String) (base : Option Expr) (fields : List (String × Expr))
  | unary (op : UnOp) (e : Expr)
  | binary (op : BinOp) (a b : Expr)
  | ternary (c a b : Expr)
  | matchOpt (subject : Expr) (x : String) (some none : Expr)
  /-- `(p₀, p₁) => body`: only ever the callback of `map` or `filter`. -/
  | arrow (params : List String) (body : Expr)
  /-- `value` bound once to `x` in `body` (the expander's, no surface syntax). -/
  | letE (x : String) (value body : Expr)
  /-- An authored named argument (`share(title: …)`), outside a record. -/
  | named (name : String) (e : Expr)
  /-- `e` read at a declared type (the expander's, for a prop's or an
  inject's argument): a type ascription, with no effect on the value. -/
  | typed (e : Expr) (ty : Ty)
  deriving Repr, Inhabited

/-- A statement in an action body. -/
inductive Stmt where
  | letS (x : String) (e : Expr)
  | assign (target : String) (e : Expr)
  | command (name : String) (args : List Expr)
  | send (target source : String) (args : List Expr)
  | refresh (target : String)
  | ifS (c : Expr) (thn els : List Stmt)
  | matchS (subject : Expr) (x : String) (some none : List Stmt)
  /-- A call of the program's action `action` (LLP 1089 D9). `args` is
  its whole parameter list: a lifted callee's capture parameters, the
  arguments curried where it was passed, then the call's own. The callee
  runs in the caller's commit, reading the state the action started with;
  its meaning is given here, not by the compiler's expansion, so the
  differential tests check that expansion. -/
  | call (action : String) (args : List Expr)
  deriving Repr, Inhabited

/-- A view node. `props` are an element's attributes that are values;
`handlers` are its event attributes: (event, action, curried arguments). -/
inductive Node where
  | element (tag : String) (positional : List Expr) (props : List (String × Expr))
      (handlers : List (String × String × List Expr)) (children : List Node)
  /-- `tag` names the region, as an `each`'s does: its arms (0 then, 1
  else) own the state of the children used in them. -/
  | when (tag : Nat) (c : Expr) (thn els : List Node)
  | each (tag : Nat) (x : String) (index : Option String) (list key : Expr) (body : List Node)
  /-- Arm 0 is `some`, arm 1 is `none`. -/
  | matchN (tag : Nat) (subject : Expr) (x : String) (some none : List Node)
  deriving Repr, Inhabited

structure Field where
  name : String
  ty : Ty
  deriving Repr, Inhabited

structure Shape where
  name : String
  fields : List Field
  deriving Repr, Inhabited

structure FnDecl where
  name : String
  params : List (String × Ty)
  ret : Ty
  body : Expr
  deriving Repr, Inhabited

/-- A `state`. A child component's state lives exactly as long as its
instance: `owner` is the region arm that owns it — (tag, arm) of the
innermost `each` row (arm 0), `when` arm or `match` arm around its use —
one value per arm instance; `none` for a root slot. A root slot that
holds a child used outside every region is `late`: initialized after boot
settlement, so it may read derives and resources. -/
structure StateDecl where
  name : String
  ty : Ty
  init : Expr
  owner : Option (Nat × Nat) := .none
  late : Bool := false
  deriving Repr, Inhabited

structure DeriveDecl where
  name : String
  ty : Ty
  body : Expr
  deriving Repr, Inhabited

/-- `resource name = source(args) as shape ty`. -/
structure ResourceDecl where
  name : String
  ty : Ty
  source : String
  args : List Expr
  deriving Repr, Inhabited

/-- `mutation name as shape ty`: a slot of type `option<ty>`. -/
structure MutationDecl where
  name : String
  ty : Ty
  refreshes : List String := []
  andThen : Option String := .none
  /-- `queue` (LLP 1092 D1): a send waits while an earlier one of this
  mutation waits, and is asked by an `advance`'s drain. -/
  queue : Bool := false
  deriving Repr, Inhabited

structure ActionDecl where
  name : String
  params : List (String × Ty)
  body : List Stmt
  deriving Repr, Inhabited

inductive TaskKind where
  | every
  | after
  | frame
  deriving Repr, BEq, Inhabited, DecidableEq

structure TaskDecl where
  name : String
  kind : TaskKind
  ms : Expr
  action : String
  /-- `when cond` (LLP 1092 D7): the timer exists while it holds. -/
  gate : Option Expr := .none
  /-- `key=expr`: a new key restarts the timer, as a new `each` key a row. -/
  key : Option Expr := .none
  deriving Repr, Inhabited

/-- A row of the `routes` table (LLP 1038 D2), in declaration order: the
table the compiler checked (`exact_route::Table`). A notfound row has an
empty pattern; `parent` indexes the table. -/
structure RouteDecl where
  name : String
  pattern : String
  parent : Option Nat := .none
  tab : Bool := false
  notfound : Bool := false
  deriving Repr, Inhabited

/-- A whole program: the expanded root and the file's declarations. -/
structure Program where
  shapes : List Shape := []
  fns : List FnDecl := []
  states : List StateDecl := []
  derives : List DeriveDecl := []
  resources : List ResourceDecl := []
  mutations : List MutationDecl := []
  actions : List ActionDecl := []
  tasks : List TaskDecl := []
  view : List Node := []
  /-- The route table, when the program declares `routes`. -/
  routes : List RouteDecl := []
  /-- The router's slot: the root state `routes <slot>` names (the
  expander puts it first), of type `Router`. -/
  router : Option String := .none
  /-- The strings tables `t(...)` reads (LLP 1060), the base first: each
  locale's (key, text) pairs. -/
  strings : List (String × List (String × String)) := []
  /-- The slot holding the resolved locale, when the program has strings
  tables (the plan's `locale` slot): a root state the observation leaves
  out. -/
  locale : Option String := .none
  /-- Each data source's one signature (`type-source-signature`): its
  parameter types and its answer's, unified across every use. A source
  the runner answers itself has none (each reader has its own shape). -/
  sources : List (String × List Ty × Ty) := []
  deriving Repr, Inhabited

namespace Expr
/-- A number literal from a `Float` (for hand-written programs). -/
def n (f : Float) : Expr := .num f.toBits
end Expr

end Contract
