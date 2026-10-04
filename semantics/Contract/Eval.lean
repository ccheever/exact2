/-
The executable semantics of expressions and statements.

`eval` and `exec` are the interpreters; `Contract.Big` gives the same
semantics as inductive big-step relations and proves the two agree. The
runner (`runner/src/vm.rs`) is what this is tested against: every rule
here names the behaviour of the opcode sequence the compiler emits for the
construct (`contract/lower/src/expr.rs`, `stmts.rs`).
-/
import Contract.Syntax
import Contract.Value
import Contract.Route

namespace Contract

/-- Bound names, innermost first: action parameters, `let`s, a `match`'s
binding, an `each` row's item and index, a callback's parameters. -/
abbrev Locals := List (String × Value)

def lookup (x : String) : List (String × Value) → Option Value
  | [] => .none
  | (y, v) :: rest => if x == y then .some v else lookup x rest

/-- Everything an expression may read besides its locals. `derives` and
`resources` hold what has settled so far in this update; a read of one not
yet there is `pending`. `rows` are the row slots of the rows in force. -/
structure Env where
  prog : Program
  slots : List (String × Value)
  derives : List (String × Value) := []
  resources : List (String × Value) := []
  rows : List (String × Value) := []
  now : Float := 0
  deriving Inhabited

namespace Env

/-- A component-level name, in the order the checker's component scope
lists them: states, derives, resources, mutations. -/
def global (env : Env) (x : String) : Result Value :=
  match env.prog.states.find? (·.name == x) with
  | .some st =>
    match st.owner with
    | .none => (lookup x env.slots).elim (.error (.unbound x)) .ok
    | .some _ => (lookup x env.rows).elim (.error (.refused s!"row slot `{x}` read outside its row")) .ok
  | .none =>
    if env.prog.derives.any (·.name == x) then
      (lookup x env.derives).elim (.error .pending) .ok
    else if env.prog.resources.any (·.name == x) then
      (lookup x env.resources).elim (.error .pending) .ok
    else if env.prog.mutations.any (·.name == x) then
      (lookup x env.slots).elim (.error (.unbound x)) .ok
    else .error (.unbound x)

def shape (env : Env) (s : String) : Option Shape := env.prog.shapes.find? (·.name == s)

def fieldIndex (env : Env) (s field : String) : Option Nat :=
  (env.shape s).bind fun sh => sh.fields.findIdx? (·.name == field)

end Env

/-- A roster entry applied to evaluated arguments. `map` and `filter` are
not here: their callback is evaluated by `eval`. -/
def stdlib (env : Env) (f : String) (args : List Value) : Result Value :=
  match f, args with
  | "now", [] => .ok (.num env.now)
  | "length", [.list xs] => .ok (.num (Float.ofNat xs.length))
  | "length", [.str s] => .ok (.num (Float.ofNat (Str.utf16Length s)))
  | "isEmpty", [.list xs] => .ok (.bool xs.isEmpty)
  | "isEmpty", [.str s] => .ok (.bool s.isEmpty)
  | "toString", [v] => Value.str <$> v.display
  | "floor", [.num x] => .ok (.num x.floor)
  | "max", [.num a, .num b] => .ok (.num (Number.fmax a b))
  | "min", [.num a, .num b] => .ok (.num (Number.fmin a b))
  | "first", [.list xs] => .ok (match xs with | [] => .none | x :: _ => .some x)
  | "at", [.list xs, .num i] =>
    let i := if Number.isNaN i then 0 else Number.trunc i
    let len := Float.ofNat xs.length
    let j := if i < 0 then len + i else i
    if 0 ≤ j && j < len then
      .ok (match xs[j.toUInt64.toNat]? with | .some v => .some v | .none => .none)
    else .ok .none
  | "includes", [.str a, .str b] => .ok (.bool (Str.includes a b))
  | "startsWith", [.str a, .str b] => .ok (.bool (Str.startsWith a b))
  | "endsWith", [.str a, .str b] => .ok (.bool (Str.endsWith a b))
  | "trim", [.str s] => .ok (.str (Str.trim s))
  | "encodeURIComponent", [.str s] => .ok (.str (encodeURIComponent s))
  -- The router (LLP 1038, `Contract.Route`): verbs and reads over the
  -- program's table.
  | "open", [r, .str l] => Route.verb env.prog.routes r (Route.open env.prog.routes · l)
  | "push", [r, .str l] => Route.verb env.prog.routes r (Route.push env.prog.routes · l)
  | "replace", [r, .str l] => Route.verb env.prog.routes r (Route.replace env.prog.routes · l)
  | "back", [r] => Route.verb env.prog.routes r (.ok ∘ Route.back)
  | "select", [r, .str n] => Route.verb env.prog.routes r (Route.select · n)
  | "go", [r, .str l] => Route.verb env.prog.routes r (Route.go env.prog.routes · l)
  | "stack", [r] => Route.read env.prog.routes r fun x =>
      Option.some (.list ((Route.stack x).map (Route.entryValue env.prog.routes)))
  | "top", [r] => Route.read env.prog.routes r fun x => (Route.top x).map (Route.entryValue env.prog.routes)
  | "depth", [r] => Route.read env.prog.routes r fun x => Option.some (.num (Float.ofNat (Route.depth x)))
  | "params", [r, .str n] => Route.read env.prog.routes r fun x =>
      Option.some (.list ((Route.params x n).map .str))
  | "searchParam", [e, .str n] =>
    match Route.entryOf env.prog.routes e with
    | .some en => .ok (.str (Route.searchParam en.url n))
    | .none => .error (.type "`searchParam` of a value that is not an entry")
  | "encodeRouteSegment", [.str s] =>
    match Route.encodeRouteSegment s with
    | .some t => .ok (.str t)
    | .none => .error (.refused "a path parameter cannot be empty, `.` or `..`")
  | "path", .str name :: vs => Route.pathValue env.prog.routes name vs
  | "join", [.list xs, .str sep] =>
    match xs with
    | [.str s] => .ok (.str s)
    | _ => do
      let parts ← xs.mapM fun
        | v@(.str _) | v@(.num _) | v@(.bool _) => v.display
        | _ => .error (.type "join of an item that is not a string, number or bool")
      .ok (.str (sep.intercalate parts))
  | "length", _ | "isEmpty", _ | "floor", _ | "max", _ | "min", _ | "first", _ | "at", _
  | "includes", _ | "startsWith", _ | "endsWith", _ | "trim", _ | "join", _
  | "encodeURIComponent", _ => .error (.type s!"`{f}` of arguments it does not take")
  | f, _ => .error (.unsupported s!"roster entry `{f}`")

/-- A binary operator other than `and`/`or`, on evaluated operands. `+`
concatenates two strings and adds two numbers (the compiler picks `Concat`
by the left operand's static type). -/
def binop (op : BinOp) (a b : Value) : Result Value :=
  let num2 (f : Float → Float → Value) : Result Value :=
    match a, b with
    | .num x, .num y => .ok (f x y)
    | _, _ => .error (.type "arithmetic or comparison on values that are not numbers")
  match op with
  | .add =>
    match a, b with
    | .str x, .str y => .ok (.str (x ++ y))
    | .str _, _ => .error (.type "concatenation of a value that is not a string")
    | _, _ => num2 fun x y => .num (x + y)
  | .sub => num2 fun x y => .num (x - y)
  | .mul => num2 fun x y => .num (x * y)
  | .div => num2 fun x y => .num (x / y)
  | .rem => num2 fun x y => .num (Number.fmod x y)
  | .lt => num2 fun x y => .bool (x < y)
  | .le => num2 fun x y => .bool (x ≤ y)
  | .gt => num2 fun x y => .bool (x > y)
  | .ge => num2 fun x y => .bool (x ≥ y)
  | .eq => (Value.equal a b).elim (.error (.type "`==` on values of two types")) (.ok ∘ .bool)
  | .ne => (Value.equal a b).elim (.error (.type "`!=` on values of two types")) (fun e => .ok (.bool !e))
  | .and | .or => .error (.type "short-circuit operator evaluated strictly")

/-- Bind a callback's parameters to an item and its index. -/
def bindParams (ps : List String) (item : Value) (i : Nat) (ls : Locals) : Locals :=
  match ps with
  | [] => ls
  | [p] => (p, item) :: ls
  | p :: q :: _ => (q, .num (Float.ofNat i)) :: (p, item) :: ls

/-- The expression written for field `f`, if any. -/
def lookupField (f : String) : List (String × Expr) → Option Expr
  | [] => .none
  | (g, e) :: rest => if f == g then .some e else lookupField f rest

/-- Fuel ran out: the interpreter's depth bound, never reached by a
program the compiler accepts at the fuel `run` gives (`Contract.fuel`). -/
def outOfFuel : Err := .refused "out of fuel"

/-- Enough fuel for any evaluation the runner's own bounds admit. -/
def fuel : Nat := 1 <<< 24

mutual

/-- Evaluate an expression. `inFn` hides the component's names: a `fn`
body sees only its parameters (LLP 1017 P5). -/
def eval : Nat → Env → Bool → Locals → Expr → Result Value
  | 0, _, _, _, _ => .error outOfFuel
  | fuel + 1, env, inFn, ls, e =>
  match e with
  | .num b => .ok (.num (Float.ofBits b))
  | .str s => .ok (.str s)
  | .bool b => .ok (.bool b)
  | .none => .ok .none
  | .emptyList => .ok (.list [])
  | .some e => do .ok (.some (← eval fuel env inFn ls e))
  | .template parts => do
    let ss ← evalDisplays fuel env inFn ls parts
    .ok (.str (String.join ss))
  | .var x =>
    match lookup x ls with
    | .some v => .ok v
    | .none => if inFn then .error (.unbound x) else env.global x
  | .member e f => do
    match ← eval fuel env inFn ls e with
    | .record shape fields =>
      match env.fieldIndex shape f with
      | .some i => (fields[i]?).elim (.error (.type s!"short record `{shape}`")) .ok
      | .none => .error (.type s!"`{shape}` has no field `{f}`")
    | _ => .error (.type "member of a value that is not a record")
  | .call name args =>
    match env.prog.fns.find? (·.name == name) with
    | .some fd => do
      let vs ← evalList fuel env inFn ls args
      let ps := (fd.params.map (·.1)).zip vs
      eval fuel env true ps.reverse fd.body
    | .none =>
      match name, args with
      | "map", [l, .arrow ps body] => do
        let xs ← (← eval fuel env inFn ls l).asList
        let ys ← evalMap fuel env inFn ls ps body xs 0
        .ok (.list ys)
      | "filter", [l, .arrow ps body] => do
        let xs ← (← eval fuel env inFn ls l).asList
        let ys ← evalFilter fuel env inFn ls ps body xs 0
        .ok (.list ys)
      | "pending", [.var x] =>
        -- Answers here are synchronous: a settled resource is never
        -- pending, and an unsettled one is not known yet.
        if env.prog.resources.any (·.name == x) then
          (lookup x env.resources).elim (.error .pending) (fun _ => .ok (.bool false))
        else .ok (.bool false)
      | "failed", [.var x] =>
        -- A source here never fails.
        if env.prog.resources.any (·.name == x) then
          (lookup x env.resources).elim (.error .pending) (fun _ => .ok (.bool false))
        else .ok (.bool false)
      | _, _ => do
        let vs ← evalList fuel env inFn ls args
        stdlib env name vs
  | .record shape base fields => do
    let b ← match base with
      | .none => pure Option.none
      | .some e => do
        match ← eval fuel env inFn ls e with
        | .record _ fs => pure (Option.some fs)
        | _ => .error (.type "record base that is not a record")
    let decl ← (env.shape shape).elim (.error (.type s!"unknown shape `{shape}`")) .ok
    let vs ← evalFields fuel env inFn ls fields b decl.fields 0
    .ok (.record shape vs)
  | .unary .neg e => do
    match ← eval fuel env inFn ls e with
    | .num x => .ok (.num (-x))
    | _ => .error (.type "negation of a value that is not a number")
  | .unary .not e => do
    match ← eval fuel env inFn ls e with
    | .bool b => .ok (.bool !b)
    | _ => .error (.type "`not` of a value that is not a bool")
  | .binary .and a b => do
    match ← eval fuel env inFn ls a with
    | .bool false => .ok (.bool false)
    | .bool true => eval fuel env inFn ls b
    | _ => .error (.type "`and` of a value that is not a bool")
  | .binary .or a b => do
    match ← eval fuel env inFn ls a with
    | .bool true => .ok (.bool true)
    | .bool false => eval fuel env inFn ls b
    | _ => .error (.type "`or` of a value that is not a bool")
  | .binary op a b => do
    let va ← eval fuel env inFn ls a
    let vb ← eval fuel env inFn ls b
    binop op va vb
  | .ternary c a b => do
    match ← eval fuel env inFn ls c with
    | .bool true => eval fuel env inFn ls a
    | .bool false => eval fuel env inFn ls b
    | _ => .error (.type "condition that is not a bool")
  | .matchOpt s x a b => do
    match ← eval fuel env inFn ls s with
    | .some v => eval fuel env inFn ((x, v) :: ls) a
    | .none => eval fuel env inFn ls b
    | _ => .error (.type "match on a value that is not an option")
  | .arrow _ _ => .error (.type "an arrow outside `map` or `filter`")
  | .letE x v body => do
    let w ← eval fuel env inFn ls v
    eval fuel env inFn ((x, w) :: ls) body
  | .named _ _ => .error (.type "a named argument outside a record or command")
  | .typed e _ => eval fuel env inFn ls e

def evalList : Nat → Env → Bool → Locals → List Expr → Result (List Value)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, [] => .ok []
  | fuel + 1, env, inFn, ls, e :: es => do
    let v ← eval fuel env inFn ls e
    let vs ← evalList fuel env inFn ls es
    .ok (v :: vs)

def evalDisplays : Nat → Env → Bool → Locals → List Expr → Result (List String)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, [] => .ok []
  | fuel + 1, env, inFn, ls, e :: es => do
    let s ← (← eval fuel env inFn ls e).display
    let ss ← evalDisplays fuel env inFn ls es
    .ok (s :: ss)

def evalMap : Nat → Env → Bool → Locals → List String → Expr → List Value → Nat → Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, env, inFn, ls, ps, body, x :: xs, i => do
    let y ← eval fuel env inFn (bindParams ps x i ls) body
    let ys ← evalMap fuel env inFn ls ps body xs (i + 1)
    .ok (y :: ys)

def evalFilter : Nat → Env → Bool → Locals → List String → Expr → List Value → Nat → Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, env, inFn, ls, ps, body, x :: xs, i => do
    let keep ← eval fuel env inFn (bindParams ps x i ls) body
    let ys ← evalFilter fuel env inFn ls ps body xs (i + 1)
    match keep with
    | .bool true => .ok (x :: ys)
    | .bool false => .ok ys
    | _ => .error (.type "filter callback that is not a bool")

/-- A record's fields in declaration order: the one written, else the
base's. -/
def evalFields : Nat → Env → Bool → Locals → List (String × Expr) → Option (List Value) →
    List Field → Nat → Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, env, inFn, ls, written, base, f :: fs, i => do
    let v ← match lookupField f.name written with
      | .some e => eval fuel env inFn ls e
      | .none =>
        match base with
        | .some bs => (bs[i]?).elim (.error (.type "short record base")) .ok
        | .none => .error (.type s!"record without field `{f.name}`")
    let vs ← evalFields fuel env inFn ls written base fs (i + 1)
    .ok (v :: vs)

end

end Contract
