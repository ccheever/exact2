/-
What a differential run compares: a canonical text of a configuration.

The Rust side (`semantics/difftest/src/observe.rs`) prints the runner's
state in exactly this form; the two texts are compared line by line.

  `== <step>`              the step that produced what follows
  `outcome ok|refused|poisoned`
  `slot <name> <value>`    each root slot, declaration order, mutations last
  `derive <name> <value>`  each derive, declaration order
  `resource <name> <value>`
  `queued <name> <count>`   each `queue` mutation's waiting sends
  `command <name> <value>…` each command the step's commit issued
  `view <testId> <text>`   each element with a `testId`, in preorder, but
                           none inside a virtualized `list` or a literal
                           `role="tabpanel"`: the runner builds only the
                           rows a list's window lays out and the routes of
                           selected tabs, the semantics all of them

Values: a number is `n` and its 16 hex digits of IEEE bits (every NaN is
`7ff8000000000000`); a string is quoted with `\"`, `\\`, `\n`, `\r`, `\t`
and `\u00xx` for the other controls; `true`, `false`, `()`, `none`,
`some(v)`, `[v,…]` for a list and `{v,…}` for a record.
-/
import Contract.Runtime

namespace Contract.Observe

def hex16 (n : UInt64) : String :=
  let digits := "0123456789abcdef".toList
  String.ofList ((List.range 16).reverse.map fun i =>
    digits.getD ((n >>> (UInt64.ofNat (4 * i))) &&& 0xf).toNat '0')

def quote (s : String) : String :=
  let body := s.foldl (fun acc c =>
    match c with
    | '"' => acc ++ "\\\""
    | '\\' => acc ++ "\\\\"
    | '\n' => acc ++ "\\n"
    | '\r' => acc ++ "\\r"
    | '\t' => acc ++ "\\t"
    | c => if c.toNat < 0x20 then
        acc ++ "\\u00" ++ String.ofList [ "0123456789abcdef".toList.getD (c.toNat / 16) '0',
                                         "0123456789abcdef".toList.getD (c.toNat % 16) '0']
      else acc.push c) ""
  "\"" ++ body ++ "\""

partial def value : Value → String
  | .num f => "n" ++ hex16 (Number.canonicalBits f)
  | .bool b => if b then "true" else "false"
  | .str s => quote s
  | .unit => "()"
  | .none => "none"
  | .some v => "some(" ++ value v ++ ")"
  | .list xs => "[" ++ ",".intercalate (xs.map value) ++ "]"
  | .record _ xs => "{" ++ ",".intercalate (xs.map value) ++ "}"

partial def viewLines : List VNode → List String
  | [] => []
  | n :: rest =>
    let here := match n.testId with
      | .some id => ["view " ++ quote id ++ " " ++ (n.text.map quote).getD "-"]
      | .none => []
    here ++ (if n.windowed then [] else viewLines n.children) ++ viewLines rest

def why : Err → String
  | .type w => "type: " ++ w
  | .pending => "pending"
  | .unbound x => "unbound: " ++ x
  | .unsupported w => "unsupported: " ++ w
  | .refused w => "refused: " ++ w

/-- The outcome line; a refusal's reason follows on a `#` line, which a
comparison skips (the runner names its reasons differently). -/
def outcome : Outcome → String
  | .ok => "outcome ok"
  | .refused e => "outcome refused\n# " ++ why e
  | .poisoned e => "outcome poisoned\n# " ++ why e

/-- The lines for one step: `label`, the outcome, then (unless poisoned)
the state, the commands issued since `commandsBefore`, and the view. -/
def lines (p : Program) (label : String) (c : Config) (out : Outcome) (commandsBefore : Nat) :
    List String :=
  let head := ["== " ++ label, outcome out]
  match out with
  | .poisoned _ => head
  | _ =>
    if c.poisoned then head else
    head ++
    ((c.slots.filter fun (x, _) => p.locale != .some x).map fun (x, v) => "slot " ++ x ++ " " ++ value v) ++
    (p.derives.filterMap fun d => (lookup d.name c.settled.derives).map fun v =>
      "derive " ++ d.name ++ " " ++ value v) ++
    (p.resources.filterMap fun r => (lookup r.name c.settled.resources).map fun v =>
      "resource " ++ r.name ++ " " ++ value v) ++
    ((p.mutations.filter (·.queue)).map fun m =>
      "queued " ++ m.name ++ " " ++ toString ((c.queued.filter (·.1 == m.name)).length)) ++
    ((c.commands.drop commandsBefore).map fun (n, vs) =>
      "command " ++ n ++ String.join (vs.map (" " ++ value ·))) ++
    viewLines c.view

/-- A host event, as a script names it. -/
inductive Event where
  | tap (target : String)
  | change (target : String) (text : String)
  | clock (ms : F64)
  /-- Any other event with an optional payload (`hover`, `key`, …). -/
  | other (target event : String) (payload : Option Value)
  deriving Inhabited

def Event.label : Event → String
  | .tap t => "tap " ++ quote t
  | .change t s => "type " ++ quote t ++ " " ++ quote s
  | .clock ms => "clock +" ++ Number.jsToString ms
  | .other t e _ => e ++ " " ++ quote t

def step (p : Program) (o : Oracle) (c : Config) : Event → Config × Outcome
  | .tap t => dispatch p o c t "press" .none
  | .change t s => dispatch p o c t "change" (.some (.str s))
  | .clock ms => advance p o c (c.now + ms)
  | .other t e v => dispatch p o c t e v

/-- Boot, then every event: the whole observation text. Stops after a
poisoned outcome (a poisoned runner refuses everything after). -/
def run (p : Program) (o : Oracle) (events : List Event) : List String := Id.run do
  let (c, out) := boot p o
  match out with
  | .ok => pure ()
  | _ => return ["== boot", outcome out]
  let mut acc := lines p "boot" c out 0
  let mut c := c
  for e in events do
    let before := c.commands.length
    let (c', out) := step p o c e
    acc := acc ++ lines p e.label c' out before
    c := c'
    match out with
    | .poisoned _ => return acc
    | _ => pure ()
  return acc

end Contract.Observe
