/-
The abstract syntax of an unexpanded Contract file: every component as
written, with its props, injects, `provide` section, `slot`, states,
derives and actions, and a view whose `Use` nodes are still uses.

`contract lean --components <file>` (contract/cli/src/lean.rs) emits one
of these for any program the compiler accepts. Its meaning is given
directly, instance by instance, in `Contract.CompSem`; `Contract.Expand`
is an expander mirroring `contract_syntax::inline`, and
`Contract.ExpandProof` relates the two.

Expressions and statements are `Contract.Expr` and `Contract.Stmt`: the
same language, read in a component's own scope. A handler names an action
of its component or an `action` prop; a tail call (LLP 1017 §11) is a
`command` naming an `action` prop.
-/
import Contract.Syntax

namespace Contract.Components

open Contract

/-- A prop or an inject: its declared type (`.unknown` where none resolves),
whether a type was written at all, and whether it is an `action` prop (an
action reference, never a value). -/
structure PropDecl where
  name : String
  ty : Ty := .unknown
  declared : Bool := true
  action : Bool := false
  deriving Repr, Inhabited

/-- A view node of a component as written. `id` names the node within its
component (preorder, from the emitter): an instance's identity is the path
of uses, region arms and `children` nodes that leads to it. -/
inductive CNode where
  | element (tag : String) (positional : List Expr) (props : List (String × Expr))
      (handlers : List (String × String × List Expr)) (children : List CNode)
  | when (id : Nat) (c : Expr) (thn els : List CNode)
  | each (id : Nat) (x : String) (index : Option String) (list key : Expr) (body : List CNode)
  | matchN (id : Nat) (subject : Expr) (x : String) (some none : List CNode)
  /-- `Name(arg=…, …)` with the nodes indented under it (its fill). -/
  | use (id : Nat) (name : String) (args : List (String × Expr)) (fill : List CNode)
  /-- Where a `slot` component's fill goes. -/
  | children (id : Nat)
  deriving Repr, Inhabited

/-- A component. Only the root holds resources, mutations and tasks. -/
structure CComponent where
  name : String
  props : List PropDecl := []
  injects : List PropDecl := []
  /-- The `provide` section: each fills the same-named inject of every
  component used in this one's view, unless a nearer one provides it. -/
  provides : List (String × Expr) := []
  slot : Bool := false
  states : List StateDecl := []
  derives : List DeriveDecl := []
  resources : List ResourceDecl := []
  mutations : List MutationDecl := []
  actions : List ActionDecl := []
  tasks : List TaskDecl := []
  view : List CNode := []
  deriving Repr, Inhabited

/-- A whole file, unexpanded. The router's slot is not among the root's
states: it is the compiler's (the expander puts it first). -/
structure CProgram where
  shapes : List Shape := []
  fns : List FnDecl := []
  /-- The declared shapes that are not also `fn`s: a call naming one builds
  a record ahead of any name in scope (LLP 1035.005.000 D3). -/
  records : List String := []
  root : CComponent
  /-- Every other component, in file order. -/
  components : List CComponent := []
  routes : List RouteDecl := []
  router : Option String := .none
  deriving Repr, Inhabited

def CProgram.component? (p : CProgram) (name : String) : Option CComponent :=
  p.components.find? (·.name == name)

/-- The commands the host performs (`contract_syntax::HOST_COMMANDS`): a
statement naming one is never a call. -/
def hostCommands : List String :=
  ["blur", "copyText", "deliveryActivate", "deliveryCheck", "focus", "format", "haptic", "openURL",
   "reload", "selectText", "setScheme", "showPicker", "share", "showNotification",
   "closeNotification", "saveFile", "showOpenFilePicker", "showDirectoryPicker",
   "showSaveFilePicker", "scrollIntoView", "postMessage", "preventDefault", "stopPropagation",
   "close"]

/-- `c`'s statements with every same-component call made a `call` (calls.rs
`expand_file`, the bodies apart). -/
def ownCalls (c : CComponent) : List Stmt → List Stmt
  | [] => []
  | .command n args :: rest =>
    (if !hostCommands.contains n && c.actions.any (·.name == n) then .call n args else .command n args) ::
      ownCalls c rest
  | .ifS cnd a b :: rest => .ifS cnd (ownCalls c a) (ownCalls c b) :: ownCalls c rest
  | .matchS subj x a b :: rest => .matchS subj x (ownCalls c a) (ownCalls c b) :: ownCalls c rest
  | st :: rest => st :: ownCalls c rest

def ownCallsComponent (c : CComponent) : CComponent :=
  { c with actions := c.actions.map fun a => { a with body := ownCalls c a.body } }

/-- Every component's own calls made (calls.rs `expand_file`). -/
def ownCallsProgram (p : CProgram) : CProgram :=
  { p with root := ownCallsComponent p.root, components := p.components.map ownCallsComponent }

/-- Whether a statement of `body` is `name(…)`. -/
def commandsIn (name : String) : List Stmt → Bool
  | [] => false
  | .command n _ :: rest => n == name || commandsIn name rest
  | .ifS _ a b :: rest | .matchS _ _ a b :: rest => commandsIn name a || commandsIn name b || commandsIn name rest
  | _ :: rest => commandsIn name rest

end Contract.Components
