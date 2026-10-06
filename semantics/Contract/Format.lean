/-
The roster's formatting and localization, as the runner computes them.

`formatTime` is `runner/src/stdlib.rs`'s, `formatDate` and `formatNumber`
the linked `format` capability's (`runner/src/format.rs`), `t` the
strings tables' (`exact_plan::strings`: `Plan::localized` and `fill`).
Each is transcribed: `en-US` at the fixed UTC offset the call names, the
calendar by Hinnant's `civil_from_days` over integers, compact numbers
from the shortest decimal digits Rust's `{}` prints (`shortestUp`: of two
equally near shortest forms, the upper, where JavaScript takes the even).
-/
import Contract.Number

namespace Contract.Format

open Number

/-- Rust's `{}` digits of a finite nonzero double: the shortest decimal
that reads back as `|x|`, the nearest of them, a tie the upper (core's
Dragon rounds up at exactly half). `Number.shortest` with that one change. -/
def shortestUp (p : Parts) : Nat × Int :=
  let x := absQ p
  let (lo, hi, incl) := roundingInterval p
  let inside (c : Q) : Bool :=
    if incl then lo.le c && c.le hi else lo.lt c && c.lt hi
  let dist (c : Q) : Q :=
    let a := c.num * x.den
    let b := x.num * c.den
    { num := if a ≥ b then a - b else b - a, den := c.den * x.den }
  let top := floorLog10 x
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
          if dc.lt db then c else if db.lt dc then b else max b c) c
        (best, q)
  let (s, q) := go 40 top
  let rec strip (fuel : Nat) (s : Nat) (q : Int) : Nat × Int :=
    match fuel with
    | 0 => (s, q)
    | f + 1 => if s % 10 == 0 && s > 0 then strip f (s / 10) (q + 1) else (s, q)
  strip 40 s q

def zeros (n : Nat) : List Char := List.replicate n '0'

/-- Rust's `{}` of `|x|` (finite, nonzero), never exponential, as its
integer digits (`[]` below one) and its fraction digits. -/
def rustDecimal (x : F64) : List Char × List Char :=
  let (s, q) := shortestUp (parts x)
  let ds := (toString s).toList
  let k : Int := ds.length
  let n := q + k
  if n ≤ 0 then ([], zeros (-n).toNat ++ ds)
  else if n < k then (ds.take n.toNat, ds.drop n.toNat)
  else (ds ++ zeros (n - k).toNat, [])

/-- An integer part grouped by threes. -/
def group (int : List Char) : List Char :=
  let len := int.length
  (int.zipIdx.map fun (d, i) => if i > 0 && (len - i) % 3 == 0 then [',', d] else [d]).flatten

/-- `formatNumber(n, "compact")`: `Intl.NumberFormat("en-US", { notation:
"compact", roundingMode: "trunc", signDisplay: "negative" })` as
`format.rs`'s `compact` reads the shortest digits: the suffix from the
unrounded magnitude (K, M, B, T, T above), every integer digit and below
ten two significant digits, truncated; grouping from five integer digits;
`""` for a non-finite value. -/
def compact (n : F64) : String :=
  if !isFinite n then ""
  else if n == 0 then "0"
  else
    let (int, frac) := rustDecimal n
    let scale := if int.length ≥ 4 then min ((int.length - 1) / 3) 4 else 0
    let cut := int.length - 3 * scale
    let moved := int.drop cut
    let int := int.take cut
    let frac := moved ++ frac
    let kept := match int.length with
      | 0 =>
        let first := (frac.findIdx? (· != '0')).getD frac.length
        frac.take (min (first + 2) frac.length)
      | 1 => frac.take (min 1 frac.length)
      | _ => []
    let kept := (kept.reverse.dropWhile (· == '0')).reverse
    let body :=
      (if int.isEmpty then ['0'] else if int.length ≥ 5 then group int else int) ++
      (if kept.isEmpty then [] else '.' :: kept)
    let suffix := match scale with
      | 1 => "K" | 2 => "M" | 3 => "B" | 4 => "T" | _ => ""
    (if n < 0 then "-" else "") ++ String.ofList body ++ suffix

/-- 0001-01-01T00:00 and 9999-12-31T23:59:59.999, in ms since the epoch. -/
def firstWall : F64 := -(62135596800000 : F64)
def lastWall : F64 := 253402300799999

/-- `wall_ms`: the wall time of `epoch` at `offset` minutes east, or none
(a non-finite argument, an offset past ±18 h, a wall time outside years
1–9999). The instant is truncated toward zero first. -/
def wallMs (epoch offset : F64) : Option F64 :=
  if !isFinite epoch || !isFinite offset || offset > 1080 || offset < -(1080 : F64) then .none
  else
    let wall := trunc epoch + offset * 60000
    if firstWall ≤ wall && wall ≤ lastWall then .some wall else .none

/-- Rust's `f64::rem_euclid`. -/
def remEuclid (a b : F64) : F64 :=
  let r := fmod a b
  if r < 0 then r + (if b < 0 then -b else b) else r

/-- A non-negative integral double as a `Nat` (Rust's `as u32` on one in range). -/
def toNat (x : F64) : Nat := x.toNat

/-- `x.floor() as i64` for `|x|` well inside `i64`. -/
def floorInt (x : F64) : Int := Int.ediv x.floor.scaled (2 ^ 1074)

/-- `formatTime(epochMs, utcOffset, "short")`: `h:mm AM`, `""` when invalid. -/
def formatTime (epoch offset : F64) : String :=
  match wallMs epoch offset with
  | .none => ""
  | .some wall =>
    let minutes := toNat ((remEuclid wall 86400000) / 60000).floor
    let h := minutes / 60
    let m := minutes % 60
    let (h12, suffix) :=
      if h == 0 then (12, " AM") else if h ≤ 11 then (h, " AM")
      else if h == 12 then (12, " PM") else (h - 12, " PM")
    toString h12 ++ ":" ++ toString (m / 10) ++ toString (m % 10) ++ suffix

/-- The proleptic Gregorian (year, month, day) of `days` since 1970-01-01. -/
def civil (days : Int) : Int × Nat × Nat :=
  let z := days + 719468
  let era := Int.ediv z 146097
  let doe := Int.emod z 146097
  let yoe := Int.ediv (doe - Int.ediv doe 1460 + Int.ediv doe 36524 - Int.ediv doe 146096) 365
  let doy := doe - (365 * yoe + Int.ediv yoe 4 - Int.ediv yoe 100)
  let mp := Int.ediv (5 * doy + 2) 153
  let day := doy - Int.ediv (153 * mp + 2) 5 + 1
  let month := if mp < 10 then mp + 3 else mp - 9
  (yoe + era * 400 + (if month ≤ 2 then 1 else 0), month.toNat, day.toNat)

def months : List String :=
  ["January", "February", "March", "April", "May", "June", "July", "August", "September",
   "October", "November", "December"]

/-- `formatDate(epochMs, utcOffset, style)`: `Sep 26, 2026` (`medium`) or
`September 2026` (`month-year`), `""` when invalid. -/
def formatDate (epoch offset : F64) (monthYear : Bool) : String :=
  match wallMs epoch offset with
  | .none => ""
  | .some wall =>
    let (year, month, day) := civil (floorInt (wall / 86400000))
    let name := months.getD (month - 1) ""
    let head := if monthYear then name else String.ofList (name.toList.take 3) ++ " " ++ toString day ++ ","
    head ++ " " ++ toString year

/-- `formatDate(epochMs, utcOffset, "iso")`: `YYYY-MM-DD`, the date part of
`toISOString` at that wall time (LLP 1102 §3.4), `""` when invalid. Years
are 1–9999 (`wallMs`), so four digits. -/
def formatIsoDate (epoch offset : F64) : String :=
  match wallMs epoch offset with
  | .none => ""
  | .some wall =>
    let (year, month, day) := civil (floorInt (wall / 86400000))
    let pad (n w : Nat) : String := let s := toString n; String.ofList (zeros (w - s.length)) ++ s
    pad year.toNat 4 ++ "-" ++ pad month 2 ++ "-" ++ pad day 2

/-! ## Localized text -/

/-- The strings tables: (locale, (key, text)…), the base first. -/
abbrev Tables := List (String × List (String × String))

def lookupText (key : String) : List (String × String) → Option String
  | [] => .none
  | (k, t) :: rest => if k == key then .some t else lookupText key rest

/-- `Plan::localized`: `key`'s text in the table named `locale`, else in
the base. -/
def localized (tables : Tables) (locale key : String) : Option String :=
  match (tables.find? (·.1 == locale)).bind (lookupText key ·.2) with
  | .some t => .some t
  | .none => tables.head?.bind (lookupText key ·.2)

/-- A placeholder's value: the first name/value pair named `name`, the
pairs read two by two (`chunks_exact(2)`). -/
def pairValue (name : String) : List (Option String) → Option String
  | n :: v :: rest => if n == .some name then v else pairValue name rest
  | _ => .none

/-- `strings::fill` of a validated message: text as written, `\{`, `\}`
and `\\` as the brace or backslash, `{$name}` or `{name}` as the value its
pair gives (the placeholder's own spelling when none does). -/
def fill (value : String → Option String) : Nat → List Char → List Char → List Char
  | 0, _, acc => acc
  | _ + 1, [], acc => acc
  | f + 1, '\\' :: c :: rest, acc => fill value f rest (acc ++ [c])
  | f + 1, '{' :: rest, acc =>
    let inner := rest.takeWhile (· != '}')
    let after := (rest.dropWhile (· != '}')).drop 1
    let raw := '{' :: inner ++ ['}']
    let e := ((inner.dropWhile Char.isWhitespace).reverse.dropWhile Char.isWhitespace).reverse
    let name := match e with | '$' :: r => r | r => r
    fill value f after (acc ++ ((value (String.ofList name)).map String.toList).getD raw)
  | f + 1, c :: rest, acc => fill value f rest (acc ++ [c])

/-- `t(locale, key, pairs)`: the text `localized` finds, filled. -/
def text (tables : Tables) (locale key : String) (pairs : List (Option String)) : Option String :=
  (localized tables locale key).map fun t =>
    String.ofList (fill (pairValue · pairs) (t.length + 1) t.toList [])

end Contract.Format
