/-
The program-level operational semantics: statements, actions as
transactions, settlement of derives and resources, rendering, timers, and
the events a host delivers.

A configuration steps on an event to a new configuration. Each step is one
commit or none: an action reads the state as it was when the event arrived,
its writes land together after its body has run (the last write to a slot
wins), and anything that refuses — a trap, a value of the wrong shape, a
settlement that fails — leaves the configuration exactly as it was. A
failure while rendering poisons the configuration: it keeps its state and
refuses every later event (the runner's `poison`).
-/
import Contract.Eval

namespace Contract

/-- What an action body asks for, in statement order. -/
structure Effects where
  writes : List (String × Value) := []
  rowWrites : List (String × Value) := []
  commands : List (String × List Value) := []
  sends : List (String × String × List Value) := []
  refreshes : List String := []
  deriving Inhabited

def isRootState (p : Program) (x : String) : Bool :=
  p.states.any (fun s => s.name == x && s.owner.isNone) || p.mutations.any (·.name == x)

def isRowState (p : Program) (x : String) : Bool :=
  p.states.any (fun s => s.name == x && s.owner.isSome)

/-- Run a block of statements. A `let` scopes over the statements after it
in its block and the blocks nested there; a branch's block is a scope of
its own. -/
def exec : Nat → Env → Locals → List Stmt → Effects → Result Effects
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, [], fx => .ok fx
  | fuel + 1, env, ls, s :: rest, fx =>
    match s with
    | .letS x e => do
      let v ← eval fuel env false ls e
      exec fuel env ((x, v) :: ls) rest fx
    | .assign t e => do
      let v ← eval fuel env false ls e
      let fx ←
        if isRootState env.prog t then pure { fx with writes := fx.writes ++ [(t, v)] }
        else if isRowState env.prog t then
          if (lookup t env.rows).isSome then pure { fx with rowWrites := fx.rowWrites ++ [(t, v)] }
          else .error (.refused s!"row slot `{t}` written outside its row")
        else .error (.unbound t)
      exec fuel env ls rest fx
    | .command name args => do
      let vs ← evalList fuel env false ls args
      exec fuel env ls rest { fx with commands := fx.commands ++ [(name, vs)] }
    | .send t src args => do
      let vs ← evalList fuel env false ls args
      exec fuel env ls rest { fx with sends := fx.sends ++ [(t, src, vs)] }
    | .refresh t => exec fuel env ls rest { fx with refreshes := fx.refreshes ++ [t] }
    | .ifS c thn els => do
      let fx ← match ← eval fuel env false ls c with
        | .bool true => exec fuel env ls thn fx
        | .bool false => exec fuel env ls els fx
        | _ => .error (.type "`if` on a value that is not a bool")
      exec fuel env ls rest fx
    | .matchS subj x sm nn => do
      let fx ← match ← eval fuel env false ls subj with
        | .some v => exec fuel env ((x, v) :: ls) sm fx
        | .none => exec fuel env ls nn fx
        | _ => .error (.type "`match` on a value that is not an option")
      exec fuel env ls rest fx

/-! ## Types at run time -/

/-- Whether a value has a type: what the runner checks at every boundary
(a slot written, a derive settled, a source's answer, an action's
argument). A `number` that crosses a boundary is finite: infinities and
NaN live only inside an evaluation. -/
partial def conforms (p : Program) : Value → Ty → Bool
  | _, .unknown => true
  | .num f, .number => Number.isFinite f
  | .bool _, .bool => true
  | .str _, .string => true
  | .unit, .unit => true
  | .none, .option _ => true
  | .some v, .option t => conforms p v t
  | .list xs, .list t => xs.all (conforms p · t)
  | .record _ vs, .record s =>
    match p.shapes.find? (·.name == s) with
    | .some sh => vs.length == sh.fields.length &&
        (vs.zip sh.fields).all fun (v, f) => conforms p v f.ty
    | .none => false
  | _, _ => false

/-! ## The data seam -/

/-- Bit-for-bit sameness (NaN is itself; records by fields). What two
calls to a source must share to be one call. -/
partial def Value.same : Value → Value → Bool
  | .num a, .num b => a.toBits == b.toBits
  | .bool a, .bool b => a == b
  | .str a, .str b => a == b
  | .unit, .unit => true
  | .none, .none => true
  | .some a, .some b => Value.same a b
  | .list xs, .list ys => xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.same a b
  | .record _ xs, .record _ ys => xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.same a b
  | _, _ => false

/-- The data sources: a function of (source, arguments). The semantics is
parametric in it; a differential run supplies the answers the runner's
source gave, as a table. -/
structure Oracle where
  answers : List (String × List Value × Value) := []
  /-- Host facts, by resource: what a resource whose source the host
  answers itself (`exactViewport`, `exactPage`, …) holds. -/
  facts : List (String × Value) := []
  deriving Inhabited

def Oracle.ask (o : Oracle) (src : String) (args : List Value) (resource : String := "") :
    Result Value :=
  match o.answers.find? (fun (s, as, _) =>
      s == src && as.length == args.length && (as.zip args).all fun (a, b) => Value.same a b) with
  | .some (_, _, v) => .ok v
  | .none =>
    match lookup resource o.facts with
    | .some v => .ok v
    | .none => .error (.refused s!"the oracle has no answer for {src}")

/-! ## Settlement -/

structure Settled where
  derives : List (String × Value) := []
  resources : List (String × Value) := []
  /-- The arguments each resource's value answers. -/
  args : List (String × List Value) := []
  deriving Inhabited

/-- The runner's `==` on values (`exact_plan::Value`'s `PartialEq`):
numbers by IEEE `==` (`-0 = 0`, `NaN ≠ NaN`), the rest structurally. Two
argument lists equal under it ask one question. -/
partial def Value.rustEq : Value → Value → Bool
  | .num a, .num b => a == b
  | .bool a, .bool b => a == b
  | .str a, .str b => a == b
  | .unit, .unit => true
  | .none, .none => true
  | .some a, .some b => Value.rustEq a b
  | .list xs, .list ys => xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.rustEq a b
  | .record _ xs, .record _ ys =>
    xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.rustEq a b
  | _, _ => false

def argsEq (xs ys : List Value) : Bool :=
  xs.length == ys.length && (xs.zip ys).all fun (a, b) => Value.rustEq a b

/-- Settle every derive and resource against the slots: evaluate each
whose inputs have settled, until all have. A derive or resource that reads
one not yet settled waits (`pending`); a pass with no progress is a cycle.
The result does not depend on the order of evaluation (the dependency
graph is acyclic, and each value is a function of the values it reads).

A resource whose arguments equal (`argsEq`) those its previous value
answered keeps that value and asks nothing, unless the commit refreshes it
(`force`): its source is asked only when the question changes. -/
def settle (p : Program) (o : Oracle) (slots : List (String × Value)) (now : Float)
    (prev : Settled := {}) (force : List String := []) : Result Settled :=
  let rec pass : Nat → Settled → Result Settled
    | 0, _ => .error (.refused "settlement did not converge")
    | n + 1, st => do
      let mut st := st
      let mut progress := false
      let mut waiting := false
      for d in p.derives do
        if (lookup d.name st.derives).isSome then continue
        let env : Env := { prog := p, slots, derives := st.derives, resources := st.resources, now }
        match eval fuel env false [] d.body with
        | .ok v =>
          if !conforms p v d.ty then throw (.refused s!"derive `{d.name}` of the wrong type")
          st := { st with derives := st.derives ++ [(d.name, v)] }
          progress := true
        | .error .pending => waiting := true
        | .error e => throw e
      for r in p.resources do
        if (lookup r.name st.resources).isSome then continue
        let env : Env := { prog := p, slots, derives := st.derives, resources := st.resources, now }
        match evalList fuel env false [] r.args with
        | .ok args =>
          let kept : Option (Value × List Value) :=
            match lookup r.name prev.resources, prev.args.find? (·.1 == r.name) with
            | .some v, .some (_, old) =>
              if !force.contains r.name && argsEq old args then Option.some (v, old) else Option.none
            | _, _ => Option.none
          let (v, held) ← match kept with
            | .some k => pure k
            | .none => do
              let v ← o.ask r.source args r.name
              if !conforms p v r.ty then throw (.refused s!"resource `{r.name}` of the wrong shape")
              pure (v, args)
          st := { st with resources := st.resources ++ [(r.name, v)], args := st.args ++ [(r.name, held)] }
          progress := true
        | .error .pending => waiting := true
        | .error e => throw e
      if !waiting then return st
      if !progress then throw (.refused "a cycle among derives and resources")
      pass n st
  pass (p.derives.length + p.resources.length + 2) {}

/-! ## Rendering -/

/-- A row's identity: the `each` it belongs to, its key (a string, a
finite number with `-0` read as `0`, or a bool; anything else fails the
render, as the runner's `key_text` does), and how many
earlier rows of that list had the same key (the runner's `dup`). A row in
a nested `each` is named by its path from the outermost. -/
abbrev RowId := List (Nat × Value × Nat)

/-- Each live row's slots. -/
abbrev RowStore := List (RowId × List (String × Value))

def RowStore.find (rs : RowStore) (id : RowId) : Option (List (String × Value)) :=
  (rs.find? fun (i, _) => i.length == id.length &&
    (i.zip id).all fun ((t, k, d), (t', k', d')) => t == t' && d == d' && Value.same k k').map (·.2)

/-- A rendered element: what the view shows and what a host can address. -/
structure VNode where
  tag : String
  testId : Option String
  /-- The first positional argument, displayed: a `text`'s text. -/
  text : Option String
  /-- (event, action, curried arguments): the arguments are evaluated when
  the event arrives, in the element's scope (`locals`, `rows`) against the
  configuration then — the same state the element was rendered from, but
  the clock may have moved since (`now()`). -/
  handlers : List (String × String × List Expr)
  /-- The names bound where the element stands. -/
  locals : Locals
  /-- The form control it is, when not a text field (`select`, or an
  `input` whose literal `type` is `checkbox`, `range`, `date`, …): its
  payloads follow HTML's rules, which the semantics leaves out. -/
  control : Option String := .none
  /-- The rows this element stands in, innermost first. -/
  rows : List RowId
  children : List VNode
  deriving Inhabited

structure RenderCx where
  env : Env
  store : RowStore
  rows : List RowId := []

/-- A row key as the runner identifies it (`key_text`): a string, a bool,
or a finite number with `-0` read as `0`. -/
def rowKey : Value → Result Value
  | .str t => .ok (.str t)
  | .bool b => .ok (.bool b)
  | .num f =>
    if Number.isFinite f then .ok (.num (if f == 0 then 0 else f))
    else .error (.refused "a row key that is not finite")
  | _ => .error (.refused "a row key that is not a string, number or bool")

/-- Render a view. Returns the elements and the rows that are live after
it (their slots, kept from `store` or initialized now). -/
def render : Nat → RenderCx → Locals → List Node → RowStore → Result (List VNode × RowStore)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, [], live => .ok ([], live)
  | fuel + 1, cx, ls, n :: rest, live => do
    let (here, live) ← match n with
      | .element tag pos props hs children => do
        -- Only what a host observes here is forced: the first positional
        -- argument of a tag whose positional is its text (`text`, `tspan`,
        -- `option`) and the `testId`. Every other attribute is
        -- presentation, kept unevaluated (the runner evaluates it as it
        -- binds it; on a well-typed program neither can fail).
        let text ← match pos with
          | e :: _ =>
            if tag == "text" || tag == "tspan" || tag == "option" then do
              let v ← eval fuel cx.env false ls e
              pure (Option.some (← v.display))
            else pure Option.none
          | [] => pure Option.none
        let testId ← match lookupField "testId" props with
          | .some e => do pure (Option.some (← (← eval fuel cx.env false ls e).display))
          | .none => pure Option.none
        let (kids, live) ← render fuel cx ls children live
        let control :=
          if tag == "select" then Option.some "select"
          else if tag == "input" then
            match lookupField "type" props with
            | .some (.str t) =>
              if ["checkbox", "file", "range", "date", "time", "datetime-local"].contains t.toLower
              then Option.some t else Option.none
            | _ => Option.none
          else Option.none
        pure ([{ tag, testId, text, handlers := hs, locals := ls, rows := cx.rows,
                 control, children := kids : VNode }], live)
      | .when c thn els => do
        match ← eval fuel cx.env false ls c with
        | .bool true => render fuel cx ls thn live
        | .bool false => render fuel cx ls els live
        | _ => .error (.type "`when` on a value that is not a bool")
      | .matchN s x sm nn => do
        match ← eval fuel cx.env false ls s with
        | .some v => render fuel cx ((x, v) :: ls) sm live
        | .none => render fuel cx ls nn live
        | _ => .error (.type "`match` on a value that is not an option")
      | .each tag x ix list key body => do
        let items ← (← eval fuel cx.env false ls list).asList
        renderRows fuel cx ls tag x ix key body items 0 [] live
    let (more, live) ← render fuel cx ls rest live
    pure (here ++ more, live)
where
  /-- The rows of an `each`, in list order. `seen` holds the keys of the
  rows before this one, for `dup`. -/
  renderRows : Nat → RenderCx → Locals → Nat → String → Option String → Expr → List Node →
      List Value → Nat → List Value → RowStore → Result (List VNode × RowStore)
    | 0, _, _, _, _, _, _, _, _, _, _, _ => .error outOfFuel
    | _ + 1, _, _, _, _, _, _, _, [], _, _, live => .ok ([], live)
    | fuel + 1, cx, ls, tag, x, ix, key, body, item :: items, i, seen, live => do
      let ls' := (x, item) :: ls
      let ls' := match ix with
        | .some n => (n, Value.num (Float.ofNat i)) :: ls'
        | .none => ls'
      let k ← rowKey (← eval fuel cx.env false ls' key)
      let dup := (seen.filter (Value.same k ·)).length
      let id : RowId := (cx.rows.head?.getD []) ++ [(tag, k, dup)]
      -- The row's slots: kept, or initialized from their initializers in
      -- the row's own scope (an initializer may read the item).
      let slots ← match cx.store.find id with
        | .some s => pure s
        | .none => do
          let mut s : List (String × Value) := []
          for st in cx.env.prog.states do
            if st.owner == Option.some tag then
              let env := { cx.env with rows := s ++ cx.env.rows }
              let v ← eval fuel env false ls' st.init
              if !conforms cx.env.prog v st.ty then
                throw (.refused s!"row slot `{st.name}` initialized with a value of the wrong type")
              s := s ++ [(st.name, v)]
          pure s
      let cx' : RenderCx := { cx with env := { cx.env with rows := slots ++ cx.env.rows }, rows := id :: cx.rows }
      let (vs, live) ← render fuel cx' ls' body (live ++ [(id, slots)])
      let (more, live) ← renderRows fuel cx ls tag x ix key body items (i + 1) (seen ++ [k]) live
      pure (vs ++ more, live)

/-! ## Configurations and steps -/

structure Timer where
  action : String
  interval : Float
  once : Bool
  next : Float
  deriving Inhabited

structure Config where
  slots : List (String × Value)
  settled : Settled
  store : RowStore
  view : List VNode
  now : Float
  timers : List Timer
  poisoned : Bool := false
  /-- The commands committed actions issued, oldest first. -/
  commands : List (String × List Value) := []
  deriving Inhabited

/-- A step's outcome, as a host sees it. -/
inductive Outcome where
  | ok
  | refused (why : Err)
  | poisoned (why : Err)
  deriving Inhabited

def setSlot (slots : List (String × Value)) (x : String) (v : Value) : List (String × Value) :=
  slots.map fun (y, w) => if x == y then (y, v) else (y, w)

/-- The slots of the rows in force, innermost first. -/
def rowSlots (store : RowStore) (rows : List RowId) : List (String × Value) :=
  rows.foldr (fun id acc => (store.find id).getD [] ++ acc) []

/-- Apply row writes: each goes to the innermost row in force whose `each`
owns the slot. -/
def applyRowWrites (p : Program) (store : RowStore) (rows : List RowId)
    (ws : List (String × Value)) : RowStore :=
  ws.foldl (fun store (x, v) =>
    let owner := (p.states.find? (·.name == x)).bind (·.owner)
    match rows.find? (fun id => (id.getLast?.map (·.1)) == owner) with
    | .some id => store.map fun (i, s) =>
        if (RowStore.find [(i, s)] id).isSome then (i, setSlot s x v) else (i, s)
    | .none => store) store

/-- Settle and render against new slots. -/
def update (p : Program) (o : Oracle) (slots : List (String × Value)) (store : RowStore)
    (now : Float) (prev : Settled := {}) (force : List String := []) :
    Result (Settled × Result (List VNode × RowStore)) := do
  let st ← settle p o slots now prev force
  let env : Env := { prog := p, slots, derives := st.derives, resources := st.resources, now }
  .ok (st, render fuel { env, store } [] p.view [])

/-- Run an action as one commit. `rows` are the rows in force where the
event arrived (none for a timer). -/
def runAction (p : Program) (o : Oracle) (c : Config) (name : String) (args : List Value)
    (rows : List RowId) : Config × Outcome :=
  let refuse (e : Err) := (c, Outcome.refused e)
  if c.poisoned then refuse (.refused "poisoned") else
  match p.actions.find? (·.name == name) with
  | .none => refuse (.unbound name)
  | .some a =>
    if a.params.length != args.length then refuse (.refused "arity") else
    if !((a.params.zip args).all fun ((_, t), v) => conforms p v t) then
      refuse (.refused "an argument of the wrong type") else
    let env : Env := { prog := p, slots := c.slots, derives := c.settled.derives,
                       resources := c.settled.resources, rows := rowSlots c.store rows, now := c.now }
    let ls : Locals := ((a.params.map (·.1)).zip args).reverse
    match exec fuel env ls a.body {} with
    | .error e => refuse e
    | .ok fx =>
      -- Sends ask their source now; an answer lands in the mutation's slot.
      let answered : Result (List (String × Value)) := fx.sends.mapM fun (m, src, vs) => do
        let v ← o.ask src vs
        let ty := ((p.mutations.find? (·.name == m)).map (·.ty)).getD .unknown
        if !conforms p v ty then throw (.refused s!"mutation `{m}` answered with the wrong shape")
        pure (m, Value.some v)
      match answered with
      | .error e => refuse e
      | .ok answered =>
        let slotTy (x : String) : Ty :=
          match p.states.find? (·.name == x) with
          | .some s => s.ty
          | .none => ((p.mutations.find? (·.name == x)).map (fun m => Ty.option m.ty)).getD .unknown
        if !((fx.writes ++ fx.rowWrites).all fun (x, v) => conforms p v (slotTy x)) then
          refuse (.refused "a write of the wrong type") else
        let slots := (answered ++ fx.writes).foldl (fun s (x, v) => setSlot s x v) c.slots
        let store := applyRowWrites p c.store rows fx.rowWrites
        match update p o slots store c.now c.settled fx.refreshes with
        | .error e => refuse e
        | .ok (st, .ok (view, live)) =>
          ({ c with slots, settled := st, store := live, view,
                    commands := c.commands ++ fx.commands }, .ok)
        | .ok (st, .error e) =>
          ({ c with slots, settled := st, store, poisoned := true,
                    commands := c.commands ++ fx.commands }, .poisoned e)

/-- Boot: initialize the root slots in declaration order (an initializer
reads the slots before it), settle, render, and start the timers. -/
def boot (p : Program) (o : Oracle) : Config × Outcome :=
  let empty : Config := { slots := [], settled := {}, store := [], view := [], now := 0, timers := [] }
  let init : Result (List (String × Value)) := p.states.foldlM (fun slots st => do
      if st.owner.isSome then return slots
      let env : Env := { prog := p, slots, now := 0 }
      let v ← eval fuel env false [] st.init
      if !conforms p v st.ty then throw (.refused s!"slot `{st.name}` initialized with the wrong type")
      pure (slots ++ [(st.name, v)])) []
  if p.states.any (·.ty == .record "Router") then (empty, .refused (.unsupported "routes")) else
  match init with
  | .error e => (empty, .refused e)
  | .ok slots =>
    let slots := slots ++ p.mutations.map (·.name, Value.none)
    let timers : Result (List Timer) := p.tasks.mapM fun t => do
      if t.kind == .frame then throw (.unsupported "frame tasks")
      let ms ← (← eval fuel { prog := p, slots } false [] t.ms).asNum
      pure { action := t.action, interval := ms, once := t.kind == .after, next := ms }
    match timers with
    | .error e => (empty, .refused e)
    | .ok timers =>
      if p.mutations.any (·.andThen.isSome) then (empty, .refused (.unsupported "`then`")) else
      match update p o slots [] 0 with
      | .error e => (empty, .refused e)
      | .ok (st, .ok (view, live)) =>
        ({ slots, settled := st, store := live, view, now := 0, timers }, .ok)
      -- A view that fails to render at boot fails the boot: there is no
      -- runner to poison.
      | .ok (_, .error e) => (empty, .refused e)

/-! ## Events -/

/-- The first element, in preorder, whose `testId` is `id`. -/
partial def findTestId (id : String) : List VNode → Option VNode
  | [] => .none
  | n :: rest =>
    if n.testId == Option.some id then .some n
    else match findTestId id n.children with
      | .some m => .some m
      | .none => findTestId id rest

/-- Deliver `event` (with its payload, if it has one) to the element
`id`: its handler's curried arguments, then the payload, are the action's
arguments. -/
def dispatch (p : Program) (o : Oracle) (c : Config) (target : String) (event : String)
    (payload : Option Value) : Config × Outcome :=
  if c.poisoned then (c, .refused (.refused "poisoned")) else
  match findTestId target c.view with
  | .none => (c, .refused (.refused s!"no element `{target}`"))
  | .some n =>
    match n.handlers.find? (·.1 == event) with
    | .none => (c, .refused (.refused s!"`{target}` has no {event} handler"))
    | .some (_, a, args) =>
      if payload.isSome && n.control.isSome then
        (c, .refused (.unsupported s!"a payload for a `{n.control.getD ""}` control")) else
      let env : Env := { prog := p, slots := c.slots, derives := c.settled.derives,
                         resources := c.settled.resources, rows := rowSlots c.store n.rows,
                         now := c.now }
      match evalList fuel env false n.locals args with
      | .error e => (c, .refused e)
      | .ok vs => runAction p o c a (vs ++ payload.toList) n.rows

/-- The most timer fires one advance makes. -/
def timerFireLimit : Nat := 4096

/-- Move the clock to `t`, firing every timer due by then in order of due
time (ties by declaration order), each at its own due time. A refusal stops
the advance there, with the clock at the refusing timer's due time. -/
def advance (p : Program) (o : Oracle) (c : Config) (t : Float) : Config × Outcome :=
  let due (c : Config) : Option (Timer × Nat) :=
    (c.timers.zipIdx).foldl (fun best (tm, i) =>
      if tm.next ≤ t then
        match best with
        | .none => Option.some (tm, i)
        | .some (b, j) => if tm.next < b.next then Option.some (tm, i) else Option.some (b, j)
      else best) Option.none
  let finish (c : Config) : Config × Outcome := ({ c with now := if t > c.now then t else c.now }, .ok)
  -- At most `timerFireLimit` fires per advance (the runner's
  -- `TIMER_FIRE_LIMIT`); one more due is a refusal.
  let rec go : Nat → Config → Config × Outcome
    | 0, c =>
      match due c with
      | .none => finish c
      | .some _ => (c, .refused (.refused "too many timer fires"))
    | n + 1, c =>
      if c.poisoned then (c, .refused (.refused "poisoned")) else
      match due c with
      | .none => finish c
      | .some (tm, i) =>
        let next := if tm.once then (1.0 / 0.0) else tm.next + tm.interval
        let timers := c.timers.set i { tm with next }
        let c := { c with timers, now := tm.next }
        match runAction p o c tm.action [] [] with
        | (c, .ok) => go n c
        | (c, out) => (c, out)
  if t < c.now then (c, .ok) else go timerFireLimit c

end Contract
