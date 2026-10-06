/-
IEEE-754 binary64, modelled exactly.

A double is its 64 bits (`F64`). Every finite double is an integer multiple
of `2^-1074`, so its value is carried as that integer (`scaled`): a
significand `sig < 2^53` times `2^uexp`, signed. Arithmetic is the exact
result rounded once, to nearest with ties to even, by `round` — over `Nat`
and `Int` alone, so the kernel computes it and proofs can reason about it.
`Contract.Binary64Facts` proves `round` correct (the nearest double, ties to
even, overflow exactly at `2^1024 - 2^970`), monotone, and exact on what is
representable; `difftest arith` checks every operation here against the
hardware's `f64` (`semantics/README.md`).
-/
namespace Contract

/-- A binary64 value, by its bits. -/
structure F64 where
  bits : UInt64
  deriving Inhabited, Repr, DecidableEq

namespace F64

@[inline] def ofBits (b : UInt64) : F64 := ⟨b⟩
@[inline] def toBits (x : F64) : UInt64 := x.bits

/-! ## Decoding -/

/-- The bits as a number. -/
def word (x : F64) : Nat := x.bits.toNat

/-- The sign bit. -/
def sign (x : F64) : Bool := decide (2 ^ 63 ≤ x.word)

/-- The biased exponent field (11 bits). -/
def bexp (x : F64) : Nat := x.word / 2 ^ 52 % 2 ^ 11

/-- The fraction field (52 bits). -/
def frac (x : F64) : Nat := x.word % 2 ^ 52

def isNaN (x : F64) : Bool := x.bexp == 2047 && x.frac != 0
def isInf (x : F64) : Bool := x.bexp == 2047 && x.frac == 0
def isFinite (x : F64) : Bool := x.bexp != 2047

/-- The significand, with the hidden bit of a normal number. -/
def sig (x : F64) : Nat := if x.bexp = 0 then x.frac else x.frac + 2 ^ 52

/-- The exponent of the significand's unit, offset by 1074: `|x| = sig *
2^(uexp - 1074)`. A subnormal's is 0, as is the smallest normal's. -/
def uexp (x : F64) : Nat := x.bexp - 1

/-- `|x| * 2^1074` of a finite double: a natural number. -/
def mag (x : F64) : Nat := x.sig * 2 ^ x.uexp

/-- `x * 2^1074` of a finite double, signed. `±0` are both `0`. -/
def scaled (x : F64) : Int := if x.sign then -(x.mag : Int) else x.mag

/-! ## Encoding -/

/-- The finite double `±m * 2^(p - 1074)`, for `m < 2^53` with `m ≥ 2^52`
or `p = 0` (subnormal or zero), and `p ≤ 2045`. -/
def make (s : Bool) (m p : Nat) : F64 :=
  ⟨UInt64.ofNat ((if s then 2 ^ 63 else 0) +
    if m < 2 ^ 52 then m else (p + 1) * 2 ^ 52 + (m - 2 ^ 52))⟩

def inf (s : Bool) : F64 := ⟨UInt64.ofNat ((if s then 2 ^ 63 else 0) + 2047 * 2 ^ 52)⟩

/-- The default NaN (what an invalid operation gives on aarch64 and x86). -/
def nan : F64 := ⟨0x7ff8000000000000⟩

/-! ## Rounding -/

/-- The integer nearest to `a / d`, ties to even (`d > 0`). -/
def rne (a d : Nat) : Nat :=
  let f := a / d
  let r := a % d
  if 2 * r < d then f
  else if d < 2 * r then f + 1
  else if f % 2 = 0 then f else f + 1

/-- The unit exponent of the double nearest `a / d` (`a, d > 0`), before
rounding: `max 0 (⌊log₂ (a / d)⌋ - 52)`, so that `a / (d * 2^p) < 2^53`
and, unless `p = 0`, `≥ 2^52`. -/
def ulpExp (a d : Nat) : Nat :=
  let p := a.log2 - (d.log2 + 52)
  if 0 < p ∧ a < d * 2 ^ (p + 52) then p - 1 else p

/-- The magnitude `a / d` (`a, d > 0`) rounded to a double: its
significand and unit exponent, or `none` past the largest finite. -/
def roundMag (a d : Nat) : Option (Nat × Nat) :=
  let p := ulpExp a d
  let m := rne a (d * 2 ^ p)
  -- Rounding up to `2^53` is the next binade's `2^52`.
  let q := if m = 2 ^ 53 then p + 1 else p
  if q ≤ 2045 then some (if m = 2 ^ 53 then 2 ^ 52 else m, q) else none

/-- `±a / (d * 2^1074)` rounded to nearest, ties to even (`a, d > 0`):
signed infinity past the largest finite, signed zero below the smallest
subnormal's half. -/
def round (s : Bool) (a d : Nat) : F64 :=
  match roundMag a d with
  | some (m, p) => make s m p
  | none => inf s

/-- The double of the integer `z * 2^-1074` (`z` a multiple of the result's
unit, so exact); `zs` is the sign of a zero result. -/
def ofScaled (z : Int) (zs : Bool) : F64 :=
  if z = 0 then make zs 0 0 else round (decide (z < 0)) z.natAbs 1

/-- The double nearest the natural number `n` (`n` itself up to `2^53`). -/
def ofNat (n : Nat) : F64 := if n = 0 then make false 0 0 else round false (n * 2 ^ 1074) 1

instance : OfNat F64 n := ⟨ofNat n⟩

/-! ## Arithmetic -/

/-- Negation flips the sign bit, NaN included. -/
def neg (x : F64) : F64 :=
  ⟨UInt64.ofNat (if x.word < 2 ^ 63 then x.word + 2 ^ 63 else x.word - 2 ^ 63)⟩

/-- `a + b`. An exact zero sum is `+0` unless both are `-0`. -/
def add (a b : F64) : F64 :=
  if a.isNaN || b.isNaN then nan
  else if a.isInf then (if b.isInf && b.sign != a.sign then nan else a)
  else if b.isInf then b
  else
    let z := a.scaled + b.scaled
    if z = 0 then make (a.sign && b.sign) 0 0 else round (decide (z < 0)) z.natAbs 1

def sub (a b : F64) : F64 := add a (neg b)

/-- `a * b`: `|a| |b| 2^2148` over `2^1074`. -/
def mul (a b : F64) : F64 :=
  let s := a.sign != b.sign
  if a.isNaN || b.isNaN then nan
  else if a.isInf || b.isInf then
    (if (a.isFinite && a.mag = 0) || (b.isFinite && b.mag = 0) then nan else inf s)
  else if a.mag = 0 || b.mag = 0 then make s 0 0
  else round s (a.mag * b.mag) (2 ^ 1074)

/-- `a / b`: `|a| 2^1074` over `|b|`. -/
def div (a b : F64) : F64 :=
  let s := a.sign != b.sign
  if a.isNaN || b.isNaN then nan
  else if a.isInf then (if b.isInf then nan else inf s)
  else if b.isInf then make s 0 0
  else if b.mag = 0 then (if a.mag = 0 then nan else inf s)
  else if a.mag = 0 then make s 0 0
  else round s (a.mag * 2 ^ 1074) b.mag

/-- `a % b` as C's `fmod` (Rust's `%` on `f64`): exact, the sign of `a`.
Both are integers on one grid, so it is the integer remainder. -/
def fmod (a b : F64) : F64 :=
  if a.isNaN || b.isNaN || a.isInf || (b.isFinite && b.mag = 0) then nan
  else if b.isInf || a.mag = 0 then a
  else
    let r := a.mag % b.mag
    if r = 0 then make a.sign 0 0 else round a.sign r 1

/-- `⌊x⌋`, exact: NaN and the infinities are their own floor, and a zero
result is `+0` except `floor(-0) = -0`. -/
def floor (x : F64) : F64 :=
  if !x.isFinite then x
  else
    let z := x.scaled / 2 ^ 1074 * 2 ^ 1074
    if z = x.scaled then x else ofScaled z false

/-- `⌈x⌉` as `-⌊-x⌋`: exact, and `ceil(-0.5) = -0` (LLP 1102 §3.2). -/
def ceil (x : F64) : F64 := neg (floor (neg x))

/-- JavaScript's `Math.round` (LLP 1102 §3.2): `⌊x + 1/2⌋` taken exactly, so
a half rounds up (`round(-2.5) = -2`) and `0.49999999999999994` rounds to 0,
where Rust's `f64::round` rounds a half away from zero. A zero result keeps
`x`'s sign (`round(-0.4) = -0`); NaN and the infinities are their own. -/
def jsRound (x : F64) : F64 :=
  if !x.isFinite then x
  else ofScaled ((x.scaled + 2 ^ 1073) / 2 ^ 1074 * 2 ^ 1074) x.sign

/-- A finite non-negative integer double as a `Nat` (else its floor's, or
0 when negative or not finite). -/
def toNat (x : F64) : Nat := if x.isFinite then (x.scaled / 2 ^ 1074).toNat else 0

/-! ## Comparison -/

/-- An order key: the infinities beyond every finite. Not for NaN. -/
def key (x : F64) : Int :=
  if x.isInf then (if x.sign then -2 ^ 2100 else 2 ^ 2100) else x.scaled

/-- IEEE comparisons: false when either is NaN; `-0 = +0`. -/
def lt (a b : F64) : Bool := !a.isNaN && !b.isNaN && decide (a.key < b.key)
def le (a b : F64) : Bool := !a.isNaN && !b.isNaN && decide (a.key ≤ b.key)
def beq (a b : F64) : Bool := !a.isNaN && !b.isNaN && decide (a.key = b.key)

instance : Add F64 := ⟨add⟩
instance : Sub F64 := ⟨sub⟩
instance : Mul F64 := ⟨mul⟩
instance : Div F64 := ⟨div⟩
instance : Neg F64 := ⟨neg⟩
/-- `==` is IEEE equality, as on a `Float`; structural equality is `=`. -/
instance (priority := high) : BEq F64 := ⟨beq⟩
instance : LT F64 := ⟨fun a b => lt a b = true⟩
instance : LE F64 := ⟨fun a b => le a b = true⟩
instance (a b : F64) : Decidable (a < b) := inferInstanceAs (Decidable (lt a b = true))
instance (a b : F64) : Decidable (a ≤ b) := inferInstanceAs (Decidable (le a b = true))

/-- `+∞`, for a timer that never fires again. -/
def posInf : F64 := inf false

end F64
end Contract
