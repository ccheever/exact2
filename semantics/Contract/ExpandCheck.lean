/-
The Lean half of `difftest expansion`: the Lean expander's output on a
file's component-level embedding against the Rust expander's (the flat
embedding `contract lean` emits), structurally; and the component-level
semantics against the flat semantics, by observation.

Structure is compared declaration by declaration and node by node, through
`repr`. Types are compared apart: a child's states and parameters carry
the types the checker gave the child standalone, the flat embedding the
types it inferred in the expansion, which may be more precise. A lifted
type that differs is a difference, except a curried argument's hidden
parameter's, whose flat type need only be at least the expansion's
(`paramsAgree`).
-/
import Contract.Expand
import Contract.CompSem
import Contract.Types

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

/-- A curried argument's hidden parameter, `@capture:n:aI:J` (a prop's or an
inject's is `@capture:n:i`). -/
def curried (x : String) : Bool :=
  match x.splitOn ":" with
  | ["@capture", _, a, _] => a.startsWith "a"
  | _ => false

/-- One action's flat parameter types against the Lean expansion's: the
same, except a curried argument's hidden parameter's, which need only be at
least the expansion's (`Ty.le`). Both expansions leave it `?`, as inline.rs gives
it no type; the checker types it from the handler that binds its action
(`press=go#1(3)`: `number`), as it does any untyped parameter. -/
def paramsAgree (l r : List (String × Ty)) : Bool :=
  Ty.leAll (l.map (·.2)) (r.map (·.2)) &&
    (l.zip r).all fun ((x, a), (_, b)) => curried x || a == b

/-- The first lifted declaration whose flat type differs from its type in
the Lean expansion, as `paramsAgree` allows. -/
def typeDiff (l r : Program) : Option String :=
  firstDiff "state types" (l.states.map (·.ty)) (r.states.map (·.ty)) <|>
  firstDiffBy paramsAgree "parameter types" (l.actions.map (·.params)) (r.actions.map (·.params))

-- A curried argument the checker typed is no difference
-- (corpus/actions/calls/prop-row); on it, a conflict or a type the flat
-- embedding lost is. Any other parameter's types must be the same, `?`
-- against `number` included.
#guard paramsAgree [("n", .number), ("@capture:0:a1:0", .unknown)]
  [("n", .number), ("@capture:0:a1:0", .number)]
#guard paramsAgree [("@capture:0:a1:0", .unknown)] [("@capture:0:a1:0", .number)]
#guard !paramsAgree [("@capture:0:a1:0", .number)] [("@capture:0:a1:0", .string)]
#guard !paramsAgree [("@capture:0:a1:0", .number)] [("@capture:0:a1:0", .unknown)]
#guard !paramsAgree [("n", .unknown)] [("n", .number)]
#guard !paramsAgree [("@capture:0:1", .unknown)] [("@capture:0:1", .number)]
#guard !paramsAgree [("@capture:0:a1:0", .unknown)] []

-- A virtualized panel must keep its `each` directly under the list: the
-- collection owns its rows. A synthetic panel arm would hide that `each`.
private def listPanel : CProgram :=
  { root := { name := "App", view := [
      .element "main" [] [("navigationKey", .str "a"), ("navigationBack", .str "back")] [] [
        .element "list" [] [("role", .str "tabpanel"), ("virtualized", .bool true)] [] [
          .each 0 "item" .none (.list []) (.var "item") [
            .element "text" [.var "item"] [] [] []]]] ] } }

#guard match Expand.expand listPanel with
  | .ok { view := [.element _ _ _ _ [.element "list" _ _ _ [.each ..]]], .. } => true
  | _ => false

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
