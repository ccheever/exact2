/-
The router (LLP 1038): a location, a retained stack per tab, and six pure
verbs over a route table.

This is `route/src` (the `exact_route` crate) transcribed: `location.rs`'s
canonical path and query, `table.rs`'s matching, chains and `path`, and
`router.rs`'s verbs and reads; and the runner's boundary,
`runner/src/runner/router.rs`, which reads a router out of a plan value
(validating it) and writes one back. A refused verb answers its input
value unchanged; the runner journals the refusal, which no observation
shows.

The table is the one the compiler checked (`exact_route::Table::check` in
`contract/types`): every program the semantics is given has a valid one,
so `open`'s first-launch check, which can only pass, is left out.

Strings are worked on as lists of characters and, where the crate works on
bytes (percent-encoding and -decoding), as their UTF-8 bytes.
-/
import Contract.Syntax
import Contract.Value

namespace Contract

/-- `encodeURIComponent`: UTF-8 bytes, all but the unreserved marks
percent-encoded in uppercase hex. -/
def encodeURIComponent (s : String) : String :=
  let keep (b : UInt8) : Bool :=
    let c := b.toNat
    (0x41 ≤ c && c ≤ 0x5a) || (0x61 ≤ c && c ≤ 0x7a) || (0x30 ≤ c && c ≤ 0x39) ||
    "-_.!~*'()".toList.any (·.toNat == c)
  let hex := "0123456789ABCDEF".toList
  s.toUTF8.toList.foldl (fun acc b =>
    if keep b then acc.push (Char.ofNat b.toNat)
    else acc.push '%' |>.push (hex.getD (b.toNat / 16) '0') |>.push (hex.getD (b.toNat % 16) '0')) ""

namespace Route

abbrev Table := List RouteDecl

/-- A parameter record: every table parameter, in first-declaration order,
unbound names `""`. -/
abbrev Params := List (String × String)

structure Entry where
  id : Nat
  name : String
  url : String
  tab : String
  params : Params
  deriving Inhabited

structure Tab where
  name : String
  stack : List Entry
  deriving Inhabited

structure Router where
  tab : String := ""
  tabs : List Tab := []
  next : Nat := 0
  deriving Inhabited

/-- An entry before it has a visit id. -/
structure Destination where
  name : String
  url : String
  tab : String
  params : Params
  deriving Inhabited

/-! ## Strings -/

/-- Rust's `str::split` on one character: `""` is `[""]`. -/
def splitC (c : Char) (s : List Char) : List (List Char) :=
  s.foldr (fun x acc =>
    if x == c then [] :: acc
    else match acc with
      | h :: t => (x :: h) :: t
      | [] => [[x]]) [[]]

def split (s : String) (c : Char) : List String := (splitC c s.toList).map String.ofList

/-- `split(c).next()`: everything before the first `c`. -/
def before (s : String) (c : Char) : String := ((split s c).head?).getD ""

/-- `split_once(c)`. -/
def splitOnce (s : String) (c : Char) : Option (String × String) :=
  let rec go : List Char → Option (List Char × List Char)
    | [] => .none
    | x :: xs => if x == c then .some ([], xs) else (go xs).map fun (a, b) => (x :: a, b)
  (go s.toList).map fun (a, b) => (String.ofList a, String.ofList b)

def startsWith (s : String) (c : Char) : Bool := s.toList.head? == .some c
def endsWith (s : String) (c : Char) : Bool := s.toList.getLast? == .some c
def dropFirst (s : String) : String := String.ofList s.toList.tail

/-- `strip_prefix(':')`. -/
def paramName? (segment : String) : Option String :=
  if startsWith segment ':' then .some (dropFirst segment) else .none

def asciiLower (s : String) : String :=
  String.ofList (s.toList.map fun c => if 'A' ≤ c && c ≤ 'Z' then Char.ofNat (c.toNat + 32) else c)

def hexDigit (n : Nat) : Char := "0123456789ABCDEF".toList.getD n '0'

/-- Percent-encode the UTF-8 bytes `escape` names (every byte it keeps is
ASCII). -/
def encode (s : String) (escape : Nat → Bool) : String :=
  s.toUTF8.toList.foldl (fun acc b =>
    let n := b.toNat
    if escape n then ((acc.push '%').push (hexDigit (n / 16))).push (hexDigit (n % 16))
    else acc.push (Char.ofNat n)) ""

/-- The path percent-encode set (WHATWG URL), plus `^` `|`. -/
def pathEscape (b : Nat) : Bool :=
  !(0x21 ≤ b && b ≤ 0x7e) || [0x22, 0x23, 0x3c, 0x3e, 0x3f, 0x5e, 0x60, 0x7b, 0x7d, 0x7c].contains b

/-- The special-query percent-encode set (it includes `'`). -/
def queryEscape (b : Nat) : Bool :=
  !(0x21 ≤ b && b ≤ 0x7e) || [0x22, 0x23, 0x3c, 0x3e, 0x27].contains b

def hexVal (b : UInt8) : Option Nat :=
  let n := b.toNat
  if 0x30 ≤ n && n ≤ 0x39 then .some (n - 0x30)
  else if 0x61 ≤ n && n ≤ 0x66 then .some (n - 0x61 + 10)
  else if 0x41 ≤ n && n ≤ 0x46 then .some (n - 0x41 + 10)
  else .none

/-- Percent-decoding over bytes; `plus` reads `+` as a space. A `%` not
followed by two hex digits is itself. -/
partial def decodeBytes (plus : Bool) : List UInt8 → List UInt8
  | [] => []
  | b :: rest =>
    if b == 0x25 then
      match rest with
      | h :: l :: r2 =>
        match hexVal h, hexVal l with
        | .some x, .some y => UInt8.ofNat (x * 16 + y) :: decodeBytes plus r2
        | _, _ => b :: decodeBytes plus rest
      | _ => b :: decodeBytes plus rest
    else (if plus && b == 0x2b then 0x20 else b) :: decodeBytes plus rest

def isCont (b : UInt8) : Bool := 0x80 ≤ b && b ≤ 0xbf

/-- `String::from_utf8_lossy`: each maximal invalid subpart is one U+FFFD. -/
partial def lossy : List UInt8 → List Char
  | [] => []
  | b :: rest =>
    let bad := '�'
    let n := b.toNat
    if n < 0x80 then Char.ofNat n :: lossy rest
    else if 0xc2 ≤ n && n ≤ 0xdf then
      match rest with
      | c :: r2 =>
        if isCont c then Char.ofNat (((n &&& 0x1f) <<< 6) ||| (c.toNat &&& 0x3f)) :: lossy r2
        else bad :: lossy rest
      | [] => [bad]
    else if 0xe0 ≤ n && n ≤ 0xef then
      let (lo, hi) : UInt8 × UInt8 :=
        if n == 0xe0 then (0xa0, 0xbf) else if n == 0xed then (0x80, 0x9f) else (0x80, 0xbf)
      match rest with
      | c1 :: r1 =>
        if lo ≤ c1 && c1 ≤ hi then
          match r1 with
          | c2 :: r2 =>
            if isCont c2 then
              Char.ofNat (((n &&& 0x0f) <<< 12) ||| ((c1.toNat &&& 0x3f) <<< 6) ||| (c2.toNat &&& 0x3f))
                :: lossy r2
            else bad :: lossy r1
          | [] => [bad]
        else bad :: lossy rest
      | [] => [bad]
    else if 0xf0 ≤ n && n ≤ 0xf4 then
      let (lo, hi) : UInt8 × UInt8 :=
        if n == 0xf0 then (0x90, 0xbf) else if n == 0xf4 then (0x80, 0x8f) else (0x80, 0xbf)
      match rest with
      | c1 :: r1 =>
        if lo ≤ c1 && c1 ≤ hi then
          match r1 with
          | c2 :: r2 =>
            if isCont c2 then
              match r2 with
              | c3 :: r3 =>
                if isCont c3 then
                  Char.ofNat (((n &&& 0x07) <<< 18) ||| ((c1.toNat &&& 0x3f) <<< 12) |||
                    ((c2.toNat &&& 0x3f) <<< 6) ||| (c3.toNat &&& 0x3f)) :: lossy r3
                else bad :: lossy r2
              | [] => [bad]
            else bad :: lossy r1
          | [] => [bad]
        else bad :: lossy rest
      | [] => [bad]
    else bad :: lossy rest

def decode (s : String) (plus : Bool) : String :=
  String.ofList (lossy (decodeBytes plus s.toUTF8.toList))

/-! ## Locations (`location.rs`) -/

/-- Trim C0 controls and spaces at both ends; drop tabs and newlines. -/
def clean (s : String) : String :=
  let ws (c : Char) : Bool := c.toNat ≤ 0x20
  let cs := ((s.toList.dropWhile ws).reverse.dropWhile ws).reverse
  String.ofList (cs.filter fun c => c != '\t' && c != '\n' && c != '\r')

/-- The browser's `pathname + search`: dot segments resolved, the path and
query encode sets applied, existing escapes kept, the fragment dropped. -/
def pathQuery (input : String) : String :=
  let input := before input '#'
  let (path, query) := (splitOnce input '?').getD (input, "")
  let path := String.ofList (path.toList.map fun c => if c == '\\' then '/' else c)
  let path := if startsWith path '/' then dropFirst path else path
  let rec go : List String → List String → List String
    | [], acc => acc
    | s :: rest, acc =>
      let dot := asciiLower s
      if dot == "." || dot == "%2e" then go rest (if rest.isEmpty then acc ++ [""] else acc)
      else if dot == ".." || dot == ".%2e" || dot == "%2e." || dot == "%2e%2e" then
        let acc := acc.dropLast
        go rest (if rest.isEmpty then acc ++ [""] else acc)
      else go rest (acc ++ [encode s pathEscape])
  let segs := go (split path '/') []
  "/" ++ "/".intercalate segs ++ (if query.isEmpty then "" else "?" ++ encode query queryEscape)

/-- A location's canonical form: a missing leading slash prefixed. -/
def canonical (location : String) : String :=
  let absolute := if startsWith location '/' then location else "/" ++ location
  pathQuery (clean absolute)

/-- `URLSearchParams.get` on an entry's URL: the first occurrence, form
decoded, `""` when absent. -/
def searchParam (url name : String) : String :=
  match splitOnce url '?' with
  | .none => ""
  | .some (_, query) =>
    let pairs := (split (before query '#') '&').filter (!·.isEmpty)
    let rec find : List String → String
      | [] => ""
      | pair :: rest =>
        let (key, value) := (splitOnce pair '=').getD (pair, "")
        if decode key true == name then decode value true else find rest
    find pairs

/-! ## Parameters -/

def Params.get (ps : Params) (name : String) : Option String :=
  (ps.find? (·.1 == name)).map (·.2)

def Params.insert (ps : Params) (name value : String) : Params :=
  if ps.any (·.1 == name) then ps.map fun (k, v) => if k == name then (k, value) else (k, v)
  else ps ++ [(name, value)]

/-- Equal as maps from name to value. -/
def Params.eq (a b : Params) : Bool :=
  a.length == b.length && a.all fun (k, v) => Params.get b k == .some v

/-! ## The table (`table.rs`) -/

def namesIn (pattern : String) : List String := (split pattern '/').filterMap paramName?

def segments (path : String) : List String :=
  if path == "/" then [] else split (if startsWith path '/' then dropFirst path else path) '/'

/-- Parameter names in first-declaration order, each once: the field order
of the plan's `Params` record. -/
def paramNames (t : Table) : List String :=
  t.foldl (fun acc r =>
    if r.notfound then acc
    else (namesIn r.pattern).foldl (fun acc n => if acc.contains n then acc else acc ++ [n]) acc) []

def emptyParams (t : Table) : Params := (paramNames t).map (·, "")

/-- The first declared pattern that matches a canonical URL, with the
decoded parameters. A trailing slash (but `/`) matches none. -/
def matchPatternIndex (t : Table) (url : String) : Option (Nat × Params) :=
  let path := before url '?'
  let parts := segments path
  if path == "/" || !endsWith path '/' then
    let rec go : Nat → List RouteDecl → Option (Nat × Params)
      | _, [] => .none
      | i, r :: rest =>
        if r.notfound then go (i + 1) rest else
        let pattern := segments (canonical r.pattern)
        if pattern.length != parts.length then go (i + 1) rest else
        let bound : Option Params := (pattern.zip parts).foldlM (m := Option) (fun (ps : Params) (seg, value) =>
          match paramName? seg with
          | Option.some name =>
            if value.isEmpty then Option.none else Option.some (Params.insert ps name (decode value false))
          | Option.none => if seg == value then Option.some ps else Option.none) (emptyParams t)
        match bound with
        | .some ps => .some (i, ps)
        | .none => go (i + 1) rest
    go 0 t
  else .none

/-- A declared pattern, else the first notfound row. -/
def matchIndex (t : Table) (url : String) : Option (Nat × Params) :=
  match matchPatternIndex t url with
  | .some m => .some m
  | .none => (t.findIdx? (·.notfound)).map (·, emptyParams t)

def matchName (t : Table) (location : String) : Option (String × Params) :=
  (matchIndex t (canonical location)).map fun (i, ps) => ((t[i]?.map (·.name)).getD "", ps)

/-- A path parameter: not empty, `.` or `..`, then `encodeURIComponent`. -/
def encodeRouteSegment (value : String) : Option String :=
  if value == "" || value == "." || value == ".." then .none else .some (encodeURIComponent value)

/-- `Table::path`: a named route formatted with its parameters. -/
def path (t : Table) (name : String) (params : List String) : Option String := do
  let route ← t.find? fun r => r.name == name && !r.notfound
  if (namesIn route.pattern).length != params.length then .none
  let rec go : Bool → List String → List String → Option String
    | _, [], _ => .some ""
    | first, seg :: segs, vs => do
      let sep := if first then "" else "/"
      let (piece, vs) ← if startsWith seg ':' then do
          let (v, vs) := match vs with | v :: vs => (v, vs) | [] => ("", [])
          pure (← encodeRouteSegment v, vs)
        else pure (seg, vs)
      let more ← go false segs vs
      pure (sep ++ piece ++ more)
  go true (split route.pattern '/') params

/-- Tab roots in declaration order: the `tab` rows, else the first row. -/
def roots (t : Table) : List Nat :=
  let rs := (t.zipIdx.filter (·.1.tab)).map (·.2)
  if rs.isEmpty && !t.isEmpty then [0] else rs

def tabNames (t : Table) : List String := (roots t).filterMap fun i => t[i]?.map (·.name)

/-- The tab root above a row (a notfound row's is the first tab's). -/
def rootFor (t : Table) (index : Nat) : Option Nat := do
  let rs := roots t
  let r ← t[index]?
  if !r.notfound then
    let rec walk : Nat → Option Nat → Option (Option Nat)
      | 0, _ => .some .none
      | _, .none => .some .none
      | n + 1, .some i =>
        if rs.contains i then .some (.some i)
        else match t[i]? with
          | .none => .none
          | .some row => walk n row.parent
    match ← walk t.length (.some index) with
    | .some i => return i
    | .none => pure ()
  rs.head?

/-- The declared root-to-leaf chain without ids, or `[]` on no match.
Only the leaf keeps the incoming URL; ancestors are formatted from their
own patterns. -/
def chain (t : Table) (location : String) : List Destination := Id.run do
  let url := canonical location
  let .some (index, params) := matchIndex t url | return []
  let .some root := rootFor t index | return []
  let .some leaf := t[index]? | return []
  let tab := ((t[root]?).map (·.name)).getD ""
  -- Up the parent links to the root (the crate's loop, bounded).
  let rec up : Nat → List Nat → Option Nat → Option (List Nat)
    | 0, acc, _ => .some acc
    | n + 1, acc, parent =>
      if (acc.getLast?.getD root) == root then .some acc else
      match parent with
      | .none => .some acc
      | .some p =>
        if acc.contains p then .none else
        match t[p]? with
        | .none => .none
        | .some r => up n (acc ++ [p]) r.parent
  let indices ←
    if leaf.notfound then pure [index]
    else match up (t.length + 1) [index] leaf.parent with
      | .some is => pure is
      | .none => return []
  let indices := (if indices.contains root then indices else indices ++ [root]).reverse
  return indices.filterMap fun i => (t[i]?).map fun r =>
    let entryUrl :=
      if i == index then url
      else
        let values := (namesIn r.pattern).map fun n => (Params.get params n).getD ""
        canonical ((path t r.name values).getD "")
    let own := (namesIn r.pattern).foldl (fun own n =>
      match Params.get params n with
      | .some v => Params.insert own n v
      | .none => own) (emptyParams t)
    { name := r.name, url := entryUrl, tab, params := own }

/-! ## The router (`router.rs`) -/

/-- The selected stack, root first. -/
def stack (r : Router) : List Entry :=
  ((r.tabs.find? (·.name == r.tab)).map (·.stack)).getD []

def top (r : Router) : Option Entry := (stack r).getLast?

def depth (r : Router) : Nat := (stack r).length

/-- A parameter's non-empty values over the selected stack. -/
def params (r : Router) (name : String) : List String :=
  ((stack r).filterMap (fun e => Params.get e.params name)).filter (!·.isEmpty)

abbrev Verb := Except String Router

def noMatch (location : String) : String := s!"no route matches {canonical location}"

def selected (r : Router) : Except String Nat :=
  match r.tabs.findIdx? (fun t => t.name == r.tab && !t.stack.isEmpty) with
  | .some i => .ok i
  | .none => .error "router has no selected stack"

/-- The largest id a plan number carries exactly, less one. -/
def idLimit : Nat := 2 ^ 53 - 1

def mint (r : Router) (d : Destination) : Except String (Entry × Router) :=
  if r.next ≥ idLimit then .error "router entry ids exhausted"
  else .ok ({ id := r.next, name := d.name, url := d.url, tab := d.tab, params := d.params },
            { r with next := r.next + 1 })

def setStack (r : Router) (i : Nat) (f : List Entry → List Entry) : Router :=
  { r with tabs := r.tabs.modify i fun t => { t with stack := f t.stack } }

/-- Select the declared tab and replace its chain: the same URL at the
same position keeps its id. Seeds every tab at its root first, when there
are none. -/
def «open» (t : Table) (r : Router) (location : String) : Verb := do
  let ch := chain t location
  let .some d0 := ch.head? | throw (noMatch location)
  let target := d0.tab
  let mut out := r
  if out.tabs.isEmpty then
    for root in roots t do
      let .some route := t[root]? | pure ()
      let d : Destination := { name := route.name, url := canonical route.pattern,
                               tab := route.name, params := emptyParams t }
      let (e, o) ← mint out d
      out := { o with tabs := o.tabs ++ [{ name := route.name, stack := [e] }] }
  let .some index := out.tabs.findIdx? (·.name == target) | throw s!"unknown tab {target}"
  let old := ((out.tabs[index]?).map (·.stack)).getD []
  let mut entries : List Entry := []
  for (d, position) in ch.zipIdx do
    match old[position]? with
    | .some o =>
      if o.url == d.url then
        entries := entries ++ [{ id := o.id, name := d.name, url := d.url, tab := d.tab, params := d.params }]
        continue
    | .none => pure ()
    let (e, o) ← mint out d
    out := o
    entries := entries ++ [e]
  out := setStack out index fun _ => entries
  return { out with tab := target }

def destination (t : Table) (r : Router) (location : String) : Except String Destination :=
  let url := canonical location
  match matchIndex t url with
  | .none => .error (noMatch location)
  | .some (i, ps) => .ok { name := ((t[i]?).map (·.name)).getD "", params := ps, url, tab := r.tab }

/-- Append a fresh visit to the selected stack, unless the location is on
top already. -/
def push (t : Table) (r : Router) (location : String) : Verb :=
  if r.tabs.isEmpty then «open» t r location else do
  let d ← destination t r location
  let index ← selected r
  let tab := r.tabs[index]?
  if ((tab.bind (·.stack.getLast?)).map (·.url)) == .some d.url then return r
  let (e, out) ← mint r d
  return setStack out index (· ++ [e])

/-- Rewrite the top visit, keeping its id; at depth one only to the tab's
own root route. -/
def replace (t : Table) (r : Router) (location : String) : Verb :=
  if r.tabs.isEmpty then «open» t r location else do
  let d ← destination t r location
  let index ← selected r
  let .some tab := r.tabs[index]? | throw "router has no top"
  if tab.stack.length == 1 && d.name != tab.name then
    throw "replace cannot change the tab's root route"
  let .some old := tab.stack.getLast? | throw "router has no top"
  let e : Entry := { id := old.id, name := d.name, url := d.url, tab := d.tab, params := d.params }
  return setStack r index fun s => s.dropLast ++ [e]

/-- Pop the selected stack; a root is unchanged. Never refuses. -/
def back (r : Router) : Router :=
  match selected r with
  | .ok i =>
    if (((r.tabs[i]?).map (·.stack.length)).getD 0) > 1 then setStack r i (·.dropLast) else r
  | .error _ => r

/-- Show a retained tab; reselecting the current one pops it to its root. -/
def select (r : Router) (name : String) : Verb :=
  match r.tabs.findIdx? (fun t => t.name == name && !t.stack.isEmpty) with
  | .none => .error s!"unknown tab {name}"
  | .some i =>
    let r := if r.tab == name then setStack r i (·.take 1) else r
    .ok { r with tab := name }

/-- Stay, pop to the nearest selected occurrence, select another tab's top,
or push — in that order, comparing canonical locations. -/
def go (t : Table) (r : Router) (location : String) : Verb :=
  let url := canonical location
  if (matchIndex t url).isNone then .error (noMatch location) else
  let s := stack r
  -- `rposition`: the last occurrence.
  let pos := (s.zipIdx.filter (·.1.url == url)).getLast?.map (·.2)
  match pos with
  | .some p =>
    match selected r with
    | .ok i => .ok (setStack r i (·.take (p + 1)))
    | .error _ => .ok r
  | .none =>
    match r.tabs.find? (fun tb => tb.name != r.tab && (tb.stack.getLast?.map (·.url)) == .some url) with
    | .some tb => select r tb.name
    | .none => push t r location

/-- Seed every tab at its root, then open the launch location. -/
def launch (t : Table) (location : String) : Verb := «open» t {} location

/-! ## The plan boundary (`runner/src/runner/router.rs`) -/

/-- A number the runner reads as an id: an integer in `[0, 2^53 - 1]`. -/
def integer : Value → Option Nat
  | .num f =>
    if 0 ≤ f && f ≤ 9007199254740991.0 && f.floor == f then .some f.toUInt64.toNat else .none
  | _ => .none

def str? : Value → Option String
  | .str s => .some s
  | _ => .none

def entryOf (t : Table) (v : Value) : Option Entry := do
  let .record _ [id, name, url, tab, .record _ ps] := v | .none
  let names := paramNames t
  if ps.length != names.length then .none
  let values ← ps.mapM str?
  return { id := ← integer id, name := ← str? name, url := ← str? url, tab := ← str? tab,
           params := names.zip values }

/-- A router read out of a value, when it is a valid one: the shapes, a
total top, unique tab names and ids, ids below `next`, and every entry's
URL canonical and matching its name and parameters. -/
def routerOf (t : Table) (v : Value) : Option Router := do
  let .record _ [tab, .list tabs, next] := v | .none
  let tabs ← tabs.mapM fun tv => do
    let .record _ [name, .list stack] := tv | .none
    return { name := ← str? name, stack := ← stack.mapM (entryOf t) : Tab }
  let r : Router := { tab := ← str? tab, tabs, next := ← integer next }
  if (top r).isNone then .none
  let names := r.tabs.map (·.name)
  if names.eraseDups.length != names.length then .none
  let ids := (r.tabs.flatMap (·.stack)).map (·.id)
  if ids.eraseDups.length != ids.length then .none
  let ok := r.tabs.all fun tb =>
    (tb.stack.head?.map (·.name)) == .some tb.name &&
    tb.stack.all fun e =>
      e.tab == tb.name && e.id < r.next && canonical e.url == e.url &&
      match matchName t e.url with
      | .some (n, ps) => n == e.name && Params.eq ps e.params
      | .none => false
  if ok then return r else .none

def paramsValue (t : Table) (ps : Params) : Value :=
  .record "Params" ((paramNames t).map fun n => .str ((Params.get ps n).getD ""))

def entryValue (t : Table) (e : Entry) : Value :=
  .record "Entry" [.num (Float.ofNat e.id), .str e.name, .str e.url, .str e.tab, paramsValue t e.params]

def routerValue (t : Table) (r : Router) : Value :=
  .record "Router" [.str r.tab,
    .list (r.tabs.map fun tb => .record "Tab" [.str tb.name, .list (tb.stack.map (entryValue t))]),
    .num (Float.ofNat r.next)]

/-- A verb on a router value: a refusal answers the input value itself.
A value of the shape `Router` that is not a valid router (a source can
answer one) traps: a refusal, not a type error, since it has its type. -/
def verb (t : Table) (v : Value) (f : Router → Verb) : Result Value := do
  let .some r := routerOf t v | .error (.refused "a router verb on a value that is not a valid router")
  match f r with
  | .ok r => .ok (routerValue t r)
  | .error _ => .ok v

/-- A read of a router value. -/
def read (t : Table) (v : Value) (f : Router → Option Value) : Result Value :=
  match (routerOf t v).bind f with
  | .some w => .ok w
  | .none => .error (.refused "a router read of a value that is not a valid router")

/-- `path(name, args…)` as the compiler expands it: each argument (a number
through `toString`) through `encodeRouteSegment` into the route's pattern. -/
def pathValue (t : Table) (name : String) (args : List Value) : Result Value := do
  let .some route := t.find? (fun r => r.name == name && !r.notfound)
    | .error (.type s!"`path` of an unknown route `{name}`")
  let texts ← args.mapM fun
    | .str s => .ok s
    | .num f => .ok (Number.jsToString f)
    | _ => .error (.type "`path` of a value that is not a string or number")
  let rec go : Bool → List String → List String → Result String
    | _, [], _ => .ok ""
    | first, seg :: segs, vs => do
      let sep := if first then "" else "/"
      let (piece, vs) ← if startsWith seg ':' then
          match vs with
          | v :: vs => match encodeRouteSegment v with
            | .some e => pure (e, vs)
            | .none => .error (.refused "a path parameter cannot be empty, `.` or `..`")
          | [] => .error (.type "`path` with too few parameters")
        else pure (seg, vs)
      pure (sep ++ piece ++ (← go false segs vs))
  .str <$> go true (split route.pattern '/') texts

end Route

end Contract
