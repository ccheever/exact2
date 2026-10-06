/-
Values and the roster's pure functions over them.

A record value is its fields in declaration order, as in the runner: two
records are equal when their fields are, whatever shape names them.
-/
import Contract.Syntax
import Contract.Number

namespace Contract

inductive Value where
  | num (f : F64)
  | bool (b : Bool)
  | str (s : String)
  | unit
  | none
  | some (v : Value)
  | list (xs : List Value)
  /-- A record: the shape it was built as (for field access by name and
  printing; never compared) and its fields in declaration order. -/
  | record (shape : String) (fields : List Value)
  deriving Repr, Inhabited

/-- Why an evaluation has no value. `pending` is not a failure: a derive or
resource read before it settled, which settlement retries. -/
inductive Err where
  | type (what : String)
  | pending
  | unbound (name : String)
  | unsupported (what : String)
  | refused (why : String)
  deriving Repr, Inhabited, BEq

abbrev Result := Except Err

namespace Value

mutual
/-- Structural equality as the runner's `compare::equal`: numbers by IEEE
`==` (so `NaN ≠ NaN`, `-0 = 0`), lists and records item by item stopping at
the first unequal pair; `none` (`Option.none`) when the two are not of one
type. -/
def equal : Value → Value → Option Bool
  | .num a, .num b => Option.some (a == b)
  | .bool a, .bool b => Option.some (a == b)
  | .str a, .str b => Option.some (a == b)
  | .unit, .unit => Option.some true
  | .none, .none => Option.some true
  | .some _, .none => Option.some false
  | .none, .some _ => Option.some false
  | .some a, .some b => equal a b
  | .list xs, .list ys => if xs.length != ys.length then Option.some false else equalItems xs ys
  | .record _ xs, .record _ ys =>
    if xs.length != ys.length then Option.some false else equalItems xs ys
  | _, _ => Option.none
/-- Item by item, stopping at the first unequal pair. -/
def equalItems : List Value → List Value → Option Bool
  | [], _ => Option.some true
  | _, [] => Option.some true
  | x :: xs, y :: ys =>
    match equal x y with
    | Option.none => Option.none
    | Option.some false => Option.some false
    | Option.some true => equalItems xs ys
end

def asNum : Value → Result F64
  | .num f => .ok f
  | _ => .error (.type "number")

def asBool : Value → Result Bool
  | .bool b => .ok b
  | _ => .error (.type "bool")

def asStr : Value → Result String
  | .str s => .ok s
  | _ => .error (.type "string")

def asList : Value → Result (List Value)
  | .list xs => .ok xs
  | _ => .error (.type "list")

/-- `Array.prototype.includes`'s SameValueZero on what `includes` takes of a
list's items (LLP 1088 §9.1): numbers by IEEE `==` with NaN equal to NaN
(`-0` is `0`), strings and bools by value. -/
def sameValueZero : Value → Value → Bool
  | .num a, .num b => a == b || (Number.isNaN a && Number.isNaN b)
  | .str a, .str b => a == b
  | .bool a, .bool b => a == b
  | _, _ => false

/-- `Array.prototype.indexOf`'s IsStrictlyEqual on what `indexOf` takes of a
list's items (LLP 1088 §9.1): numbers by IEEE `==` (NaN is never found,
`-0` is `0`), strings and bools by value. -/
def strictEq : Value → Value → Bool
  | .num a, .num b => a == b
  | .str a, .str b => a == b
  | .bool a, .bool b => a == b
  | _, _ => false

end Value

namespace Str

/-- The web's `String.length`: UTF-16 code units. -/
def utf16Length (s : String) : Nat :=
  s.foldl (fun n c => n + if c.toNat > 0xFFFF then 2 else 1) 0

/-- UTF-8 byte length (the runner's string budget counts bytes). -/
def utf8Length (s : String) : Nat := s.utf8ByteSize

/-- ECMA-262 WhiteSpace and LineTerminator: what `trim` strips. -/
def isJsSpace (c : Char) : Bool :=
  let n := c.toNat
  (0x9 ≤ n && n ≤ 0xd) || n == 0x20 || n == 0xa0 || n == 0x1680 ||
  (0x2000 ≤ n && n ≤ 0x200a) || n == 0x2028 || n == 0x2029 || n == 0x202f ||
  n == 0x205f || n == 0x3000 || n == 0xfeff

def trim (s : String) : String :=
  let cs := s.toList
  let cs := cs.dropWhile isJsSpace
  let cs := (cs.reverse.dropWhile isJsSpace).reverse
  String.ofList cs

def isPrefix : List Char → List Char → Bool
  | [], _ => true
  | _, [] => false
  | a :: as, b :: bs => a == b && isPrefix as bs

/-- `haystack.includes(needle)`: literal and case-sensitive; the empty
needle is found. Over scalar values, which answers as UTF-16 search does
for well-formed text. -/
def includes (hay needle : String) : Bool :=
  let n := needle.toList
  let rec go : List Char → Bool
    | [] => isPrefix n []
    | h :: t => isPrefix n (h :: t) || go t
  go hay.toList

def startsWith (s p : String) : Bool := isPrefix p.toList s.toList

def endsWith (s p : String) : Bool := isPrefix p.toList.reverse s.toList.reverse

/-! The web's strings as JavaScript holds them, UTF-16 code units (LLP 1088
D1, D2): their order, and `slice` and `replaceAll` over them, each result
made well formed once (`toWellFormed`). -/

/-- A string's UTF-16 code units. -/
def utf16Units (s : String) : List Nat :=
  s.toList.flatMap fun c =>
    if c.toNat > 0xFFFF then
      [0xD800 + (c.toNat - 0x10000) / 0x400, 0xDC00 + (c.toNat - 0x10000) % 0x400]
    else [c.toNat]

def isHigh (u : Nat) : Bool := 0xD800 ≤ u && u ≤ 0xDBFF
def isLow (u : Nat) : Bool := 0xDC00 ≤ u && u ≤ 0xDFFF

/-- One code unit that pairs with nothing: a lone half is U+FFFD. -/
def lone (u : Nat) : Char := if isHigh u || isLow u then '\uFFFD' else Char.ofNat u

/-- Code units as scalar values, well formed: a high half and the low half
after it are their pair's scalar, any other half U+FFFD. -/
def wellFormed : List Nat → List Char
  | [] => []
  | [u] => [lone u]
  | u :: l :: rest =>
    if isHigh u && isLow l then Char.ofNat (0x10000 + (u - 0xD800) * 0x400 + (l - 0xDC00)) :: wellFormed rest
    else lone u :: wellFormed (l :: rest)

def ofUnits (us : List Nat) : String := String.ofList (wellFormed us)

/-- JavaScript's `IsLessThan` for two Strings: code units in order, a proper
prefix first. -/
def unitsLt : List Nat → List Nat → Bool
  | [], [] => false
  | [], _ :: _ => true
  | _ :: _, [] => false
  | a :: as, b :: bs => a < b || (a == b && unitsLt as bs)

def lt (a b : String) : Bool := unitsLt (utf16Units a) (utf16Units b)

/-- ToIntegerOrInfinity, then `slice`'s clamp to `0..len`: NaN is 0, a
fraction truncates toward zero, a negative index counts from the end. -/
def clampIndex (i : F64) (len : Nat) : Nat :=
  let i := if Number.isNaN i then 0 else Number.trunc i
  let l := F64.ofNat len
  if i < 0 then (if l + i < 0 then 0 else (l + i).toNat)
  else if i < l then i.toNat else len

/-- `String.prototype.slice(start, end)`, well formed. -/
def slice (s : String) (a b : F64) : String :=
  let us := utf16Units s
  let f := clampIndex a us.length
  let t := clampIndex b us.length
  ofUnits ((us.drop f).take (t - f))

/-- ECMA-262's `GetSubstitution` for a string pattern matched at `p`: `$$`
is `$`, `$&` the match, `` $` `` what precedes it, `$'` what follows; any
other `$` is itself. -/
def substitute (str find : List Nat) (p : Nat) : List Nat → List Nat
  | 36 :: 36 :: rest => 36 :: substitute str find p rest
  | 36 :: 38 :: rest => find ++ substitute str find p rest
  | 36 :: 96 :: rest => str.take p ++ substitute str find p rest
  | 36 :: 39 :: rest => str.drop (p + find.length) ++ substitute str find p rest
  | u :: rest => u :: substitute str find p rest
  | [] => []

def isPrefixN : List Nat → List Nat → Bool
  | [], _ => true
  | _, [] => false
  | a :: as, b :: bs => a == b && isPrefixN as bs

/-- `replaceAll`'s units from position `p`, where `rest` is what is left of
the string: every match left to right, none overlapping; an empty `find`
matches at every code-unit boundary. Each step consumes a unit or a match,
so the string's length bounds the steps (`fuel`). -/
def replaceFrom (str find w : List Nat) : Nat → Nat → List Nat → List Nat
  | 0, _, _ => []
  | fuel + 1, p, rest =>
    if find.isEmpty then
      substitute str find p w ++ match rest with
        | [] => []
        | u :: us => u :: replaceFrom str find w fuel (p + 1) us
    else if isPrefixN find rest then
      substitute str find p w ++ replaceFrom str find w fuel (p + find.length) (rest.drop find.length)
    else match rest with
      | [] => []
      | u :: us => u :: replaceFrom str find w fuel (p + 1) us

/-- Where `needle`'s units first match from position `p` of `rest`; the
empty needle at once. -/
def indexFrom (needle : List Nat) : Nat → List Nat → Option Nat
  | p, [] => if needle.isEmpty then .some p else .none
  | p, u :: us => if isPrefixN needle (u :: us) then .some p else indexFrom needle (p + 1) us

/-- `String.prototype.indexOf(needle)` (LLP 1088 §9.1): the first match's
position in UTF-16 code units. -/
def indexOf (hay needle : String) : Option Nat :=
  indexFrom (utf16Units needle) 0 (utf16Units hay)

/-- `split`'s pieces of `rest`, `cur` the units of the piece so far: a
match of `sep` (not empty) ends a piece. Each step consumes a unit or a
match, so the string's length bounds the steps (`fuel`). -/
def splitFrom (sep : List Nat) : Nat → List Nat → List Nat → List (List Nat)
  | 0, cur, _ => [cur]
  | fuel + 1, cur, rest =>
    if isPrefixN sep rest then cur :: splitFrom sep fuel [] (rest.drop sep.length)
    else match rest with
      | [] => [cur]
      | u :: us => splitFrom sep fuel (cur ++ [u]) us

/-- `String.prototype.split(sep)` with a string `sep` (LLP 1088 §9.1), each
piece well formed: an empty `sep` splits into code units, `""` into none. -/
def split (s sep : String) : List String :=
  let us := utf16Units s
  let sp := utf16Units sep
  if sp.isEmpty then us.map fun u => ofUnits [u]
  else (splitFrom sp (us.length + 1) [] us).map ofUnits

/-- `String.prototype.replaceAll(find, with)` with a string `find`, well
formed. The runner's `MAX_STRING` bound is not modelled. -/
def replaceAll (s find w : String) : String :=
  let us := utf16Units s
  ofUnits (replaceFrom us (utf16Units find) (utf16Units w) (us.length + 2) 0 us)

/-! ### Numbers and dates in text (LLP 1102 §3.1, §3.4) -/

/-- The ASCII digits at the front, and the rest. -/
def digitRun : List Char → List Char × List Char
  | c :: cs => if c.isDigit then let (d, r) := digitRun cs; (c :: d, r) else ([], c :: cs)
  | [] => ([], [])

def digitsValue (ds : List Char) : Nat := ds.foldl (fun n c => n * 10 + (c.toNat - '0'.toNat)) 0

/-- An optional sign: whether it is `-`, and the rest. -/
def signRun : List Char → Bool × List Char
  | '-' :: cs => (true, cs)
  | '+' :: cs => (false, cs)
  | cs => (false, cs)

/-- The exponent part: nothing, or `e`/`E`, a sign and digits ending the text. -/
def exponent : List Char → Option Int
  | [] => .some 0
  | c :: cs =>
    if c = 'e' ∨ c = 'E' then
      let (neg, cs) := signRun cs
      let (ds, rest) := digitRun cs
      if ds.isEmpty || !rest.isEmpty then .none
      else .some (if neg then -(digitsValue ds : Int) else digitsValue ds)
    else .none

/-- `parseNumber(text)`: the text, trimmed as `trim` does, must be
`[+-]? (digits ('.' digits?)? | '.' digits) ([eE] [+-]? digits)?`; its
value rounded to the nearest double as `Number(text)` rounds it, and none
when that is infinite or a zero from nonzero digits. `-0` is `-0`. The value
`d * 10^k` is decided before any power is taken: past `10^310` it overflows
and under `10^-330` it underflows, whatever its digits. -/
def parseNumber (s : String) : Option F64 :=
  let (neg, cs) := signRun (trim s).toList
  let (whole, cs) := digitRun cs
  let (frac, cs) := match cs with
    | '.' :: r => digitRun r
    | r => ([], r)
  if whole.isEmpty && frac.isEmpty then .none
  else match exponent cs with
    | .none => .none
    | .some e =>
      let d := digitsValue (whole ++ frac)
      let k : Int := e - frac.length
      if d = 0 then .some (F64.make neg 0 0)
      else
        let n : Int := (toString d).length
        if n + k > 310 || n + k < -330 then .none
        else
          let x := if 0 ≤ k then F64.round neg (d * 10 ^ k.toNat * 2 ^ 1074) 1
            else F64.round neg (d * 2 ^ 1074) (10 ^ (-k).toNat)
          if x.isFinite && x.mag != 0 then .some x else .none

def isLeap (y : Nat) : Bool := y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)

def monthDays (y m : Nat) : Nat :=
  if m = 2 then (if isLeap y then 29 else 28)
  else if m = 4 ∨ m = 6 ∨ m = 9 ∨ m = 11 then 30 else 31

/-- A `YYYY-MM-DD` date that exists (proleptic Gregorian, year 0000–9999). -/
def isoDate (s : String) : Option (Nat × Nat × Nat) :=
  match s.toList with
  | [y1, y2, y3, y4, '-', m1, m2, '-', d1, d2] =>
    let ds := [y1, y2, y3, y4, m1, m2, d1, d2]
    if !ds.all Char.isDigit then .none
    else
      let y := digitsValue [y1, y2, y3, y4]
      let m := digitsValue [m1, m2]
      let d := digitsValue [d1, d2]
      if 1 ≤ m ∧ m ≤ 12 ∧ 1 ≤ d ∧ d ≤ monthDays y m then .some (y, m, d) else .none
  | _ => .none

/-- `a` is earlier than `b`, as dates. -/
def dateLt (a b : Nat × Nat × Nat) : Bool :=
  a.1 < b.1 || (a.1 == b.1 && (a.2.1 < b.2.1 || (a.2.1 == b.2.1 && a.2.2 < b.2.2)))

/-- The whole years or months from `a` to a date `b` not before it: a period
completes when `b`'s month and day reach `a`'s (years compare month then
day, months the day), so Feb 29 completes a year on Mar 1 of a common year, as Temporal's
`PlainDate.until` counts with `largestUnit` years or months. -/
def periods (a b : Nat × Nat × Nat) (months : Bool) : Int :=
  let (y1, m1, d1) := a
  let (y2, m2, d2) := b
  if months then ((y2 : Int) - y1) * 12 + ((m2 : Int) - m1) - (if d2 < d1 then 1 else 0)
  else ((y2 : Int) - y1) - (if dateLt (0, m2, d2) (0, m1, d1) then 1 else 0)

/-- `calendarDiff(from, to, unit)`: the periods from `from` to `to`, or,
when `to` is earlier, those from `to` to `from` negated; none when either
is not a `YYYY-MM-DD` date. -/
def calendarDiff (a b : String) (months : Bool) : Option Int :=
  match isoDate a, isoDate b with
  | .some x, .some y => .some (if dateLt y x then -periods y x months else periods x y months)
  | _, _ => .none

end Str

/-- `toString(v)` and template interpolation: numbers as JavaScript prints
them, bools as `true`/`false`, strings as themselves. -/
def Value.display : Value → Result String
  | .num f => .ok (Number.jsToString f)
  | .bool b => .ok (if b then "true" else "false")
  | .str s => .ok s
  | _ => .error (.type "toString of a value that is not a number, bool or string")

end Contract
