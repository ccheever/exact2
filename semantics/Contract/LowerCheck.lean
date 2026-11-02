/-
Translation validation of the real compiler: the Lean half of
`difftest lowering`.

For a program, the Rust compiler's plan supplies the bytecode of every
derive, root slot initializer, resource argument and action, with what
decoding needs (`Vm.Pool`) and where it put each declaration. Here each
body is decoded and run on the VM model against the configurations the
semantics reaches (boot, then after each event of the script), and its
result compared with `eval` (or, for an action, `exec`'s effects) in the
same configuration; and the body `Contract.Lower` compiles from the
semantics' own syntax is compared with the decoded one, instruction for
instruction.
-/
import Contract.Lower
import Contract.Observe
import Contract.OracleText

namespace Contract.LowerCheck

open Vm Lower

inductive Kind where
  | derive
  | slot
  | resource (arg : Nat)
  | action
  deriving Inhabited, BEq

def Kind.text : Kind → String
  | .derive => "derive"
  | .slot => "slot"
  | .resource k => s!"resource-arg{k}"
  | .action => "action"

/-- One body of the plan. -/
structure Body where
  kind : Kind
  name : String
  /-- The code, as hex. -/
  bytes : String
  /-- An action's `writes`. -/
  writes : List Nat := []
  deriving Inhabited

/-- What the difftest reads from the plan. -/
structure PlanInfo where
  strings : Array String := #[]
  types : Array (String × Nat) := #[]
  /-- Every slot: its name and whether a region instance owns it. -/
  slots : List (String × Bool) := []
  derives : List String := []
  resources : List String := []
  /-- Every mutation: its name and slot. -/
  mutations : List (String × Nat) := []
  bodies : List Body := []
  deriving Inhabited

def hexByte (a b : Char) : Option UInt8 := do
  let h ← OracleText.hexVal a
  let l ← OracleText.hexVal b
  pure (UInt8.ofNat (h * 16 + l))

def unhex (s : String) : ByteArray := Id.run do
  let cs := s.toList.toArray
  let mut out := ByteArray.empty
  for i in [0:cs.size / 2] do
    match hexByte cs[2 * i]! cs[2 * i + 1]! with
    | .some b => out := out.push b
    | .none => pure ()
  return out

/-- Values equal bit for bit, records with their shape names. -/
partial def Value.exact : Value → Value → Bool
  | .num a, .num b => Number.canonicalBits a == Number.canonicalBits b
  | .bool a, .bool b => a == b
  | .str a, .str b => a == b
  | .unit, .unit => true
  | .none, .none => true
  | .some a, .some b => Value.exact a b
  | .list xs, .list ys => xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.exact a b
  | .record s xs, .record t ys =>
    s == t && xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.exact a b
  | _, _ => false

def indexOf (x : String) (xs : List String) : Option Nat := xs.findIdx? (· == x)

def layout (p : Program) (pi : PlanInfo) : Layout :=
  let slotNames := pi.slots.map (·.1)
  { slots := p.states.filterMap fun s => (indexOf s.name slotNames).map (s.name, ·)
    derives := p.derives.filterMap fun d => (indexOf d.name pi.derives).map (d.name, ·)
    resources := p.resources.filterMap fun r => (indexOf r.name pi.resources).map (r.name, ·)
    mutations := p.mutations.filterMap fun m =>
      ((pi.mutations.map (·.1)).findIdx? (· == m.name)).bind fun k =>
        (pi.mutations[k]?).map fun (_, s) => (m.name, s, k) }

/-- The VM's environment in a configuration of the semantics: root slots
and mutations from the configuration, no region instance in force (an
owned slot has none), every derive and resource as settled, nothing in
flight. -/
def vmEnv (p : Program) (pi : PlanInfo) (c : Config) (params : List Value) (writes : List Nat) :
    Vm.Env :=
  { slots := pi.slots.map fun (n, owned) =>
      if owned then Option.none
      else if isRootState p n then lookup n c.slots else Option.some .unit
    owned := pi.slots.map (·.2)
    derives := pi.derives.map (lookup · c.settled.derives)
    resources := pi.resources.map (lookup · c.settled.resources)
    params, now := c.now, writable := writes
    mutationSlots := pi.mutations.map (·.2), routes := p.routes,
    strings := p.strings }

def semEnv (p : Program) (c : Config) : Contract.Env :=
  { prog := p, slots := c.slots, derives := c.settled.derives,
    resources := c.settled.resources, now := c.now }

/-- An argument of each type, two ways. -/
partial def sample (p : Program) (k : Nat) : Ty → Value
  | .number => .num (if k == 0 then 0 else F64.ofBits 0x4004000000000000) -- 2.5
  | .bool => .bool (k != 0)
  | .string => .str (if k == 0 then "" else "ab")
  | .unit => .unit
  | .option t => if k == 0 then .none else .some (sample p k t)
  | .list t => if k == 0 then .list [] else .list [sample p 0 t, sample p 1 t]
  | .record s =>
    match p.shapes.find? (·.name == s) with
    | .some sh => .record s (sh.fields.map fun f => sample p k f.ty)
    | .none => .unit
  | .unknown => .unit

def showFx (fx : Vm.Effects) : String :=
  let vs (xs : List Value) := ",".intercalate (xs.map Observe.value)
  s!"writes [{", ".intercalate (fx.writes.map fun (j, v) => s!"{j}={Observe.value v}")}] " ++
  s!"rows [{", ".intercalate (fx.rowWrites.map fun (j, v) => s!"{j}={Observe.value v}")}] " ++
  s!"commands [{", ".intercalate (fx.commands.map fun (n, a) => s!"{n}({vs a})")}] " ++
  s!"sends [{", ".intercalate (fx.sends.map fun (m, s, a) => s!"{m}:{s}({vs a})")}] " ++
  s!"refreshes {fx.refreshes}"

def sameFx (a b : Vm.Effects) : Bool :=
  let vals (xs ys : List Value) := xs.length == ys.length && (xs.zip ys).all fun (x, y) => Value.exact x y
  a.writes.length == b.writes.length &&
    (a.writes.zip b.writes).all (fun ((i, v), (j, w)) => i == j && Value.exact v w) &&
  a.rowWrites.length == b.rowWrites.length &&
    (a.rowWrites.zip b.rowWrites).all (fun ((i, v), (j, w)) => i == j && Value.exact v w) &&
  a.commands.length == b.commands.length &&
    (a.commands.zip b.commands).all (fun ((n, xs), (m, ys)) => n == m && vals xs ys) &&
  a.sends.length == b.sends.length &&
    (a.sends.zip b.sends).all (fun ((i, s, xs), (j, t, ys)) => i == j && s == t && vals xs ys) &&
  a.refreshes == b.refreshes

def trapText : Trap → String
  | .call e => "call: " ++ Observe.why e
  | t => reprStr t

structure Tally where
  agree : Nat := 0
  bothFail : Nat := 0
  diverge : Nat := 0
  same : Nat := 0
  differ : Nat := 0
  refused : Nat := 0
  undecoded : Nat := 0

/-- Compare one run of a body with the semantics' answer. -/
def judge (what : String) (vm : Except Trap (Value × Vm.Effects))
    (sem : Except Err (Value × Vm.Effects)) (t : Tally) : Tally × List String :=
  match vm, sem with
  | .ok (v, fx), .ok (w, gx) =>
    if Value.exact v w && sameFx fx gx then ({ t with agree := t.agree + 1 }, [])
    else ({ t with diverge := t.diverge + 1 },
      [s!"DIVERGE {what}: vm {Observe.value v} {showFx fx} / semantics {Observe.value w} {showFx gx}"])
  | .error e, .error e2 =>
    match e with
    | .outOfFuel => ({ t with diverge := t.diverge + 1 }, [s!"DIVERGE {what}: the vm ran out of fuel"])
    | _ => ({ t with bothFail := t.bothFail + 1 }, [s!"BOTH {what}: vm traps {trapText e} / semantics fails: {Observe.why e2}"])
  | .ok (v, fx), .error e =>
    ({ t with diverge := t.diverge + 1 },
      [s!"DIVERGE {what}: vm {Observe.value v} {showFx fx} / semantics fails: {Observe.why e}"])
  | .error e, .ok (w, gx) =>
    ({ t with diverge := t.diverge + 1 },
      [s!"DIVERGE {what}: vm traps {trapText e} / semantics {Observe.value w} {showFx gx}"])

/-- The first instruction where two bodies differ. -/
def structural (rust lean : Code) : Option String :=
  let rec go : Nat → Code → Code → Option String
    | _, [], [] => .none
    | i, a :: as, b :: bs => if a == b then go (i + 1) as bs
      else .some s!"at {i}: rust {a.text}, lean {b.text}"
    | i, a :: _, [] => .some s!"at {i}: rust {a.text}, lean ends"
    | i, [], b :: _ => .some s!"at {i}: rust ends, lean {b.text}"
  go 0 rust lean

/-- The semantics' expression a body compiles. -/
def exprOf (p : Program) (b : Body) : Option Expr :=
  match b.kind with
  | .derive => (p.derives.find? (·.name == b.name)).map (·.body)
  | .slot => (p.states.find? (·.name == b.name)).map (·.init)
  | .resource k => (p.resources.find? (·.name == b.name)).bind (·.args[k]?)
  | .action => .none

def check (p : Program) (o : Oracle) (events : List Observe.Event) (pi : PlanInfo)
    (roster : Array (String × Nat)) : List String := Id.run do
  let pool : Pool := { strings := pi.strings, types := pi.types, roster }
  let L := layout p pi
  let mut t : Tally := {}
  let mut out : Array String := #[]
  -- The configurations to check in: boot, and after each event that is not
  -- poisoned. A refused observed step can still have committed the work due
  -- before its event, or the event after a refused advance (`dispatchAt`), so
  -- the run goes on from what it returned; `observe_step_sound` makes it reachable.
  let (c0, o0) := boot p o
  let mut cfgs : Array Config := #[]
  match o0 with
  | .ok => cfgs := cfgs.push c0
  | _ => out := out.push "# boot refused"
  let mut c := c0
  if !cfgs.isEmpty then
    for e in events do
      let (c', r) := Observe.step p o c e
      match r with
      | .ok => cfgs := cfgs.push c'; c := c'
      | .refused _ => cfgs := cfgs.push c'; c := c'
      | .poisoned _ => break
  for b in pi.bodies do
    let what := s!"{b.kind.text} {b.name}"
    match decode pool (unhex b.bytes) with
    | .error why =>
      t := { t with undecoded := t.undecoded + 1 }
      out := out.push s!"UNDECODED {what}: {why}"
    | .ok code =>
      -- The compiler of `Contract.Lower` against the Rust compiler's code.
      let lean : Except String Code := match b.kind with
        | .action =>
          match p.actions.find? (·.name == b.name) with
          | .some a => compileAction p L a
          | .none => .error "no such action"
        | _ =>
          match exprOf p b with
          | .some e => compileBody p L e
          | .none => .error "no such declaration"
      match lean with
      | .error why =>
        t := { t with refused := t.refused + 1 }
        out := out.push s!"REFUSED {what}: {why}"
      | .ok lc =>
        match structural code lc with
        | .none => t := { t with same := t.same + 1 }
        | .some d =>
          t := { t with differ := t.differ + 1 }
          out := out.push s!"STRUCT {what} {d}"
      -- The decoded code on the VM against the semantics.
      for (cfg, k) in cfgs.toList.zipIdx do
        let env := semEnv p cfg
        match b.kind with
        | .action =>
          match p.actions.find? (·.name == b.name) with
          | .none => pure ()
          | .some a =>
            for s in [0, 1] do
              let args := a.params.map fun (_, ty) => sample p s ty
              -- An action runs in a row instance of every region: each
              -- owned slot holds a value of its type, on both sides.
              let rows := p.states.filterMap fun st =>
                if st.owner.isSome then Option.some (st.name, sample p s st.ty) else Option.none
              let venv := vmEnv p pi cfg args b.writes
              let slots := (pi.slots.zip venv.slots).map fun ((n, owned), v) =>
                if owned then lookup n rows else v
              let venv := { venv with slots }
              let vm := run code venv
              let ls : Locals := ((a.params.map (·.1)).zip args).reverse
              let sem := (exec Contract.fuel { env with rows } ls a.body {}).map fun fx => (Value.unit, lowerFx L fx)
              let (t', ls') := judge s!"{what} cfg{k} args{s}" vm sem t
              t := t'
              out := out ++ ls'.toArray
        | _ =>
          match exprOf p b with
          | .none => pure ()
          | .some e =>
            let vm := run code (vmEnv p pi cfg [] [])
            let sem := (eval Contract.fuel env false [] e).map fun v => (v, ({} : Vm.Effects))
            let (t', ls') := judge s!"{what} cfg{k}" vm sem t
            t := t'
            out := out ++ ls'.toArray
  out := out.push s!"sum {t.agree} {t.bothFail} {t.diverge} {t.same} {t.differ} {t.refused} {t.undecoded}"
  return out.toList

/-- The plan crate's opcode table against `Vm.opcodes`. -/
def checkOpcodes (rust : List (String × List Operand)) : List String :=
  if rust == Vm.opcodes then [] else
  ["OPCODES the plan's table differs from Vm.opcodes"] ++
  ((rust.zip Vm.opcodes).zipIdx.filterMap fun (((a, x), (b, y)), i) =>
    if a == b && x == y then Option.none else Option.some s!"OPCODES {i}: plan {a} {repr x}, lean {b} {repr y}") ++
  (if rust.length != Vm.opcodes.length then [s!"OPCODES plan has {rust.length}, lean {Vm.opcodes.length}"] else [])

end Contract.LowerCheck
