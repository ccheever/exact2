/-
What the generated `Types.lean` needs that Aeneas's library lacks: a model
of Rust's `f64`. The machine only moves `f64`s (every operation on one is a
`Val` method, instantiated by the semantics' own), so the model is the
semantics' binary64, `Contract.F64` (`semantics/Contract/Binary64.lean`:
IEEE 754 binary64 by its bits, its operations proved correctly rounded and
checked against the hardware's `f64` by `difftest arith`).
-/
import Aeneas
import Contract.Binary64

abbrev F64 := Contract.F64
