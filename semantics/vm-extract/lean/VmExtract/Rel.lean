/-
The correspondence between the extracted machine's states and the model's.
-/
import VmExtract.Host
import VmExtract.Equal

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

/-- An opcode's name, as the model names a type mismatch. -/
def opName : Op → String
  | .Number => "Number" | .Bool => "Bool" | .Str => "Str" | .None => "None"
  | .Unit => "Unit" | .Some => "Some" | .LoadSlot => "LoadSlot" | .LoadDerive => "LoadDerive"
  | .LoadResource => "LoadResource" | .LoadParam => "LoadParam" | .LoadItem => "LoadItem"
  | .LoadBound => "LoadBound" | .Field => "Field" | .Record => "Record" | .List => "List"
  | .Add => "Add" | .Sub => "Sub" | .Mul => "Mul" | .Div => "Div" | .Rem => "Rem"
  | .Neg => "Neg" | .Eq => "Eq" | .Ne => "Ne" | .Lt => "Lt" | .Le => "Le" | .Gt => "Gt"
  | .Ge => "Ge" | .Not => "Not" | .Concat => "Concat" | .Jump => "Jump"
  | .JumpIfFalse => "JumpIfFalse" | .JumpIfNone => "JumpIfNone" | .Unwrap => "Unwrap"
  | .Call => "Call" | .StoreSlot => "StoreSlot" | .Command => "Command" | .Pop => "Pop"
  | .BindLocal => "BindLocal" | .LoadLocal => "LoadLocal" | .DropLocal => "DropLocal"
  | .Return => "Return" | .Send => "Send" | .Refresh => "Refresh"
  | .PendingResource => "PendingResource" | .PendingMutation => "PendingMutation"
  | .FailedResource => "FailedResource" | .NativeProps => "NativeProps"
  | .LoadIndex => "LoadIndex" | .Map => "Map" | .Filter => "Filter"

/-- A Rust trap and the model's for the same failure (the pc and operands
the Rust trap carries are not the model's). A roster refusal is the Rust
`TypeMismatch` at `Call`. What the model leaves out (`NativeProps`) is
`unsupported` there, whatever the Rust does. -/
def TrapRel : Trap → Contract.Vm.Trap → Prop
  | _, .unsupported _ => True
  | .StackUnderflow _, .stackUnderflow => True
  | .TypeMismatch _ .Call, .call _ => True
  | .TypeMismatch _ op, .typeMismatch s => s = opName op
  | .UnwrapNone _, .unwrapNone => True
  | .BadField _ _, .badField => True
  | .BadScope _ _, .badScope => True
  | .BadParam _ _, .badParam => True
  | .Arity _ _, .arity => True
  | .WriteNotDeclared _ _, .writeNotDeclared => True
  | .BadJump _ _, .badJump => True
  | .Malformed _, .malformed => True
  | .NoResult, .noResult => True
  | .Pending _, .pending => True
  | _, _ => False

/-- A callback in progress, Rust's and the model's. -/
def CbRel (c : machine.Callback Value Unit) (c' : Contract.Vm.Callback) : Prop :=
  c.filter = c'.filter ∧ c.start.val = c'.start ∧ c.end.val = c'.stop ∧
  c.out.val = c'.out ∧ c.locals.val = c'.base ∧ c.caller.val = c'.caller.reverse ∧
  c.next.val = c'.idx ∧
  ∃ xs, c.list = .list xs ∧ c.len.val = xs.length ∧ xs[c'.idx]? = some c'.cur ∧
    c'.rest = xs.drop (c'.idx + 1) ∧ (c'.filter = true → c'.out.Sublist (xs.take c'.idx))

/-- A Rust machine and host, and the model's machine, in `code` and `env`:
the same position, operand stack (Rust's top last, the model's first),
locals, callbacks (Rust's innermost last) and effects. -/
def Rel (code : Contract.Vm.Code) (env : Contract.Vm.Env) (h : LHost)
    (m : machine.Machine Value Unit) (M : Contract.Vm.Machine) : Prop :=
  h.code = code ∧ h.env = env ∧ h.fx = M.fx ∧
  m.pos.val = M.pc ∧ m.stack.val = M.stack.reverse ∧ m.locals.val = M.locals ∧
  List.Forall₂ CbRel m.callbacks.val.reverse M.cbs

/-- What a Rust step's outcome says of the model's. -/
def Out (code : Contract.Vm.Code) (env : Contract.Vm.Env)
    (r : core.result.Result (Option Value) Trap) (h : LHost) (m : machine.Machine Value Unit) :
    Contract.Vm.Out → Prop
  | .ok (.run M) => r = .Ok .none ∧ Rel code env h m M
  | .ok (.done v fx) => r = .Ok (.some v) ∧ h.fx = fx
  | .error t => ∃ t', r = .Err t' ∧ TrapRel t' t

/-! ## Single opcodes -/

@[simp] theorem valInst_number (f : F64) : valInst.number f = ok (.num f) := rfl
@[simp] theorem valInst_boolean (b : Bool) : valInst.boolean b = ok (.bool b) := rfl
@[simp] theorem valInst_unit : valInst.unit = ok .unit := rfl
@[simp] theorem valInst_none : valInst.none = ok .none := rfl
@[simp] theorem valInst_num (op x y) : valInst.num op x y = ok (numOp op x y) := rfl
@[simp] theorem valInst_negate (x : F64) : valInst.negate x = ok (.num (-x)) := rfl
@[simp] theorem valInst_index (n : Usize) : valInst.index n = ok (.num (Contract.F64.ofNat n.val)) := rfl
@[simp] theorem valInst_concat (a b : Value) : valInst.concat a b = ok (concatOf a b) := rfl

theorem kindOf_iff {v k} : kindOf v = ok k ↔ KindOf v k := by
  refine ⟨kindOf_eq, fun h => ?_⟩
  cases v <;> simp only [KindOf] at h <;>
    first
    | (obtain ⟨w, rfl⟩ := h; simp [kindOf, w])
    | (subst h; simp [kindOf])

@[simp] theorem kindOf_bind {β} {v : Value} {g : machine.Kind Value → Result β} {r : β} :
    Std.bind (kindOf v) g = ok r ↔ ∃ k, KindOf v k ∧ g k = ok r := by
  simp only [bind_eq_ok, kindOf_iff]

end VmExtract
