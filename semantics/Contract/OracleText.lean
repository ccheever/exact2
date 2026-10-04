/-
An oracle as text: a differential run's transcript can be megabytes, which
Lean elaborates slowly as a term and parses quickly as a string.

  answer <source> <n> <arg>… <value>   one call and its answer
  fact <resource> <value>              a host fact, by resource

A value: `n` and 16 hex digits of IEEE bits; a quoted string (`\"`, `\\`,
`\n`, `\r`, `\t`, `\u00xx`); `t`, `f`, `u` (unit); `none`; `some(v)`;
`[v,…]`; `R"Shape"{v,…}` for a record. Names are quoted strings.
-/
import Contract.Runtime

namespace Contract.OracleText

abbrev P (α : Type) := List Char → Option (α × List Char)

def hexVal (c : Char) : Option Nat :=
  if '0' ≤ c && c ≤ '9' then .some (c.toNat - '0'.toNat)
  else if 'a' ≤ c && c ≤ 'f' then .some (c.toNat - 'a'.toNat + 10)
  else .none

def hexN : Nat → Nat → P Nat
  | 0, acc, cs => .some (acc, cs)
  | k + 1, acc, c :: cs => (hexVal c).bind fun d => hexN k (acc * 16 + d) cs
  | _ + 1, _, [] => .none

/-- A quoted string, its opening quote already consumed. -/
def strBody : Nat → List Char → P String
  | 0, _, _ => .none
  | _ + 1, acc, '"' :: cs => .some (String.ofList acc.reverse, cs)
  | f + 1, acc, '\\' :: c :: cs =>
    match c with
    | '"' => strBody f ('"' :: acc) cs
    | '\\' => strBody f ('\\' :: acc) cs
    | 'n' => strBody f ('\n' :: acc) cs
    | 'r' => strBody f ('\r' :: acc) cs
    | 't' => strBody f ('\t' :: acc) cs
    | 'u' => (hexN 4 0 cs).bind fun (n, rest) => strBody f (Char.ofNat n :: acc) rest
    | _ => .none
  | f + 1, acc, c :: cs => strBody f (c :: acc) cs
  | _ + 1, _, [] => .none

def str : P String
  | '"' :: cs => strBody (cs.length + 1) [] cs
  | _ => .none

def skipSpace : List Char → List Char
  | ' ' :: cs => skipSpace cs
  | '\n' :: cs => skipSpace cs
  | cs => cs

def literal (w : String) (cs : List Char) : Option (List Char) :=
  let ws := w.toList
  if ws.isPrefixOf cs then .some (cs.drop ws.length) else .none

mutual
/-- One value; `fuel` bounds the nesting and the list lengths. -/
def value : Nat → P Value
  | 0, _ => .none
  | f + 1, cs =>
    match cs with
    | 'n' :: 'o' :: 'n' :: 'e' :: rest => .some (.none, rest)
    | 'n' :: rest => (hexN 16 0 rest).map fun (b, r) => (.num (Float.ofBits (UInt64.ofNat b)), r)
    | '"' :: _ => (str cs).map fun (s, r) => (.str s, r)
    | 't' :: rest => .some (.bool true, rest)
    | 'f' :: rest => .some (.bool false, rest)
    | 'u' :: rest => .some (.unit, rest)
    | 's' :: 'o' :: 'm' :: 'e' :: '(' :: rest => do
      let (v, r) ← value f rest
      let r ← literal ")" r
      pure (.some v, r)
    | '[' :: ']' :: rest => .some (.list [], rest)
    | '[' :: rest => do
      let (vs, r) ← items f rest ']'
      pure (.list vs, r)
    | 'R' :: rest => do
      let (shape, r) ← str rest
      let r ← literal "{" r
      match r with
      | '}' :: r => pure (.record shape [], r)
      | _ => do
        let (vs, r) ← items f r '}'
        pure (.record shape vs, r)
    | _ => .none

/-- Values separated by `,` up to `close`. -/
def items : Nat → List Char → Char → Option (List Value × List Char)
  | 0, _, _ => .none
  | f + 1, cs, close => do
    let (v, r) ← value f cs
    match r with
    | ',' :: r => do
      let (vs, r) ← items f r close
      pure (v :: vs, r)
    | c :: r => if c == close then pure ([v], r) else .none
    | [] => .none
end

def natLit : Nat → Nat → P Nat
  | 0, acc, cs => .some (acc, cs)
  | k + 1, acc, c :: cs =>
    if '0' ≤ c && c ≤ '9' then natLit k (acc * 10 + (c.toNat - '0'.toNat)) cs
    else .some (acc, c :: cs)
  | _ + 1, acc, [] => .some (acc, [])

def values (fuel : Nat) : Nat → P (List Value)
  | 0, cs => .some ([], cs)
  | k + 1, cs => do
    let (v, r) ← value fuel (skipSpace cs)
    let (vs, r) ← values fuel k r
    pure (v :: vs, r)

/-- Parse a whole oracle; `none` on malformed text. -/
def parse (text : String) : Option Oracle := Id.run do
  let mut o : Oracle := {}
  let mut answers : Array (String × List Value × Value) := #[]
  let mut facts : Array (String × Value) := #[]
  for line in text.splitOn "\n" do
    let cs := line.toList
    let fuel := cs.length + 1
    if cs.isEmpty then continue
    match literal "answer " cs with
    | .some r =>
      match (do
        let (src, r) ← str r
        let (n, r) ← natLit 10 0 (skipSpace r)
        let (args, r) ← values fuel n r
        let (v, _) ← value fuel (skipSpace r)
        pure (src, args, v)) with
      | .some a => answers := answers.push a
      | .none => return .none
    | .none =>
      match literal "fact " cs with
      | .some r =>
        match (do
          let (name, r) ← str r
          let (v, _) ← value fuel (skipSpace r)
          pure (name, v)) with
        | .some fct => facts := facts.push fct
        | .none => return .none
      | .none => return .none
  o := { answers := answers.toList, facts := facts.toList }
  return .some o

/-- [`parse`], or an oracle that answers nothing (every call refused,
which a comparison reports). -/
def parse! (text : String) : Oracle := (parse text).getD {}

end Contract.OracleText
