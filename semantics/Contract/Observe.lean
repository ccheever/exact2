/-
What a differential run compares: a canonical text of a configuration.

The Rust side (`semantics/difftest/src/observe.rs`) prints the runner's
state in exactly this form; the two texts are compared line by line.

  `== <step>`              the step that produced what follows
  `outcome ok|refused|poisoned`
  `ran <handler|task|then|queue> <action> <ok|refused|poisoned>`
                           each commit attempted, in order; a queue names
                           its mutation. Boot has none.
  `# <reason>`             after a refused or poisoned outcome or `ran`;
                           commentary, never compared
  `slot <name> <value>`    each root slot, declaration order, mutations last
  `store <name> <value>`   each persisted state (LLP 1116 D5), declaration
                           order: what the device's store keeps for it,
                           which after every commit that stands is the
                           slot's value
  `derive <name> <value>`  each derive, declaration order
  `resource <name> <value>`
  `queued <name> <count>`   each `queue` mutation's waiting sends
  `command <name> <value>…` each command the step's commit issued
  `view <testId> <text>`   each element with a `testId`, in preorder, but
                           none inside a virtualized `list`: the runner
                           shows the rows its window lays out (a layout
                           fact), the semantics every row. An unopened
                           tab panel has no descendants to observe

Values: a number is `n` and its 16 hex digits of IEEE bits (every NaN is
`7ff8000000000000`); a string is quoted with `\"`, `\\`, `\n`, `\r`, `\t`
and `\u00xx` for the other controls; `true`, `false`, `()`, `none`,
`some(v)`, `[v,…]` for a list and `{v,…}` for a record.
-/
import Contract.Runtime

namespace Contract.Observe

def hex16 (n : UInt64) : String :=
  let digits := "0123456789abcdef".toList
  String.ofList ((List.range 16).reverse.map fun i =>
    digits.getD ((n >>> (UInt64.ofNat (4 * i))) &&& 0xf).toNat '0')

def quote (s : String) : String :=
  let body := s.foldl (fun acc c =>
    match c with
    | '"' => acc ++ "\\\""
    | '\\' => acc ++ "\\\\"
    | '\n' => acc ++ "\\n"
    | '\r' => acc ++ "\\r"
    | '\t' => acc ++ "\\t"
    | c => if c.toNat < 0x20 then
        acc ++ "\\u00" ++ String.ofList [ "0123456789abcdef".toList.getD (c.toNat / 16) '0',
                                         "0123456789abcdef".toList.getD (c.toNat % 16) '0']
      else acc.push c) ""
  "\"" ++ body ++ "\""

partial def value : Value → String
  | .num f => "n" ++ hex16 (Number.canonicalBits f)
  | .bool b => if b then "true" else "false"
  | .str s => quote s
  | .unit => "()"
  | .none => "none"
  | .some v => "some(" ++ value v ++ ")"
  | .list xs => "[" ++ ",".intercalate (xs.map value) ++ "]"
  | .record _ xs => "{" ++ ",".intercalate (xs.map value) ++ "}"

partial def viewLines : List VNode → List String
  | [] => []
  | n :: rest =>
    let here := match n.testId with
      | .some id => ["view " ++ quote id ++ " " ++ (n.text.map quote).getD "-"]
      | .none => []
    here ++ (if n.windowed then [] else viewLines n.children) ++ viewLines rest

def why : Err → String
  | .type w => "type: " ++ w
  | .pending => "pending"
  | .unbound x => "unbound: " ++ x
  | .unsupported w => "unsupported: " ++ w
  | .refused w => "refused: " ++ w

/-- The outcome line; a refusal's reason follows on a `#` line, which a
comparison skips (the runner names its reasons differently). -/
def outcome : Outcome → String
  | .ok => "outcome ok"
  | .refused e => "outcome refused\n# " ++ why e
  | .poisoned e => "outcome poisoned\n# " ++ why e

/-- One attempted commit within an observed step (LLP 1148 D3). -/
structure Ran where
  kind : String
  action : String
  out : Outcome

/-- `dispatch` with the action attempted, including a refusal of its arguments. -/
def dispatchTrace (p : Program) (o : Oracle) (c : Config) (target : String) (event : String)
    (payload : Option Value) : (Config × Outcome) × List Ran :=
  if c.poisoned then ((c, .refused (.refused "poisoned")), []) else
  match findTestId target c.view with
  | .none => ((c, .refused (.refused s!"no element `{target}`")), [])
  | .some n =>
    match n.handlers.find? (·.1 == event) with
    | .none => ((c, .refused (.refused s!"`{target}` has no {event} handler")), [])
    | .some (_, a, args) =>
      if payload.isSome && n.control.isSome then
        ((c, .refused (.unsupported s!"a payload for a `{n.control.getD ""}` control")), []) else
      let env : Env := { prog := p, slots := c.slots, derives := c.settled.derives,
                         resources := c.settled.resources, rows := rowSlots c.store n.rows,
                         now := c.now }
      match evalList fuel env false n.locals args with
      | .error e => ((c, .refused e), [⟨"handler", a, .refused (.refused ("arguments refused: " ++ why e))⟩])
      | .ok vs =>
        let result := runAction p o c a (vs ++ payload.toList) n.rows
        (result, [⟨"handler", a, result.2⟩])

/-- `advance` with each attempted commit, in scheduling order. -/
def advanceTrace (p : Program) (o : Oracle) (c : Config) (t : F64) : (Config × Outcome) × List Ran :=
  let due (c : Config) : Option (Timer × Nat) :=
    (c.timers.zipIdx).foldl (fun best (tm, i) =>
      if tm.next ≤ t then
        match best with
        | .none => Option.some (tm, i)
        | .some (b, j) => if tm.precedes b then Option.some (tm, i) else Option.some (b, j)
      else best) Option.none
  let pick (c : Config) : Option Clock := clockPick p c t ((due c).map (·.1.next))
  let finish (c : Config) : (Config × Outcome) × List Ran :=
    (({ c with now := if t > c.now then t else c.now }, .ok), [])
  -- At most `timerFireLimit` fires per advance (the runner's
  -- `TIMER_FIRE_LIMIT`), `then`s and queued sends counted; one more due is
  -- a refusal.
  let rec go : Nat → Config → (Config × Outcome) × List Ran
    | 0, c =>
      match pick c, due c with
      | .none, .none => finish c
      | _, _ => ((c, .refused (.refused "too many timer fires")), [])
    | n + 1, c =>
      if c.poisoned then ((c, .refused (.refused "poisoned")), []) else
      match pick c with
      | .some (.next m w) =>
        let c := { c with nexts := c.nexts.filter (·.1 != m), now := if c.now < w then w else c.now }
        match nextCommit p o c m with
        | (c, .ok) =>
          let (result, ran) := go n c
          (result, ⟨"queue", m, .ok⟩ :: ran)
        | (c, out) => ((c, out), [⟨"queue", m, out⟩])
      | .some (.andThen m w a) =>
        let c := { c with armed := c.armed.filter (·.1 != m), now := if c.now < w then w else c.now }
        match runAction p o c a [] [] with
        | (c, .ok) =>
          let (result, ran) := go n c
          (result, ⟨"then", a, .ok⟩ :: ran)
        | (c, out) => ((c.armNexts p, out), [⟨"then", a, out⟩])
      | .none =>
      match due c with
      | .none => finish c
      | .some (tm, i) =>
        let timers := c.timers.set i tm.fired
        let c := { c with timers, now := tm.next }
        match runAction p o c tm.action [] [] with
        | (c, .ok) =>
          let (result, ran) := go n c
          (result, ⟨"task", tm.action, .ok⟩ :: ran)
        | (c, out) => ((c, out), [⟨"task", tm.action, out⟩])
  if t < c.now then ((c, .ok), []) else go timerFireLimit c

/-- Land the armed `then`s and queued sends at the current clock, without timers. -/
def land (p : Program) (o : Oracle) (c : Config) : (Config × Outcome) × List Ran :=
  let rec go : Nat → Config → (Config × Outcome) × List Ran
    | 0, c =>
      match clockPick p c c.now .none with
      | .none => ((c, .ok), [])
      | .some _ => ((c, .refused (.refused "too many timer fires")), [])
    | n + 1, c =>
      if c.poisoned then ((c, .refused (.refused "poisoned")), []) else
      match clockPick p c c.now .none with
      | .some (.next m _) =>
        let c := { c with nexts := c.nexts.filter (·.1 != m) }
        match nextCommit p o c m with
        | (c, .ok) =>
          let (result, ran) := go n c
          (result, ⟨"queue", m, .ok⟩ :: ran)
        | (c, out) => ((c, out), [⟨"queue", m, out⟩])
      | .some (.andThen m _ a) =>
        let c := { c with armed := c.armed.filter (·.1 != m) }
        match runAction p o c a [] [] with
        | (c, .ok) =>
          let (result, ran) := go n c
          (result, ⟨"then", a, .ok⟩ :: ran)
        | (c, out) => ((c.armNexts p, out), [⟨"then", a, out⟩])
      | .none => ((c, .ok), [])
  go timerFireLimit c

/-- A commit's outcome uses the same reasons as the step's. -/
def Ran.line (r : Ran) : String :=
  let head := "ran " ++ r.kind ++ " " ++ r.action ++ " "
  match r.out with
  | .ok => head ++ "ok"
  | .refused e => head ++ "refused\n# " ++ why e
  | .poisoned e => head ++ "poisoned\n# " ++ why e

/-- The label, outcome and attempted commits, then (unless poisoned)
the state, commands issued since `commandsBefore`, and view. -/
def lines (p : Program) (label : String) (c : Config) (out : Outcome) (commandsBefore : Nat) (ran : List Ran := []) :
    List String :=
  let head := ["== " ++ label, outcome out] ++ ran.map Ran.line
  match out with
  | .poisoned _ => head
  | _ =>
    if c.poisoned then head else
    head ++
    ((c.slots.filter fun (x, _) => p.locale != .some x).map fun (x, v) => "slot " ++ x ++ " " ++ value v) ++
    ((p.states.filter (·.persist)).filterMap fun st => (lookup st.name c.slots).map fun v =>
      "store " ++ st.name ++ " " ++ value v) ++
    (p.derives.filterMap fun d => (lookup d.name c.settled.derives).map fun v =>
      "derive " ++ d.name ++ " " ++ value v) ++
    (p.resources.filterMap fun r => (lookup r.name c.settled.resources).map fun v =>
      "resource " ++ r.name ++ " " ++ value v) ++
    ((p.mutations.filter (·.queue)).map fun m =>
      "queued " ++ m.name ++ " " ++ toString ((c.queued.filter (·.1 == m.name)).length)) ++
    ((c.commands.drop commandsBefore).map fun (n, vs) =>
      "command " ++ n ++ String.join (vs.map (" " ++ value ·))) ++
    viewLines c.view

/-- A host event, as a script names it. -/
inductive Event where
  | tap (target : String)
  | change (target : String) (text : String)
  | clock (ms : F64)
  /-- Any other event with an optional payload (`hover`, `key`, …). -/
  | other (target event : String) (payload : Option Value)
  deriving Inhabited

def Event.label : Event → String
  | .tap t => "tap " ++ quote t
  | .change t s => "type " ++ quote t ++ " " ++ quote s
  | .clock ms => "clock +" ++ Number.jsToString ms
  | .other t e _ => e ++ " " ++ quote t

/-- An event delivered at the current time, as every host delivers one (the
runner's `dispatch_at`, the web build's `on()`): the work already due by
`c.now` (a timer, a mutation's `then`, a queue's next send) fires first, then
the event, which runs even when that work refused. The outcome is the
event's when it failed, else the due work's; a poisoned advance ends the step
there. An event for an element the view lacks, or that it has no handler for,
advances nothing: a host attaches a listener only where one is declared, so
it cannot deliver it. A tap carries no time of its own, so the clock does not
move. -/
def dispatchAt (p : Program) (o : Oracle) (c : Config) (target event : String)
    (payload : Option Value) : Config × Outcome :=
  match findTestId target c.view with
  | .none => dispatch p o c target event payload
  | .some n =>
    if (n.handlers.find? (·.1 == event)).isNone then dispatch p o c target event payload else
    match advance p o c c.now with
    | (c₁, .poisoned e) => (c₁, .poisoned e)
    | (c₁, o₁) =>
      match dispatch p o c₁ target event payload with
      | (c₂, .ok) => (c₂, o₁)
      | (c₂, o₂) => (c₂, o₂)

def step (p : Program) (o : Oracle) (c : Config) : Event → Config × Outcome
  | .tap t => dispatchAt p o c t "press" .none
  | .change t s => dispatchAt p o c t "change" (.some (.str s))
  | .clock ms => advance p o c (c.now + ms)
  | .other t e v => dispatchAt p o c t e v

/-- Host delivery with the due commits before the handler's. -/
def dispatchAtTrace (p : Program) (o : Oracle) (c : Config) (target event : String)
    (payload : Option Value) : (Config × Outcome) × List Ran :=
  match findTestId target c.view with
  | .none => dispatchTrace p o c target event payload
  | .some n =>
    if (n.handlers.find? (·.1 == event)).isNone then dispatchTrace p o c target event payload else
    match advanceTrace p o c c.now with
    | ((c₁, .poisoned e), ran) => ((c₁, .poisoned e), ran)
    | ((c₁, o₁), due) =>
      let ((c₂, o₂), handler) := dispatchTrace p o c₁ target event payload
      ((c₂, match o₂ with | .ok => o₁ | _ => o₂), due ++ handler)

def stepTrace (p : Program) (o : Oracle) (c : Config) : Event → (Config × Outcome) × List Ran
  | .tap t => dispatchAtTrace p o c t "press" .none
  | .change t s => dispatchAtTrace p o c t "change" (.some (.str s))
  | .clock ms => advanceTrace p o c (c.now + ms)
  | .other t e v => dispatchAtTrace p o c t e v

/-- Script delivery includes `land`; the event type used by the existing theorems stays unchanged. -/
inductive ScriptEvent where
  | tap (target : String)
  | change (target : String) (text : String)
  | clock (ms : F64)
  | other (target event : String) (payload : Option Value)
  | land

def ScriptEvent.event : ScriptEvent → Option Event
  | .tap t => .some (.tap t)
  | .change t s => .some (.change t s)
  | .clock ms => .some (.clock ms)
  | .other t e v => .some (.other t e v)
  | .land => .none

def ScriptEvent.label (e : ScriptEvent) : String :=
  match e.event with
  | .some event => event.label
  | .none => "land"

#guard ScriptEvent.land.label == "land"

def ScriptEvent.ofEvent : Event → ScriptEvent
  | .tap t => .tap t
  | .change t s => .change t s
  | .clock ms => .clock ms
  | .other t e v => .other t e v

-- Structural equality includes hidden scheduling state and compares F64 bits,
-- including NaNs and signed zeros; observation text alone omits that state.
deriving instance Repr for TabInfo
deriving instance Repr for Settled
deriving instance Repr for VNode
deriving instance Repr for Timer
deriving instance Repr for Config
deriving instance Repr for Outcome

/-- Check the complete result, including state the observation does not print. -/
def checkedLines (p : Program) (label : String) (result twin : Config × Outcome)
    (before : Nat) (ran : List Ran) : List String :=
  if reprStr result == reprStr twin then lines p label result.1 result.2 before ran
  else ["== " ++ label, outcome result.2] ++ ran.map Ran.line ++ ["trace disagrees"]

#guard let c : Config := default
  (checkedLines default "clock +0" (c, .ok) ({ c with armed := [("m", 0)] }, .ok) 0 []).contains "trace disagrees"
#guard let c : Config := default
  (checkedLines default "tap" (c, .ok) (c, .refused (.refused "x")) 0 []).contains "trace disagrees"
#guard let c : Config := { (default : Config) with now := F64.ofBits 0x7ff8000000000000 }
  !(checkedLines default "land" (c, .ok) (c, .ok) 0 []).contains "trace disagrees"

/-- Boot, then every script event. The originals own the state and outcome;
a traced twin that disagrees replaces the configuration with a failing line. -/
def runScript (p : Program) (o : Oracle) (events : List ScriptEvent) : List String := Id.run do
  let (c, out) := boot p o
  match out with
  | .ok => pure ()
  | _ => return ["== boot", outcome out]
  let mut acc := lines p "boot" c out 0
  let mut c := c
  for e in events do
    let before := c.commands.length
    let (label, result, twin, ran) := match e.event with
      | .some authored =>
        let ev := match authored with
          | .change target text =>
            if ((findTestId target c.view).map fun n => n.handlers.any (·.1 == "input")).getD false then
              .other target "input" (.some (.str text))
            else authored
          | _ => authored
        let (twin, ran) := stepTrace p o c ev
        (authored.label, step p o c ev, twin, ran)
      | .none =>
        let (result, ran) := land p o c
        (e.label, result, result, ran)
    let (c', out) := result
    acc := acc ++ checkedLines p label result twin before ran
    c := c'
    match out with
    | .poisoned _ => return acc
    | _ => pure ()
  return acc

/-- Observation of the event vocabulary used by component comparison and theorems. -/
def run (p : Program) (o : Oracle) (events : List Event) : List String :=
  runScript p o (events.map ScriptEvent.ofEvent)

end Contract.Observe
