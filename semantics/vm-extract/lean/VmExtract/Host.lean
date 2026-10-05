/-
`Host` for the model: the code, the environment and the effects of
`Contract.Vm`, each method doing what `Contract.Vm.exec` does at that point
of the opcode (`runner/src/vm.rs`'s `impl Host for Run` is its counterpart).
-/
import VmExtract.Model

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

abbrev Op := exact_plan.generated.Opcode
abbrev Trap := machine.Trap

/-- What the machine reads and writes beyond its stacks: the model's code,
environment and effects so far. -/
structure LHost where
  code : Contract.Vm.Code
  env : Contract.Vm.Env
  fx : Contract.Vm.Effects

/-- An operand of `bits` bits, as the plan's bytes hold it. -/
def operand (bits : Nat) (x : Nat) : Result U64 :=
  if x < 2 ^ bits then UScalar.tryMk .U64 x else fail .panic

def zero : Result U64 := ok 0#u64

/-- An instruction of the opcode with these operands. -/
def mkIns (pc : Usize) (op : Op) (a b c : Result U64) (number : F64 := 0) :
    Result machine.Instruction := do
  let a ← a
  let b ← b
  let c ← c
  ok { pc, op, args := Array.make 3#usize [a, b, c], number }

/-- The instruction at `pc` as the plan encodes it, a jump's target an
instruction index; a pool operand (string, type, roster entry) is `pc`
itself, which the host reads back from the code. -/
def encIns (pc : Usize) : Contract.Vm.Instr → Result machine.Instruction
  | .num b => mkIns pc .Number zero zero zero (Contract.F64.ofBits b)
  | .bool b => mkIns pc .Bool (operand 8 (if b then 1 else 0)) zero zero
  | .str _ => mkIns pc .Str (operand 32 pc.val) zero zero
  | .none => mkIns pc .None zero zero zero
  | .unit => mkIns pc .Unit zero zero zero
  | .some => mkIns pc .Some zero zero zero
  | .loadSlot i => mkIns pc .LoadSlot (operand 32 i) zero zero
  | .loadDerive i => mkIns pc .LoadDerive (operand 32 i) zero zero
  | .loadResource i => mkIns pc .LoadResource (operand 32 i) zero zero
  | .loadParam i => mkIns pc .LoadParam (operand 16 i) zero zero
  | .loadItem d => mkIns pc .LoadItem (operand 16 d) zero zero
  | .loadIndex d => mkIns pc .LoadIndex (operand 16 d) zero zero
  | .loadBound d => mkIns pc .LoadBound (operand 16 d) zero zero
  | .field i => mkIns pc .Field (operand 16 i) zero zero
  | .record _ _ => mkIns pc .Record (operand 32 pc.val) zero zero
  | .list n => mkIns pc .List (operand 32 n) zero zero
  | .add => mkIns pc .Add zero zero zero
  | .sub => mkIns pc .Sub zero zero zero
  | .mul => mkIns pc .Mul zero zero zero
  | .div => mkIns pc .Div zero zero zero
  | .rem => mkIns pc .Rem zero zero zero
  | .neg => mkIns pc .Neg zero zero zero
  | .eq => mkIns pc .Eq zero zero zero
  | .ne => mkIns pc .Ne zero zero zero
  | .lt => mkIns pc .Lt zero zero zero
  | .le => mkIns pc .Le zero zero zero
  | .gt => mkIns pc .Gt zero zero zero
  | .ge => mkIns pc .Ge zero zero zero
  | .not => mkIns pc .Not zero zero zero
  | .concat => mkIns pc .Concat zero zero zero
  | .jump off => mkIns pc .Jump (operand 32 (pc.val + 1 + off)) zero zero
  | .jumpIfFalse off => mkIns pc .JumpIfFalse (operand 32 (pc.val + 1 + off)) zero zero
  | .jumpIfNone off => mkIns pc .JumpIfNone (operand 32 (pc.val + 1 + off)) zero zero
  | .unwrap => mkIns pc .Unwrap zero zero zero
  | .call _ _ => mkIns pc .Call (operand 32 pc.val) zero zero
  | .storeSlot i => mkIns pc .StoreSlot (operand 32 i) zero zero
  | .command _ n => mkIns pc .Command (operand 32 pc.val) (operand 16 n) zero
  | .pop => mkIns pc .Pop zero zero zero
  | .bindLocal => mkIns pc .BindLocal zero zero zero
  | .loadLocal i => mkIns pc .LoadLocal (operand 16 i) zero zero
  | .dropLocal => mkIns pc .DropLocal zero zero zero
  | .ret => mkIns pc .Return zero zero zero
  | .send m _ n => mkIns pc .Send (operand 32 m) (operand 32 pc.val) (operand 16 n)
  | .refresh r => mkIns pc .Refresh (operand 32 r) zero zero
  | .pendingResource r => mkIns pc .PendingResource (operand 32 r) zero zero
  | .pendingMutation k => mkIns pc .PendingMutation (operand 32 k) zero zero
  | .failedResource r => mkIns pc .FailedResource (operand 32 r) zero zero
  | .nativeProps n => mkIns pc .NativeProps (operand 32 n) zero zero
  | .map off => mkIns pc .Map (operand 32 (pc.val + 1 + off)) zero zero
  | .filter off => mkIns pc .Filter (operand 32 (pc.val + 1 + off)) zero zero

/-- Entry `i` of a table. -/
def lookup {α} (xs : List α) (i : U64) : Option α := xs[i.val]?

namespace LHost

def fetch (h : LHost) (pos : Usize) :
    Result ((core.result.Result (Option (machine.Instruction × Usize)) Trap) × LHost) :=
  match h.code[pos.val]? with
  | Option.none => ok (.Ok .none, h)
  | Option.some i => do
    let ins ← encIns pos i
    let next ← pos + 1#usize
    ok (.Ok (.some (ins, next)), h)

def codeLen (h : LHost) : Result Usize := UScalar.tryMk .Usize h.code.length

def string (h : LHost) (i : U64) (pc : Usize) : Result (core.result.Result Value Trap) :=
  match lookup h.code i with
  | Option.some (.str s) => ok (.Ok (.str s))
  | _ => ok (.Err (.Malformed pc))

def loadSlot (h : LHost) (i : U64) (pc : Usize) : Result (core.result.Result Value Trap) :=
  match lookup h.env.slots i with
  | Option.none => ok (.Err (.Malformed pc))
  | Option.some Option.none => ok (.Err (.BadScope pc 0#u16))
  | Option.some (Option.some v) => ok (.Ok v)

def loadSettled (xs : List (Option Value)) (i : U64) (pc : Usize) :
    core.result.Result Value Trap :=
  match lookup xs i with
  | Option.none => .Err (.Malformed pc)
  | Option.some Option.none => .Err (.Pending pc)
  | Option.some (Option.some v) => .Ok v

def loadParam (h : LHost) (i : U64) (pc : Usize) : Result (core.result.Result Value Trap) :=
  match lookup h.env.params i with
  | Option.none => ok (.Err (.BadParam pc 0#u16))
  | Option.some v => ok (.Ok v)

def frameRead (h : LHost) (op : Op) (d : U64) : Option Value :=
  match op with
  | .LoadIndex => ((lookup h.env.frames d).bind (·.index)).map (fun k => .num (Contract.F64.ofNat k))
  | .LoadItem => (lookup h.env.frames d).bind (·.item)
  | _ => (lookup h.env.frames d).bind (·.bound)

def loadFrame (h : LHost) (op : Op) (d : U64) (pc : Usize) :
    Result (core.result.Result Value Trap) :=
  match frameRead h op d with
  | Option.none => ok (.Err (.BadScope pc 0#u16))
  | Option.some v => ok (.Ok v)

def resourceFlag (h : LHost) (op : Op) (i : U64) (pc : Usize) :
    Result (core.result.Result Bool Trap) :=
  match lookup h.env.resources i with
  | Option.some (Option.some _) =>
    ok (.Ok (match op with
      | .FailedResource => (lookup h.env.failedResources i).getD false
      | _ => (lookup h.env.pendingResources i).getD false))
  | _ => ok (.Err (.Pending pc))

def recordLen (h : LHost) (ty : U64) (pc : Usize) : Result (core.result.Result Usize Trap) :=
  match lookup h.code ty with
  | Option.some (.record _ n) => do let n ← UScalar.tryMk .Usize n; ok (.Ok n)
  | _ => ok (.Err (.Malformed pc))

def record (h : LHost) (ty : U64) (fields : alloc.vec.Vec Value) (pc : Usize) :
    Result ((core.result.Result Value Trap) × LHost) :=
  match lookup h.code ty with
  | Option.some (.record shape _) => ok (.Ok (.record shape fields.val), h)
  | _ => ok (.Err (.Malformed pc), h)

def arity (h : LHost) (f : U64) (pc : Usize) : Result (core.result.Result Usize Trap) :=
  match lookup h.code f with
  | Option.some (.call _ n) => do let n ← UScalar.tryMk .Usize n; ok (.Ok n)
  | _ => ok (.Err (.Malformed pc))

def call (h : LHost) (f : U64) (stack : Slice Value) (at_ : Usize) (pc : Usize) :
    Result ((core.result.Result Value Trap) × LHost) :=
  match lookup h.code f with
  | Option.some (.call fn _) =>
    match Contract.stdlib (Contract.Vm.callEnv h.env) fn (stack.val.drop at_.val) with
    | .ok v => ok (.Ok v, h)
    | .error _ => ok (.Err (.TypeMismatch pc .Call), h)
  | _ => ok (.Err (.Malformed pc), h)

def store (h : LHost) (slot : U64) (v : Value) (pc : Usize) :
    Result ((core.result.Result Unit Trap) × LHost) :=
  match lookup h.env.owned slot with
  | Option.none => ok (.Err (.Malformed pc), h)
  | Option.some false =>
    ok (.Ok (), { h with fx := { h.fx with writes := h.fx.writes ++ [(slot.val, v)] } })
  | Option.some true =>
    match lookup h.env.slots slot with
    | Option.some (Option.some _) =>
      ok (.Ok (), { h with fx := { h.fx with rowWrites := h.fx.rowWrites ++ [(slot.val, v)] } })
    | _ => ok (.Err (.BadScope pc 0#u16), h)

def command (h : LHost) (nameIdx : U64) (args : alloc.vec.Vec Value) : Result LHost :=
  match lookup h.code nameIdx with
  | Option.some (.command nm _) =>
    ok { h with fx := { h.fx with commands := h.fx.commands ++ [(nm, args.val)] } }
  | _ => fail .panic

def mutationSlot (h : LHost) (m : U64) (pc : Usize) : Result (core.result.Result U64 Trap) :=
  match lookup h.env.mutationSlots m with
  | Option.none => ok (.Err (.Malformed pc))
  | Option.some s => do let s ← UScalar.tryMk .U64 s; ok (.Ok s)

def send (h : LHost) (m source : U64) (args : alloc.vec.Vec Value) : Result LHost :=
  match lookup h.code source with
  | Option.some (.send _ src _) =>
    ok { h with fx := { h.fx with sends := h.fx.sends ++ [(m.val, src, args.val)] } }
  | _ => fail .panic

end LHost

/-- `Host` for the model. -/
def hostInst : machine.Host LHost Value Unit where
  coremarkerCopyInst := BuiltinCopy Unit
  fetch := LHost.fetch
  code_len := LHost.codeLen
  string := LHost.string
  load_slot := LHost.loadSlot
  load_derive := fun h i pc => ok (LHost.loadSettled h.env.derives i pc, h)
  load_resource := fun h i pc => ok (LHost.loadSettled h.env.resources i pc, h)
  load_param := LHost.loadParam
  load_frame := LHost.loadFrame
  resource_flag := LHost.resourceFlag
  pending_mutation := fun h i => ok ((lookup h.env.pendingMutations i).getD false)
  some := fun h v _ => ok (.Ok (.some v), h)
  list := fun h items _ => ok (.Ok (.list items.val), h)
  record_len := LHost.recordLen
  record := LHost.record
  native_props := fun h _ pc => ok (.Err (.TypeMismatch pc .NativeProps), h)
  arity := LHost.arity
  call := LHost.call
  may_write := fun h slot => ok (h.env.writable.contains slot.val)
  store := LHost.store
  command := LHost.command
  mutation_slot := LHost.mutationSlot
  send := LHost.send
  refresh := fun h r => ok { h with fx := { h.fx with refreshes := h.fx.refreshes ++ [r.val] } }
  steps := fun h _ _ => ok (.Ok (), h)
  extent := fun _ => ok ()
  kept := fun h _ _ _ => ok (.Ok (), h)
  collected := fun h out _ => ok (.list out.val, h)

end VmExtract
