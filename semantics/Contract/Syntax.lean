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
  | emptyList
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
  deriving Repr, Inhabited

/-- A view node. `props` are an element's attributes that are values;
`handlers` are its event attributes: (event, action, curried arguments). -/
inductive Node where
  | element (tag : String) (positional : List Expr) (props : List (String × Expr))
      (handlers : List (String × String × List Expr)) (children : List Node)
  | when (c : Expr) (thn els : List Node)
  | each (tag : Nat) (x : String) (index : Option String) (list key : Expr) (body : List Node)
  | matchN (subject : Expr) (x : String) (some none : List Node)
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

/-- A `state`. `owner` is the tag of the `each` whose rows each hold one
(a lifted child's state under an `each`), `none` for a root slot. -/
structure StateDecl where
  name : String
  ty : Ty
  init : Expr
  owner : Option Nat := .none
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
  deriving Repr, Inhabited

namespace Expr
/-- A number literal from a `Float` (for hand-written programs). -/
def n (f : Float) : Expr := .num f.toBits
end Expr

end Contract
