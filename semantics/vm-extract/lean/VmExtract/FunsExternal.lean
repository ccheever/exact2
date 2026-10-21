/-
Models of the standard-library functions the machine calls that Aeneas's
library does not model (Aeneas lists them in `FunsExternal_Template.lean`).
These are part of the trusted base: each is the documented behaviour of the
Rust function, over the vector's list of elements.
-/
import Aeneas
import VmExtract.Types
open Aeneas Aeneas.Std Result ControlFlow Error
open vm_extract

/-- `core::mem::take`: the value, and the default left in its place. -/
@[rust_fun "core::mem::take"]
def core.mem.take {T : Type} (defaultDefaultInst : core.default.Default T) (x : T) :
    Result (T × T) := do
  let d ← defaultDefaultInst.default
  ok (x, d)

/-- `Vec::truncate`: the first `n` elements (all of them when fewer). -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::truncate"]
def alloc.vec.Vec.truncate {T : Type} (_A : Type) (v : alloc.vec.Vec T) (n : Usize) :
    Result (alloc.vec.Vec T) :=
  ok (alloc.vec.Vec.from (v.val.take n.val) (by have := v.property; simp only [List.length_take]; omega))

/-- `Vec::pop`: the last element, and the vector without it. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (_A : Type) (v : alloc.vec.Vec T) :
    Result ((Option T) × (alloc.vec.Vec T)) :=
  ok (v.val.getLast?, alloc.vec.Vec.from v.val.dropLast
    (by have := v.property; simp only [List.length_dropLast]; omega))

/-- `Vec::is_empty`. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty {T : Type} (_A : Type) (v : alloc.vec.Vec T) : Result Bool :=
  ok v.val.isEmpty

/-- `Vec::split_off(at)`: the elements from `at` on, and the vector keeping
the first `at`; a panic past the end. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::split_off"]
def alloc.vec.Vec.split_off {T : Type} {A : Type} (_cloneInst : core.clone.Clone A)
    (v : alloc.vec.Vec T) (at_ : Usize) : Result ((alloc.vec.Vec T) × (alloc.vec.Vec T)) :=
  if at_.val ≤ v.val.length then
    ok (alloc.vec.Vec.from (v.val.drop at_.val)
          (Nat.le_trans (by simp) v.property),
        alloc.vec.Vec.from (v.val.take at_.val)
          (by have := v.property; simp only [List.length_take]; omega))
  else fail .panic

/-- `Vec::default`: the empty vector. -/
@[rust_fun "alloc::vec::{core::default::Default<alloc::vec::Vec<@T>>}::default"]
def alloc.vec.Vec.Insts.CoreDefaultDefault.default (T : Type) : Result (alloc.vec.Vec T) :=
  ok (alloc.vec.Vec.new T)
