/-
Values and the roster's pure functions over them.

A record value is its fields in declaration order, as in the runner: two
records are equal when their fields are, whatever shape names them.
-/
import Contract.Syntax
import Contract.Number

namespace Contract

inductive Value where
  | num (f : Float)
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

def asNum : Value → Result Float
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

end Str

/-- `toString(v)` and template interpolation: numbers as JavaScript prints
them, bools as `true`/`false`, strings as themselves. -/
def Value.display : Value → Result String
  | .num f => .ok (Number.jsToString f)
  | .bool b => .ok (if b then "true" else "false")
  | .str s => .ok s
  | _ => .error (.type "toString of a value that is not a number, bool or string")

end Contract
