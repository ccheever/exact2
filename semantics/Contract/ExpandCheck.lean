/-
The Lean half of `difftest expansion`: the Lean expander's output on a
file's component-level embedding against the Rust expander's (the flat
embedding `contract lean` emits), structurally; and the component-level
semantics against the flat semantics, by observation.

Structure is compared declaration by declaration and node by node, through
`repr`. Types are compared apart: a child's states and parameters carry
the types the checker gave the child standalone, the flat embedding the
types it inferred in the expansion, which may be more precise; only a flat
type that is not at least the expansion's (`Ty.le`) is a difference.
-/
import Contract.Expand
import Contract.CompSem
import Contract.ValTy

namespace Contract.ExpandCheck

open Contract Components

def StateDecl.untyped (s : StateDecl) : StateDecl := { s with ty := .unknown }
def ActionDecl.untyped (a : ActionDecl) : ActionDecl :=
  { a with params := a.params.map fun (p, _) => (p, .unknown) }

/-- A `repr` on one line. -/
def oneLine (s : String) : String :=
  " ".intercalate ((s.splitOn "\n").map fun l => l.trimAscii.toString)

/-- The first index where two lists' items are not `same`, with both. -/
def firstDiffBy {α} [Repr α] (same : α → α → Bool) (what : String) (xs ys : List α) :
    Option String :=
  let rec go : Nat → List α → List α → Option String
    | _, [], [] => .none
    | i, x :: _, [] => .some s!"{what}[{i}]: lean has {oneLine (reprStr x)}, rust has nothing"
    | i, [], y :: _ => .some s!"{what}[{i}]: rust has {oneLine (reprStr y)}, lean has nothing"
    | i, x :: xs, y :: ys =>
      if same x y then go (i + 1) xs ys
      else .some s!"{what}[{i}]: lean {oneLine (reprStr x)} ≠ rust {oneLine (reprStr y)}"
  go 0 xs ys

/-- The first index where two lists' `repr`s differ, with both. -/
def firstDiff {α} [Repr α] (what : String) (xs ys : List α) : Option String :=
  firstDiffBy (fun x y => reprStr x == reprStr y) what xs ys

/-- The first structural difference between the Lean expansion `l` and the
Rust one `r`, types aside. -/
def structDiff (l r : Program) : Option String :=
  firstDiff "states" (l.states.map StateDecl.untyped) (r.states.map StateDecl.untyped) <|>
  firstDiff "derives" l.derives r.derives <|>
  firstDiff "resources" l.resources r.resources <|>
  firstDiff "mutations" l.mutations r.mutations <|>
  firstDiff "actions" (l.actions.map ActionDecl.untyped) (r.actions.map ActionDecl.untyped) <|>
  firstDiff "tasks" l.tasks r.tasks <|>
  firstDiff "view" l.view r.view <|>
  firstDiff "fns" l.fns r.fns <|>
  firstDiff "routes" l.routes r.routes <|>
  (if reprStr l.router == reprStr r.router then .none else .some "router")

/-- One action's flat parameter types are at least the expansion's, one by
one: what the expansion leaves `?`, the checker may have typed. -/
def paramsLe (l r : List Ty) : Bool :=
  l.length == r.length && (l.zip r).all fun (a, b) => Ty.le a b

/-- The first lifted declaration whose flat type is not at least its type in
the Lean expansion. A curried argument's hidden parameter
(`@capture:n:aI:J`) is `?` in both expansions, as inline.rs gives it no
type; the checker types it from the handler that binds its action
(`press=go#1(3)`: `number`), as it does any untyped parameter. -/
def typeDiff (l r : Program) : Option String :=
  firstDiffBy Ty.le "state types" (l.states.map (·.ty)) (r.states.map (·.ty)) <|>
  firstDiffBy paramsLe "parameter types" (l.actions.map fun a => a.params.map (·.2))
    (r.actions.map fun a => a.params.map (·.2))

-- A typed curried capture is no difference (corpus/actions/calls/prop-row);
-- a conflict, or a type the flat embedding lost, is.
example : paramsLe [.number, .string, .unknown] [.number, .string, .string] = true := by decide
example : paramsLe [.number] [.string] = false := by decide
example : paramsLe [.number] [.unknown] = false := by decide
example : paramsLe [.unknown] [] = false := by decide

/-- Both checks for one case, as lines: `#expand-error`, `#struct`, `#types`
(each with what differs, or `ok`), then the flat observation after
`#flat` and the component-level one after `#comp`. -/
def report (comp : CProgram) (flat : Program) (o : Oracle) (events : List Observe.Event) :
    List String :=
  let structural := match Expand.expand comp with
    | .error e => ["#expand-error " ++ e]
    | .ok l =>
      ["#struct " ++ (structDiff l flat).getD "ok", "#types " ++ (typeDiff l flat).getD "ok"]
  structural ++ ["#flat"] ++ Observe.run flat o events ++ ["#comp"] ++ CompSem.crun comp o events

end Contract.ExpandCheck
