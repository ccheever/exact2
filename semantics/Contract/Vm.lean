/-
The expression VM: a model of `runner/src/vm.rs` for the opcodes the
compiler emits for expressions and action bodies, and a decoder from the
plan's byte encoding (`plan/tables/format.json`, `opcodes`) into it.

The model runs over a list of instructions with symbolic operands: slot,
derive, resource and mutation indices stay indices, a string operand is the
pool's string, a `Record`'s type is its shape's name and field count, a
`Call`'s roster entry is its name and arity, and a jump's byte target is a
forward offset in instructions (`Plan::check_code` admits only forward,
instruction-aligned jumps; the decoder checks the same). Each step is one
iteration of the runner's loop: a callback body's end (collect what the run
left, start the next item or finish the list), else the instruction at `pc`.

What the model leaves out are the runner's resource bounds (string length,
value size and depth, list steps): a program that hits one traps on the
runner and not here, as in `Contract.Eval`. The roster is `Contract.stdlib`,
the semantics' own, so a `Call` means what `eval` means by the entry.
-/
import Contract.Eval

namespace Contract.Vm

/-- One instruction. `jump off` and its kin go to `pc + 1 + off`; a `map`
or `filter` callback body is the `off` instructions after it. -/
inductive Instr where
  | num (bits : UInt64)
  | bool (b : Bool)
  | str (s : String)
  | none
  | unit
  | some
  | loadSlot (i : Nat)
  | loadDerive (i : Nat)
  | loadResource (i : Nat)
  | loadParam (i : Nat)
  | loadItem (depth : Nat)
  | loadIndex (depth : Nat)
  | loadBound (depth : Nat)
  | field (i : Nat)
  /-- A record of the shape the type names, of its `n` fields. -/
  | record (shape : String) (n : Nat)
  | list (n : Nat)
  | add | sub | mul | div | rem | neg
  | eq | ne | lt | le | gt | ge
  | not | concat
  | jump (off : Nat)
  | jumpIfFalse (off : Nat)
  | jumpIfNone (off : Nat)
  | unwrap
  /-- A roster entry and the arity the roster gives it. -/
  | call (f : String) (arity : Nat)
  | storeSlot (i : Nat)
  | command (name : String) (n : Nat)
  | pop
  | bindLocal
  | loadLocal (i : Nat)
  | dropLocal
  | ret
  | send (m : Nat) (source : String) (n : Nat)
  | refresh (r : Nat)
  | pendingResource (r : Nat)
  | pendingMutation (m : Nat)
  | failedResource (r : Nat)
  /-- Not modelled (a native module's props): traps as unsupported. -/
  | nativeProps (n : Nat)
  | map (off : Nat)
  | filter (off : Nat)
  deriving Repr, BEq, Inhabited, DecidableEq

abbrev Code := List Instr

/-- Why a run stopped short of `Return`: the runner's `Trap`s that the
model can reach. -/
inductive Trap where
  | stackUnderflow
  | typeMismatch (op : String)
  | unwrapNone
  | badField
  | badScope
  | badParam
  | arity
  | writeNotDeclared
  | badJump
  | malformed
  | noResult
  | pending
  /-- The roster entry refused its arguments, or is one the semantics
  leaves out. -/
  | call (why : Err)
  | unsupported (what : String)
  | outOfFuel
  deriving Repr, Inhabited

/-- One instance scope (an `each` row, a `match` arm). -/
structure Frame where
  item : Option Value := .none
  index : Option Nat := .none
  bound : Option Value := .none
  deriving Inhabited

/-- Everything a body may read, by index, as the runner's `Env` holds it. -/
structure Env where
  /-- Slots by index. `none`: an owned slot with no instance in force. -/
  slots : List (Option Value) := []
  /-- Whether each slot belongs to a region instance (a write to it is a
  row write). -/
  owned : List Bool := []
  /-- Derives by index; `none` while not yet settled. -/
  derives : List (Option Value) := []
  resources : List (Option Value) := []
  params : List Value := []
  /-- Enclosing instance scopes, innermost first. -/
  frames : List Frame := []
  now : F64 := 0
  /-- The slots `StoreSlot` and `Send` may write (an action's `writes`). -/
  writable : List Nat := []
  /-- Each mutation's slot. -/
  mutationSlots : List Nat := []
  pendingResources : List Bool := []
  failedResources : List Bool := []
  pendingMutations : List Bool := []
  /-- The route table the router verbs read (the runner's `Routing`). -/
  routes : Route.Table := []
  /-- The strings tables `t` reads (the plan's `locales`). -/
  strings : Format.Tables := []
  deriving Inhabited

/-- What a body asks for, in execution order. -/
structure Effects where
  writes : List (Nat × Value) := []
  rowWrites : List (Nat × Value) := []
  commands : List (String × List Value) := []
  sends : List (Nat × String × List Value) := []
  refreshes : List Nat := []
  deriving Inhabited

/-- A `Map` or `Filter` in progress: its body is `start ..< stop`; `cur`
is the item being run, at index `idx`; `rest` the items after it; `out`
what the runs so far kept; `base` the locals depth at the opcode; `caller`
the operand stack it set aside. -/
structure Callback where
  filter : Bool
  start : Nat
  stop : Nat
  cur : Value
  idx : Nat
  rest : List Value
  out : List Value
  base : Nat
  caller : List Value
  deriving Inhabited

structure Machine where
  pc : Nat
  /-- The operand stack, top first. -/
  stack : List Value
  /-- The locals stack, bottom first: `LoadLocal i` reads `locals[i]`. -/
  locals : List Value
  /-- Callbacks in progress, innermost first. -/
  cbs : List Callback
  fx : Effects
  deriving Inhabited

inductive Status where
  | run (m : Machine)
  | done (v : Value) (fx : Effects)
  deriving Inhabited

abbrev Out := Except Trap Status

/-- The semantics' environment for a roster call: only the clock and the
route table are read. -/
def callEnv (env : Env) : Contract.Env :=
  { prog := { routes := env.routes, strings := env.strings }, slots := [], now := env.now }

/-- Two numbers to one value (`Add`, `Lt`, …). -/
def num2 (f : F64 → F64 → Value) (op : String) : Value → Value → Except Trap Value
  | .num x, .num y => .ok (f x y)
  | _, _ => .error (.typeMismatch op)

/-- A binary opcode on its operands. -/
def binary (i : Instr) (a b : Value) : Except Trap Value :=
  match i with
  | .add => num2 (fun x y => .num (x + y)) "Add" a b
  | .sub => num2 (fun x y => .num (x - y)) "Sub" a b
  | .mul => num2 (fun x y => .num (x * y)) "Mul" a b
  | .div => num2 (fun x y => .num (x / y)) "Div" a b
  | .rem => num2 (fun x y => .num (Number.fmod x y)) "Rem" a b
  -- Two strings by UTF-16 code units (LLP 1088 D1), as `binop`.
  | .lt => match a, b with | .str x, .str y => .ok (.bool (Str.lt x y)) | _, _ => num2 (fun x y => .bool (x < y)) "Lt" a b
  | .le => match a, b with | .str x, .str y => .ok (.bool !(Str.lt y x)) | _, _ => num2 (fun x y => .bool (x ≤ y)) "Le" a b
  | .gt => match a, b with | .str x, .str y => .ok (.bool (Str.lt y x)) | _, _ => num2 (fun x y => .bool (x > y)) "Gt" a b
  | .ge => match a, b with | .str x, .str y => .ok (.bool !(Str.lt x y)) | _, _ => num2 (fun x y => .bool (x ≥ y)) "Ge" a b
  | .eq => (Value.equal a b).elim (.error (.typeMismatch "Eq")) (fun e => .ok (.bool e))
  | .ne => (Value.equal a b).elim (.error (.typeMismatch "Ne")) (fun e => .ok (.bool !e))
  | .concat =>
    match a, b with
    | .str x, .str y => .ok (.str (x ++ y))
    | _, _ => .error (.typeMismatch "Concat")
  | _ => .error .malformed

def isBinary : Instr → Bool
  | .add | .sub | .mul | .div | .rem | .lt | .le | .gt | .ge | .eq | .ne | .concat => true
  | _ => false

/-- A jump's target, refused past the end of the code or out of the
callback body it is in. -/
def jumpTo (size : Nat) (cbs : List Callback) (target : Nat) : Except Trap Nat :=
  match cbs with
  | c :: _ => if target > c.stop then .error .badJump
    else if target > size then .error .badJump else .ok target
  | [] => if target > size then .error .badJump else .ok target

/-- Pop one. -/
def pop1 : List Value → Except Trap (Value × List Value)
  | v :: s => .ok (v, s)
  | [] => .error .stackUnderflow

/-- Pop two: the top, then the one below it. -/
def pop2 : List Value → Except Trap (Value × Value × List Value)
  | b :: a :: s => .ok (b, a, s)
  | _ => .error .stackUnderflow

/-- The top `n`, in push order, and what is below them. -/
def popN (n : Nat) (s : List Value) : Option (List Value × List Value) :=
  if s.length < n then Option.none else Option.some ((s.take n).reverse, s.drop n)

/-- Run the instruction `i` at `m.pc` (in code of `size` instructions). -/
def exec (size : Nat) (env : Env) (i : Instr) (m : Machine) : Out :=
  let next (s : List Value) : Out := .ok (.run { m with pc := m.pc + 1, stack := s })
  let push (v : Value) : Out := next (v :: m.stack)
  match i with
  | .num b => push (.num (F64.ofBits b))
  | .bool b => push (.bool b)
  | .str s => push (.str s)
  | .none => push .none
  | .unit => push .unit
  | .some => do let (v, s) ← pop1 m.stack; next (.some v :: s)
  | .loadSlot j =>
    match env.slots[j]? with
    | Option.none => .error .malformed
    | Option.some Option.none => .error .badScope
    | Option.some (Option.some v) => push v
  | .loadDerive j =>
    match env.derives[j]? with
    | Option.none => .error .malformed
    | Option.some Option.none => .error .pending
    | Option.some (Option.some v) => push v
  | .loadResource j =>
    match env.resources[j]? with
    | Option.none => .error .malformed
    | Option.some Option.none => .error .pending
    | Option.some (Option.some v) => push v
  | .loadParam j =>
    match env.params[j]? with
    | Option.none => .error .badParam
    | Option.some v => push v
  | .loadItem d =>
    match env.frames[d]?.bind (·.item) with
    | Option.none => .error .badScope
    | Option.some v => push v
  | .loadIndex d =>
    match env.frames[d]?.bind (·.index) with
    | Option.none => .error .badScope
    | Option.some k => push (.num (F64.ofNat k))
  | .loadBound d =>
    match env.frames[d]?.bind (·.bound) with
    | Option.none => .error .badScope
    | Option.some v => push v
  | .field j => do
    let (v, s) ← pop1 m.stack
    match v with
    | .record _ fs =>
      match fs[j]? with
      | Option.some w => next (w :: s)
      | Option.none => .error .badField
    | _ => .error (.typeMismatch "Field")
  | .record shape n =>
    match popN n m.stack with
    | Option.some (fs, s) => next (.record shape fs :: s)
    | Option.none => .error .stackUnderflow
  | .list n =>
    match popN n m.stack with
    | Option.some (xs, s) => next (.list xs :: s)
    | Option.none => .error .stackUnderflow
  | .neg => do
    let (v, s) ← pop1 m.stack
    match v with
    | .num x => next (.num (-x) :: s)
    | _ => .error (.typeMismatch "Neg")
  | .not => do
    let (v, s) ← pop1 m.stack
    match v with
    | .bool b => next (.bool (!b) :: s)
    | _ => .error (.typeMismatch "Not")
  | .jump off => do
    let t ← jumpTo size m.cbs (m.pc + 1 + off)
    .ok (.run { m with pc := t })
  | .jumpIfFalse off => do
    let (v, s) ← pop1 m.stack
    match v with
    | .bool true => next s
    | .bool false => do
      let t ← jumpTo size m.cbs (m.pc + 1 + off)
      .ok (.run { m with pc := t, stack := s })
    | _ => .error (.typeMismatch "JumpIfFalse")
  | .jumpIfNone off =>
    match m.stack with
    | .some _ :: _ => next m.stack
    | .none :: _ => do
      let t ← jumpTo size m.cbs (m.pc + 1 + off)
      .ok (.run { m with pc := t })
    | _ :: _ => .error (.typeMismatch "JumpIfNone")
    | [] => .error .stackUnderflow
  | .unwrap => do
    let (v, s) ← pop1 m.stack
    match v with
    | .some w => next (w :: s)
    | .none => .error .unwrapNone
    | _ => .error (.typeMismatch "Unwrap")
  | .call f n =>
    match popN n m.stack with
    | Option.none => .error .arity
    | Option.some (args, s) =>
      match stdlib (callEnv env) f args with
      | .ok v => next (v :: s)
      | .error e => .error (.call e)
  | .storeSlot j =>
    if !env.writable.contains j then .error .writeNotDeclared else do
    let (v, s) ← pop1 m.stack
    match env.owned[j]? with
    | Option.none => .error .malformed
    | Option.some false =>
      .ok (.run { m with pc := m.pc + 1, stack := s,
                         fx := { m.fx with writes := m.fx.writes ++ [(j, v)] } })
    | Option.some true =>
      match env.slots[j]? with
      | Option.some (Option.some _) =>
        .ok (.run { m with pc := m.pc + 1, stack := s,
                           fx := { m.fx with rowWrites := m.fx.rowWrites ++ [(j, v)] } })
      | _ => .error .badScope
  | .command name n =>
    match popN n m.stack with
    | Option.none => .error .stackUnderflow
    | Option.some (args, s) =>
      .ok (.run { m with pc := m.pc + 1, stack := s,
                         fx := { m.fx with commands := m.fx.commands ++ [(name, args)] } })
  | .send k source n =>
    match env.mutationSlots[k]? with
    | Option.none => .error .malformed
    | Option.some slot =>
      if !env.writable.contains slot then .error .writeNotDeclared else
      match popN n m.stack with
      | Option.none => .error .stackUnderflow
      | Option.some (args, s) =>
        .ok (.run { m with pc := m.pc + 1, stack := s,
                           fx := { m.fx with sends := m.fx.sends ++ [(k, source, args)] } })
  | .refresh r =>
    .ok (.run { m with pc := m.pc + 1, fx := { m.fx with refreshes := m.fx.refreshes ++ [r] } })
  | .pendingResource r =>
    match env.resources[r]? with
    | Option.some (Option.some _) => push (.bool (env.pendingResources[r]?.getD false))
    | _ => .error .pending
  | .failedResource r =>
    match env.resources[r]? with
    | Option.some (Option.some _) => push (.bool (env.failedResources[r]?.getD false))
    | _ => .error .pending
  | .pendingMutation k => push (.bool (env.pendingMutations[k]?.getD false))
  | .pop => do let (_, s) ← pop1 m.stack; next s
  | .bindLocal => do
    let (v, s) ← pop1 m.stack
    .ok (.run { m with pc := m.pc + 1, stack := s, locals := m.locals ++ [v] })
  | .loadLocal j =>
    match m.locals[j]? with
    | Option.some v => push v
    | Option.none => .error .badScope
  | .dropLocal =>
    match m.cbs with
    | c :: _ =>
      if m.locals.length ≤ c.base + 2 then .error .malformed
      else .ok (.run { m with pc := m.pc + 1, locals := m.locals.dropLast })
    | [] =>
      if m.locals.isEmpty then .error .stackUnderflow
      else .ok (.run { m with pc := m.pc + 1, locals := m.locals.dropLast })
  | .ret =>
    if !m.cbs.isEmpty then .error .malformed
    else .ok (.done (m.stack.head?.getD .unit) m.fx)
  | .nativeProps _ => .error (.unsupported "NativeProps")
  | .map off | .filter off =>
    let stop := m.pc + 1 + off
    if stop > size then .error .badJump else do
    let (v, s) ← pop1 m.stack
    match v with
    | .list [] => do
      let t ← jumpTo size m.cbs stop
      .ok (.run { m with pc := t, stack := .list [] :: s })
    | .list (x :: rest) =>
      .ok (.run { m with
        pc := m.pc + 1, stack := [], locals := m.locals ++ [x, .num (F64.ofNat 0)],
        cbs := { filter := (match i with | .filter _ => true | _ => false),
                 start := m.pc + 1, stop, cur := x, idx := 0, rest, out := [],
                 base := m.locals.length, caller := s } :: m.cbs })
    | _ => .error (.typeMismatch (match i with | .filter _ => "Filter" | _ => "Map"))
  | .add | .sub | .mul | .div | .rem | .lt | .le | .gt | .ge | .eq | .ne | .concat => do
    let (b, a, s) ← pop2 m.stack
    let v ← binary i a b
    next (v :: s)

/-- A callback body's end: one value left and every local it bound
dropped; keep the value (`map`) or the item (`filter`, on `true`), then
run the next item or push the list. -/
def bodyEnd (c : Callback) (cbs : List Callback) (m : Machine) : Out :=
  match m.stack with
  | [v] =>
    if m.locals.length ≠ c.base + 2 then .error .malformed else do
    let kept ← if c.filter then
        match v with
        | .bool true => pure [c.cur]
        | .bool false => pure []
        | _ => .error (.typeMismatch "Filter")
      else pure [v]
    let out := c.out ++ kept
    let locals := m.locals.take c.base
    match c.rest with
    | x :: rest =>
      .ok (.run { m with pc := c.start, stack := [],
                         locals := locals ++ [x, .num (F64.ofNat (c.idx + 1))],
                         cbs := { c with cur := x, idx := c.idx + 1, rest, out } :: cbs })
    | [] => .ok (.run { m with pc := c.stop, stack := .list out :: c.caller, locals, cbs })
  | _ => .error .malformed

/-- One iteration of the runner's loop. -/
def step (code : Code) (env : Env) (m : Machine) : Out :=
  match m.cbs with
  | c :: cbs => if c.stop = m.pc then bodyEnd c cbs m else
    match code[m.pc]? with
    | Option.some i => exec code.length env i m
    | Option.none => .error .noResult
  | [] =>
    match code[m.pc]? with
    | Option.some i => exec code.length env i m
    | Option.none => .error .noResult

def Machine.init : Machine := { pc := 0, stack := [], locals := [], cbs := [], fx := {} }

/-- Run from `m` for at most `fuel` steps. -/
def runFrom (code : Code) (env : Env) : Nat → Machine → Except Trap (Value × Effects)
  | 0, _ => .error .outOfFuel
  | n + 1, m =>
    match step code env m with
    | .error t => .error t
    | .ok (.done v fx) => .ok (v, fx)
    | .ok (.run m') => runFrom code env n m'

/-- Evaluate a body: what `vm::eval` returns. -/
def run (code : Code) (env : Env) (fuel : Nat := 1 <<< 22) : Except Trap (Value × Effects) :=
  runFrom code env fuel .init

/-! ## The byte encoding -/

/-- An operand layout, as `format.json` spells it. -/
inductive Operand where
  | u8 | u16 | u32 | f64 | str | enum | idx
  deriving Repr, BEq, DecidableEq

/-- The opcodes in wire order with their operand layouts
(`plan/tables/format.json`, `opcodes`; the differential run checks this
table against the plan crate's). -/
def opcodes : List (String × List Operand) :=
  [("Number", [.f64]), ("Bool", [.u8]), ("Str", [.str]), ("None", []), ("Unit", []),
   ("Some", []), ("LoadSlot", [.idx]), ("LoadDerive", [.idx]), ("LoadResource", [.idx]),
   ("LoadParam", [.u16]), ("LoadItem", [.u16]), ("LoadBound", [.u16]), ("Field", [.u16]),
   ("Record", [.idx]), ("List", [.u32]), ("Add", []), ("Sub", []), ("Mul", []), ("Div", []),
   ("Rem", []), ("Neg", []), ("Eq", []), ("Ne", []), ("Lt", []), ("Le", []), ("Gt", []),
   ("Ge", []), ("Not", []), ("Concat", []), ("Jump", [.u32]), ("JumpIfFalse", [.u32]),
   ("JumpIfNone", [.u32]), ("Unwrap", []), ("Call", [.enum]), ("StoreSlot", [.idx]),
   ("Command", [.str, .u16]), ("Pop", []), ("BindLocal", []), ("LoadLocal", [.u16]),
   ("DropLocal", []), ("Return", []), ("Send", [.idx, .str, .u16]), ("Refresh", [.idx]),
   ("PendingResource", [.idx]), ("PendingMutation", [.idx]), ("FailedResource", [.idx]),
   ("NativeProps", [.u32]), ("LoadIndex", [.u16]), ("Map", [.u32]), ("Filter", [.u32])]

/-- What decoding needs from the plan besides the bytes: its string pool,
each type's (name, field count), and the roster in wire order with each
entry's arity. -/
structure Pool where
  strings : Array String := #[]
  types : Array (String × Nat) := #[]
  roster : Array (String × Nat) := #[]
  deriving Inhabited

/-- An instruction as it sits in the bytes: jump targets still byte
offsets. -/
structure Raw where
  op : String
  args : List Nat
  number : UInt64
  deriving Inhabited

def le (bs : ByteArray) (at_ n : Nat) : Nat :=
  (List.range n).foldr (fun i acc => acc * 256 + (bs.get! (at_ + i)).toNat) 0

/-- Split the bytes into raw instructions with their byte offsets. -/
def frame (bs : ByteArray) : Except String (List (Nat × Raw)) := Id.run do
  let mut pos := 0
  let mut out : Array (Nat × Raw) := #[]
  for _ in [0:bs.size] do
    if pos ≥ bs.size then break
    let pc := pos
    let b := (bs.get! pos).toNat
    let some (name, layout) := opcodes[b]? | return .error s!"unknown opcode {b} at {pc}"
    pos := pos + 1
    let mut args : List Nat := []
    let mut number : UInt64 := 0
    for o in layout do
      let w := match o with
        | .u8 | .enum => 1 | .u16 => 2 | .u32 | .str | .idx => 4 | .f64 => 8
      if pos + w > bs.size then return .error s!"truncated at {pc}"
      let v := le bs pos w
      if o == .f64 then number := UInt64.ofNat v else args := args ++ [v]
      pos := pos + w
    out := out.push (pc, { op := name, args, number })
  return .ok out.toList

/-- Decode a code body into instructions, checking what `check_code`
checks of what the model reads: framing, the pool operands, forward
instruction-aligned jumps, and the final `Return`. -/
def decode (pool : Pool) (bs : ByteArray) : Except String Code := do
  let raws ← frame bs
  let offsets := raws.map (·.1) ++ [bs.size]
  let index (pc target : Nat) : Except String Nat :=
    match offsets.findIdx? (· == target) with
    | Option.some k => if target > pc then .ok k else .error s!"backward jump at {pc}"
    | Option.none => .error s!"jump into an instruction at {pc}"
  let mut out : Array Instr := #[]
  for (pc, r) in raws do
    let k := out.size
    let a0 := r.args.headD 0
    let a1 := r.args.getD 1 0
    let a2 := r.args.getD 2 0
    let str (n : Nat) : Except String String :=
      (pool.strings[n]?).elim (.error s!"string {n} out of range at {pc}") .ok
    let off (target : Nat) : Except String Nat := do
      let t ← index pc target
      .ok (t - (k + 1))
    let i ← match r.op with
      | "Number" =>
        if Number.isFinite (F64.ofBits r.number) then pure (Instr.num r.number)
        else .error s!"a number that is not finite at {pc}"
      | "Bool" => pure (.bool (a0 != 0))
      | "Str" => do pure (.str (← str a0))
      | "None" => pure .none
      | "Unit" => pure .unit
      | "Some" => pure .some
      | "LoadSlot" => pure (.loadSlot a0)
      | "LoadDerive" => pure (.loadDerive a0)
      | "LoadResource" => pure (.loadResource a0)
      | "LoadParam" => pure (.loadParam a0)
      | "LoadItem" => pure (.loadItem a0)
      | "LoadIndex" => pure (.loadIndex a0)
      | "LoadBound" => pure (.loadBound a0)
      | "Field" => pure (.field a0)
      | "Record" =>
        match pool.types[a0]? with
        | Option.some (name, n) => pure (.record name n)
        | Option.none => .error s!"type {a0} out of range at {pc}"
      | "List" => pure (.list a0)
      | "Add" => pure .add | "Sub" => pure .sub | "Mul" => pure .mul | "Div" => pure .div
      | "Rem" => pure .rem | "Neg" => pure .neg | "Eq" => pure .eq | "Ne" => pure .ne
      | "Lt" => pure .lt | "Le" => pure .le | "Gt" => pure .gt | "Ge" => pure .ge
      | "Not" => pure .not | "Concat" => pure .concat
      | "Jump" => do pure (.jump (← off a0))
      | "JumpIfFalse" => do pure (.jumpIfFalse (← off a0))
      | "JumpIfNone" => do pure (.jumpIfNone (← off a0))
      | "Unwrap" => pure .unwrap
      | "Call" =>
        match pool.roster[a0]? with
        | Option.some (f, n) => pure (.call f n)
        | Option.none => .error s!"roster entry {a0} out of range at {pc}"
      | "StoreSlot" => pure (.storeSlot a0)
      | "Command" => do pure (.command (← str a0) a1)
      | "Pop" => pure .pop
      | "BindLocal" => pure .bindLocal
      | "LoadLocal" => pure (.loadLocal a0)
      | "DropLocal" => pure .dropLocal
      | "Return" => pure .ret
      | "Send" => do pure (.send a0 (← str a1) a2)
      | "Refresh" => pure (.refresh a0)
      | "PendingResource" => pure (.pendingResource a0)
      | "PendingMutation" => pure (.pendingMutation a0)
      | "FailedResource" => pure (.failedResource a0)
      | "NativeProps" => pure (.nativeProps a0)
      | "Map" => do pure (.map (← off a0))
      | "Filter" => do pure (.filter (← off a0))
      | op => .error s!"opcode {op} at {pc}"
    out := out.push i
  if out.back? != Option.some .ret then .error "the body does not end with Return"
  .ok out.toList

/-! ## Printing -/

def Instr.text : Instr → String
  | .num b => s!"Number {Number.jsToString (F64.ofBits b)}"
  | .bool b => s!"Bool {b}"
  | .str s => s!"Str {repr s}"
  | .none => "None" | .unit => "Unit" | .some => "Some"
  | .loadSlot i => s!"LoadSlot {i}" | .loadDerive i => s!"LoadDerive {i}"
  | .loadResource i => s!"LoadResource {i}" | .loadParam i => s!"LoadParam {i}"
  | .loadItem d => s!"LoadItem {d}" | .loadIndex d => s!"LoadIndex {d}"
  | .loadBound d => s!"LoadBound {d}" | .field i => s!"Field {i}"
  | .record s n => s!"Record {s}/{n}" | .list n => s!"List {n}"
  | .add => "Add" | .sub => "Sub" | .mul => "Mul" | .div => "Div" | .rem => "Rem"
  | .neg => "Neg" | .eq => "Eq" | .ne => "Ne" | .lt => "Lt" | .le => "Le" | .gt => "Gt"
  | .ge => "Ge" | .not => "Not" | .concat => "Concat"
  | .jump o => s!"Jump +{o}" | .jumpIfFalse o => s!"JumpIfFalse +{o}"
  | .jumpIfNone o => s!"JumpIfNone +{o}" | .unwrap => "Unwrap"
  | .call f n => s!"Call {f}/{n}" | .storeSlot i => s!"StoreSlot {i}"
  | .command n k => s!"Command {repr n}/{k}" | .pop => "Pop" | .bindLocal => "BindLocal"
  | .loadLocal i => s!"LoadLocal {i}" | .dropLocal => "DropLocal" | .ret => "Return"
  | .send m s n => s!"Send {m} {repr s}/{n}" | .refresh r => s!"Refresh {r}"
  | .pendingResource r => s!"PendingResource {r}" | .pendingMutation m => s!"PendingMutation {m}"
  | .failedResource r => s!"FailedResource {r}" | .nativeProps n => s!"NativeProps {n}"
  | .map o => s!"Map +{o}" | .filter o => s!"Filter +{o}"

end Contract.Vm
