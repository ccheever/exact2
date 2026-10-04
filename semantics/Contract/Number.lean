/-
Numbers: IEEE-754 binary64, exactly.

Arithmetic is the hardware's (`Float` is a C `double`); what a C `double`
does not give — `%` as fmod, `max`/`min` with the runner's NaN and signed
zero rules, and JavaScript's `Number.prototype.toString` — is defined here
over exact rationals, with `Nat` bignums, so nothing rounds by accident.
-/
namespace Contract.Number

/-- The parts of a finite double: sign, significand `m`, and exponent `e`
with `|x| = m * 2^e` exactly. Zero is `m = 0`. -/
structure Parts where
  neg : Bool
  m : Nat
  e : Int
  deriving Repr

def bits (x : Float) : UInt64 := x.toBits

def isNaN (x : Float) : Bool := x != x

def isFinite (x : Float) : Bool := !isNaN x && x - x == 0

def signBit (x : Float) : Bool := (bits x >>> 63) == 1

/-- Decompose a finite double. -/
def parts (x : Float) : Parts :=
  let b := bits x
  let neg := (b >>> 63) == 1
  let ex := ((b >>> 52) &&& 0x7ff).toNat
  let frac := (b &&& 0xfffffffffffff).toNat
  if ex == 0 then { neg, m := frac, e := -1074 }
  else { neg, m := frac + 2 ^ 52, e := (ex : Int) - 1075 }

/-- A non-negative rational as numerator/denominator. -/
structure Q where
  num : Nat
  den : Nat
  deriving Repr

def pow2 (e : Int) : Q :=
  if e ≥ 0 then { num := 2 ^ e.toNat, den := 1 } else { num := 1, den := 2 ^ (-e).toNat }

def pow10 (e : Int) : Q :=
  if e ≥ 0 then { num := 10 ^ e.toNat, den := 1 } else { num := 1, den := 10 ^ (-e).toNat }

def Q.mul (a b : Q) : Q := { num := a.num * b.num, den := a.den * b.den }

/-- `a < b`, `a ≤ b` on rationals. -/
def Q.lt (a b : Q) : Bool := a.num * b.den < b.num * a.den
def Q.le (a b : Q) : Bool := a.num * b.den ≤ b.num * a.den

/-- `|x|` as a rational. -/
def absQ (p : Parts) : Q := Q.mul { num := p.m, den := 1 } (pow2 p.e)

/-- `floor(log10 q)` for `q > 0`. -/
partial def floorLog10 (q : Q) : Int :=
  -- Start from a binary estimate, then correct.
  let rec up (t : Int) : Int := if (pow10 (t + 1)).le q then up (t + 1) else t
  let rec down (t : Int) : Int := if (pow10 t).le q then t else down (t - 1)
  let est : Int := ((Nat.log2 q.num : Int) - (Nat.log2 q.den : Int)) * 30103 / 100000
  up (down (est + 2))

/-- `round(q)` to the nearest integer, ties to even. -/
def roundHalfEven (q : Q) : Nat :=
  let f := q.num / q.den
  let r := q.num % q.den
  if 2 * r < q.den then f
  else if 2 * r > q.den then f + 1
  else if f % 2 == 0 then f else f + 1

/-- The interval of rationals that read back as the double `p` (round to
nearest, ties to even): `(lo, hi, inclusive)`. -/
def roundingInterval (p : Parts) : Q × Q × Bool :=
  let incl := p.m % 2 == 0
  let hi := Q.mul { num := 2 * p.m + 1, den := 1 } (pow2 (p.e - 1))
  let lo :=
    if p.m == 2 ^ 52 && p.e > -1074 then
      Q.mul { num := 4 * p.m - 1, den := 1 } (pow2 (p.e - 2))
    else
      Q.mul { num := 2 * p.m - 1, den := 1 } (pow2 (p.e - 1))
  (lo, hi, incl)

/-- The shortest decimal that reads back as `|x|`: digits `s` and the
power of ten `q` of its last digit, `|x| ≈ s * 10^q`. Among the shortest,
the nearest to `x`; a tie, the even one. `x` finite and nonzero. -/
def shortest (p : Parts) : Nat × Int :=
  let x := absQ p
  let (lo, hi, incl) := roundingInterval p
  let inside (c : Q) : Bool :=
    if incl then lo.le c && c.le hi else lo.lt c && c.lt hi
  let dist (c : Q) : Q :=
    -- |c - x| as a rational, sign dropped.
    let a := c.num * x.den
    let b := x.num * c.den
    { num := if a ≥ b then a - b else b - a, den := c.den * x.den }
  let top := floorLog10 x
  -- Try the last digit's place from `top` down; 17 significant digits
  -- always suffice for a double, so the loop is bounded.
  let rec go (k : Nat) (q : Int) : Nat × Int :=
    match k with
    | 0 => (roundHalfEven (Q.mul x (pow10 (-q))), q)
    | k + 1 =>
      let s := roundHalfEven (Q.mul x (pow10 (-q)))
      let cands := [s, s + 1] ++ (if s > 0 then [s - 1] else [])
      let ok := cands.filter (fun c => c > 0 && inside (Q.mul { num := c, den := 1 } (pow10 q)))
      match ok with
      | [] => go k (q - 1)
      | c :: cs =>
        let best := cs.foldl (fun b c =>
          let db := dist (Q.mul { num := b, den := 1 } (pow10 q))
          let dc := dist (Q.mul { num := c, den := 1 } (pow10 q))
          if dc.lt db then c else if db.lt dc then b else if c % 2 == 0 then c else b) c
        (best, q)
  let (s, q) := go 40 top
  -- Normalize away trailing zeros (a rounding up to a power of ten).
  let rec strip (fuel : Nat) (s : Nat) (q : Int) : Nat × Int :=
    match fuel with
    | 0 => (s, q)
    | f + 1 => if s % 10 == 0 && s > 0 then strip f (s / 10) (q + 1) else (s, q)
  strip 40 s q

def digitsOf (s : Nat) : String := toString s

/-- JavaScript's `Number::toString(10)` (ECMA-262 §6.1.6.1.20). -/
def jsToString (x : Float) : String :=
  if isNaN x then "NaN"
  else if x == 0 then "0"
  else if !isFinite x then (if x < 0 then "-Infinity" else "Infinity")
  else
    let p := parts x
    let (s, q) := shortest p
    let ds := digitsOf s
    let k : Int := ds.length
    let n : Int := q + k
    let sign := if p.neg then "-" else ""
    let body :=
      if k ≤ n && n ≤ 21 then ds ++ String.ofList (List.replicate (n - k).toNat '0')
      else if 0 < n && n ≤ 21 then
        String.ofList (ds.toList.take n.toNat) ++ "." ++ String.ofList (ds.toList.drop n.toNat)
      else if -6 < n && n ≤ 0 then
        "0." ++ String.ofList (List.replicate (-n).toNat '0') ++ ds
      else
        let e := n - 1
        let es := if e ≥ 0 then "+" ++ toString e else "-" ++ toString (-e)
        let mant := if k == 1 then ds else
          String.ofList (ds.toList.take 1) ++ "." ++ String.ofList (ds.toList.drop 1)
        mant ++ "e" ++ es
    sign ++ body

/-- A finite double from an exact `±m * 2^e` that is representable. -/
def ofParts (neg : Bool) (m : Nat) (e : Int) : Float :=
  let mag := (Float.ofNat m).scaleB e
  if neg then -mag else mag

/-- `a % b` as C's `fmod` (Rust's `%` on `f64`): exact, the sign of `a`. -/
def fmod (a b : Float) : Float :=
  if isNaN a || isNaN b || !isFinite a || b == 0 then 0.0 / 0.0
  else if !isFinite b then a
  else if a == 0 then a
  else
    let pa := parts a
    let pb := parts b
    -- Bring both to the smaller exponent and take integer remainders.
    let e := min pa.e pb.e
    let ma := pa.m * 2 ^ (pa.e - e).toNat
    let mb := pb.m * 2 ^ (pb.e - e).toNat
    let r := ma % mb
    if r == 0 then (if pa.neg then -0.0 else 0.0) else ofParts pa.neg r e

/-- Rust's `f64::max` on aarch64 (`fmaxnm`): a NaN yields the other
argument; `+0` is above `-0`. -/
def fmax (a b : Float) : Float :=
  if isNaN a then b else if isNaN b then a
  else if a > b then a else if b > a then b
  else if a == 0 && b == 0 then (if signBit a then b else a)
  else a

/-- Rust's `f64::min` on aarch64 (`fminnm`). -/
def fmin (a b : Float) : Float :=
  if isNaN a then b else if isNaN b then a
  else if a < b then a else if b < a then b
  else if a == 0 && b == 0 then (if signBit a then a else b)
  else a

/-- `Math.trunc` toward zero, exact. -/
def trunc (x : Float) : Float :=
  if isNaN x || !isFinite x then x
  else if x < 0 then -((-x).floor) else x.floor

/-- The canonical bits of a number in an observation: every NaN is one. -/
def canonicalBits (x : Float) : UInt64 :=
  if isNaN x then 0x7ff8000000000000 else bits x

end Contract.Number
