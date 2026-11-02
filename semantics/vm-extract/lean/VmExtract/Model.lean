/-
The extracted machine instantiated with the semantics: `Val` with
`Contract.Value` and its operations, `Host` with the VM model's code,
environment and effects. Each method does what `Contract.Vm.exec` does at
that point; what they stand for in the runner (`runner/src/vm.rs`, `impl Val
for Value` and `impl Host for Run`) is the trusted correspondence (README.md).
-/
import VmExtract.Funs
import Contract.Vm

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

/-! ## Values -/

/-- A value's kind; a list longer than a Rust slice can be is not one. -/
def kindOf : Value → Result (machine.Kind Value)
  | .num f => ok (.Number f)
  | .bool b => ok (.Bool b)
  | .str _ => ok .Str
  | .unit => ok .Unit
  | .none => ok .None
  | .some v => ok (.Some v)
  | .list xs => if h : xs.length ≤ Usize.max then ok (.List (Slice.from xs h)) else fail .maximumSizeExceeded
  | .record _ fs => if h : fs.length ≤ Usize.max then ok (.Record (Slice.from fs h)) else fail .maximumSizeExceeded

/-- The binary number opcodes as `Contract.Vm.binary` computes them. -/
def numOp : machine.Num → F64 → F64 → Value
  | .Add, x, y => .num (x + y)
  | .Sub, x, y => .num (x - y)
  | .Mul, x, y => .num (x * y)
  | .Div, x, y => .num (x / y)
  | .Rem, x, y => .num (Contract.Number.fmod x y)
  | .Lt, x, y => .bool (x < y)
  | .Le, x, y => .bool (x ≤ y)
  | .Gt, x, y => .bool (x > y)
  | .Ge, x, y => .bool (x ≥ y)

def strEq : Value → Value → Bool
  | .str s, .str t => s == t
  | _, _ => false

def concatOf : Value → Value → Option Value
  | .str s, .str t => some (.str (s ++ t))
  | _, _ => none

/-- `Val` for the semantics' values. -/
def valInst : machine.Val Value where
  corecloneCloneInst := { clone := fun v => ok v }
  kind := kindOf
  number := fun f => ok (.num f)
  index := fun n => ok (.num (Contract.F64.ofNat n.val))
  boolean := fun b => ok (.bool b)
  unit := ok .unit
  none := ok .none
  num := fun op x y => ok (numOp op x y)
  negate := fun x => ok (.num (-x))
  num_eq := fun x y => ok (x == y)
  str_eq := fun a b => ok (strEq a b)
  concat := fun a b => ok (concatOf a b)

end VmExtract
