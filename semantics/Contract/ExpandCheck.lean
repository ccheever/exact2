/-
The Lean half of `difftest expansion`: the Lean expander's output on a
file's component-level embedding against the Rust expander's (the flat
embedding `contract lean` emits), structurally; and the component-level
semantics against the flat semantics, by observation.

Structure is compared declaration by declaration and node by node, through
`repr`. Types are compared apart: a child's states and parameters carry
the types the checker gave the child standalone, the flat embedding the
types it inferred in the expansion, which may be more precise.
-/
import Contract.Expand
import Contract.CompSem

namespace Contract.ExpandCheck

open Contract Components

def StateDecl.untyped (s : StateDecl) : StateDecl := { s with ty := .unknown }
def ActionDecl.untyped (a : ActionDecl) : ActionDecl :=
  { a with params := a.params.map fun (p, _) => (p, .unknown) }

/-- A `repr` on one line. -/
def oneLine (s : String) : String :=
  " ".intercalate ((s.splitOn "\n").map fun l => l.trimAscii.toString)

/-- The first index where two lists' `repr`s differ, with both. -/
def firstDiff {α} [Repr α] (what : String) (xs ys : List α) : Option String :=
  let rec go : Nat → List α → List α → Option String
    | _, [], [] => .none
    | i, x :: _, [] => .some s!"{what}[{i}]: lean has {oneLine (reprStr x)}, rust has nothing"
    | i, [], y :: _ => .some s!"{what}[{i}]: rust has {oneLine (reprStr y)}, lean has nothing"
    | i, x :: xs, y :: ys =>
      if reprStr x == reprStr y then go (i + 1) xs ys
      else .some s!"{what}[{i}]: lean {oneLine (reprStr x)} ≠ rust {oneLine (reprStr y)}"
  go 0 xs ys

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

/-- The first difference in a lifted declaration's type. -/
def typeDiff (l r : Program) : Option String :=
  firstDiff "state types" (l.states.map (·.ty)) (r.states.map (·.ty)) <|>
  firstDiff "parameter types" (l.actions.map fun a => a.params.map (·.2))
    (r.actions.map fun a => a.params.map (·.2))

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
