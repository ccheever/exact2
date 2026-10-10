/-
The component-level semantics: the meaning of an unexpanded file
(`Contract.Components`), given instance by instance, without expanding it.

A use of a component is an *instance*. Its identity is the path that leads
to it from the root — every use, region arm (a `when`/`match` arm, an
`each` row by key and duplicate count) and `children` node on the way,
each named by its component and node — so it is exactly "the declaration,
which use, and the enclosing `each` keys" (LLP 1017 P4c). An instance's
state lives in a store under that path for as long as the instance is
rendered: it is initialized, in the instance's scope, the first time the
instance renders, kept while it keeps rendering, and dropped when a render
leaves it out.

Names are read in a *frame*. The root's frame reads the root's own
declarations as `Contract.Eval` does. An instance's frame reads, in this
order, its component's derives (the body, evaluated in the frame at every
read: a child's derive is not settled), its states (the store at the
instance's path), its injects and props. A prop or inject is a thunk: the
expression the use (or the providing component) wrote, with the frame and
the locals where it was written, evaluated at each read. A `slot`
component's fill is rendered where its `children` node stands, in the use
site's frame, locals, providers and fill. A handler names an action of its
component or an `action` prop, resolved through the props to the action
it names with the arguments curried on the way; its arguments are
evaluated at dispatch, each in its own frame. A child's action writes its
instance's states; a call of an `action` prop last in it (LLP 1017 §11)
runs the named action's statements after it, in that action's own frame,
in the same commit, reading the same state.

The root's own declarations — slots, settlement of derives and resources,
root actions, timers — mean what `Contract.Runtime` says of a program made
of the root alone (`rootProgram`): no component is involved there.
-/
import Contract.Runtime
import Contract.Observe
import Contract.Components

namespace Contract.CompSem

open Contract Components

/-- One step of the path to an instance: a node of a component's view and,
for a region, which arm or row (`key`, `dup`). -/
structure Step where
  comp : String
  node : Nat
  key : Value := .unit
  dup : Nat := 0
  deriving Repr, Inhabited

/-- An instance's identity: its path from the root, outermost first. -/
abbrev InstId := List Step

def Step.same (a b : Step) : Bool :=
  a.comp == b.comp && a.node == b.node && Value.same a.key b.key && a.dup == b.dup

def InstId.same (a b : InstId) : Bool :=
  a.length == b.length && (a.zip b).all fun (x, y) => x.same y

/-- Each live instance's states. -/
abbrev Store := List (InstId × List (String × Value))

def Store.find (s : Store) (id : InstId) : Option (List (String × Value)) :=
  (s.find? fun (i, _) => InstId.same i id).map (·.2)

/-- Where names are read: the root, or an instance of `comp` at `id` whose
props and injects are thunks — an expression with the frame and locals
where it was written. Injects come first: a same-named inject hides a
prop. -/
inductive Frame where
  | root
  | inst (comp : String) (id : InstId) (binds : List (String × Expr × Frame × Locals))
  deriving Inhabited

def lookupBind (x : String) : List (String × Expr × Frame × Locals) → Option (Expr × Frame × Locals)
  | [] => .none
  | (y, e, f, ls) :: rest => if x == y then .some (e, f, ls) else lookupBind x rest

/-- The root alone, as a program of the flat semantics: its states (the
router's slot first, as the compiler puts it), derives, resources,
mutations, actions and tasks. -/
def rootProgram (p : CProgram) : Program :=
  { shapes := p.shapes, fns := p.fns,
    states := (match p.router with
      | .some x => [{ name := x, ty := .record "Router", init := .none }]
      | .none => []) ++ p.localeStates ++ p.root.states,
    derives := p.root.derives, resources := p.root.resources,
    mutations := p.root.mutations, actions := (ownCallsComponent p.root).actions, tasks := p.root.tasks,
    routes := p.routes, router := p.router, strings := p.strings, locale := p.locale }

/-- What an evaluation reads: the root's names (`Env` over `rootProgram`)
and the instances' states. -/
structure CEnv where
  prog : CProgram
  root : Env
  store : Store
  deriving Inhabited

def CEnv.comp (ce : CEnv) (c : String) : Result CComponent :=
  (ce.prog.component? c).elim (.error (.unbound s!"component `{c}`")) .ok

mutual

/-- Evaluate an expression in a frame. In the root's frame this is `eval`
over the root alone; in an instance's, the same rules with the instance's
names. A `fn` body is evaluated by `eval`: it sees only its parameters. -/
def ceval : Nat → CEnv → Frame → Locals → Expr → Result Value
  | 0, _, _, _, _ => .error outOfFuel
  | fuel + 1, ce, .root, ls, e => eval fuel ce.root false ls e
  | fuel + 1, ce, f@(.inst c id binds), ls, e =>
  match e with
  | .num b => .ok (.num (F64.ofBits b))
  | .str s => .ok (.str s)
  | .bool b => .ok (.bool b)
  | .none => .ok .none
  | .list items => do .ok (.list (← cevalList fuel ce f ls items))
  | .some e => do .ok (.some (← ceval fuel ce f ls e))
  | .template parts => do
    let ss ← cevalDisplays fuel ce f ls parts
    .ok (.str (String.join ss))
  | .var x =>
    match lookup x ls with
    | .some v => .ok v
    | .none => cvar fuel ce c id binds x
  | .member e fld => do
    match ← ceval fuel ce f ls e with
    | .record shape fields =>
      match ce.root.fieldIndex shape fld with
      | .some i => (fields[i]?).elim (.error (.type s!"short record `{shape}`")) .ok
      | .none => .error (.type s!"`{shape}` has no field `{fld}`")
    | _ => .error (.type "member of a value that is not a record")
  | .call name args =>
    match ce.root.prog.fns.find? (·.name == name) with
    | .some fd => do
      let vs ← cevalList fuel ce f ls args
      let ps := (fd.params.map (·.1)).zip vs
      eval fuel ce.root true ps.reverse fd.body
    | .none =>
      match name, args with
      | "map", [l, .arrow ps body] => do
        let xs ← (← ceval fuel ce f ls l).asList
        let ys ← cevalMap fuel ce f ls ps body xs 0
        .ok (.list ys)
      | "filter", [l, .arrow ps body] => do
        let xs ← (← ceval fuel ce f ls l).asList
        let ys ← cevalFilter fuel ce f ls ps body xs 0
        .ok (.list ys)
      | "pending", [.var x] => cpending fuel ce f ls "pending" x
      | "failed", [.var x] => cpending fuel ce f ls "failed" x
      | "failure", [.var x] => cpending fuel ce f ls "failure" x
      | _, _ => do
        let vs ← cevalList fuel ce f ls args
        stdlib ce.root name vs
  | .record shape base fields => do
    let b ← match base with
      | .none => pure Option.none
      | .some e => do
        match ← ceval fuel ce f ls e with
        | .record _ fs => pure (Option.some fs)
        | _ => .error (.type "record base that is not a record")
    let decl ← (ce.root.shape shape).elim (.error (.type s!"unknown shape `{shape}`")) .ok
    let vs ← cevalFields fuel ce f ls fields b decl.fields 0
    .ok (.record shape vs)
  | .unary .neg e => do
    match ← ceval fuel ce f ls e with
    | .num x => .ok (.num (-x))
    | _ => .error (.type "negation of a value that is not a number")
  | .unary .not e => do
    match ← ceval fuel ce f ls e with
    | .bool b => .ok (.bool !b)
    | _ => .error (.type "`not` of a value that is not a bool")
  | .binary .and a b => do
    match ← ceval fuel ce f ls a with
    | .bool false => .ok (.bool false)
    | .bool true => ceval fuel ce f ls b
    | _ => .error (.type "`and` of a value that is not a bool")
  | .binary .or a b => do
    match ← ceval fuel ce f ls a with
    | .bool true => .ok (.bool true)
    | .bool false => ceval fuel ce f ls b
    | _ => .error (.type "`or` of a value that is not a bool")
  | .binary op a b => do
    let va ← ceval fuel ce f ls a
    let vb ← ceval fuel ce f ls b
    binop op va vb
  | .ternary cnd a b => do
    match ← ceval fuel ce f ls cnd with
    | .bool true => ceval fuel ce f ls a
    | .bool false => ceval fuel ce f ls b
    | _ => .error (.type "condition that is not a bool")
  | .matchOpt s x a b => do
    match ← ceval fuel ce f ls s with
    | .some v => ceval fuel ce f ((x, v) :: ls) a
    | .none => ceval fuel ce f ls b
    | _ => .error (.type "match on a value that is not an option")
  | .arrow _ _ => .error (.type "an arrow outside `map` or `filter`")
  | .letE x v body => do
    let w ← ceval fuel ce f ls v
    ceval fuel ce f ((x, w) :: ls) body
  | .named _ _ => .error (.type "a named argument outside a record or command")
  | .typed e _ => ceval fuel ce f ls e

/-- A name an instance's frame reads that no local binds: a derive (its
body, in the frame, with no locals), a state (the store), an inject or a
prop (its thunk). -/
def cvar : Nat → CEnv → String → InstId → List (String × Expr × Frame × Locals) → String →
    Result Value
  | 0, _, _, _, _, _ => .error outOfFuel
  | fuel + 1, ce, c, id, binds, x => do
    let C ← ce.comp c
    match C.derives.find? (·.name == x) with
    | .some d => ceval fuel ce (.inst c id binds) [] d.body
    | .none =>
      if C.states.any (·.name == x) then
        match ce.store.find id with
        | .some s => (lookup x s).elim (.error (.refused s!"instance slot `{x}` missing")) .ok
        | .none => .error (.refused s!"instance slot `{x}` read outside its instance")
      else
        match lookupBind x binds with
        | .some (e, f', ls') => ceval fuel ce f' ls' e
        | .none => .error (.unbound x)

/-- What `pending(x)`, `failed(x)` (`false`) or `failure(x)` (`none`) says
of a name that is no resource. -/
def quiet (which : String) : Value := if which = "failure" then .none else .bool false

/-- `pending(x)` / `failed(x)` / `failure(x)` in an instance's frame: about
the resource the name stands for when it is a prop (or a derive) that is a
name; a state or a local is no resource. -/
def cpending : Nat → CEnv → Frame → Locals → String → String → Result Value
  | 0, _, _, _, _, _ => .error outOfFuel
  | fuel + 1, ce, .root, ls, which, x => eval fuel ce.root false ls (.call which [.var x])
  | fuel + 1, ce, f@(.inst c _ binds), ls, which, x =>
    match lookup x ls with
    | .some _ => .ok (quiet which)
    | .none => do
      let C ← ce.comp c
      match C.derives.find? (·.name == x) with
      | .some d =>
        match d.body with
        | .var y => cpending fuel ce f [] which y
        | _ => .error (.unsupported s!"roster entry `{which}`")
      | .none =>
        if C.states.any (·.name == x) then .ok (quiet which) else
        match lookupBind x binds with
        | .some (.var y, f', ls') => cpending fuel ce f' ls' which y
        | .some _ => .error (.unsupported s!"roster entry `{which}`")
        | .none => eval fuel ce.root false [] (.call which [.var x])

def cevalList : Nat → CEnv → Frame → Locals → List Expr → Result (List Value)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, [] => .ok []
  | fuel + 1, ce, f, ls, e :: es => do
    let v ← ceval fuel ce f ls e
    let vs ← cevalList fuel ce f ls es
    .ok (v :: vs)

def cevalDisplays : Nat → CEnv → Frame → Locals → List Expr → Result (List String)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, [] => .ok []
  | fuel + 1, ce, f, ls, e :: es => do
    let s ← (← ceval fuel ce f ls e).display
    let ss ← cevalDisplays fuel ce f ls es
    .ok (s :: ss)

def cevalMap : Nat → CEnv → Frame → Locals → List String → Expr → List Value → Nat →
    Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, ce, f, ls, ps, body, x :: xs, i => do
    let y ← ceval fuel ce f (bindParams ps x i ls) body
    let ys ← cevalMap fuel ce f ls ps body xs (i + 1)
    .ok (y :: ys)

def cevalFilter : Nat → CEnv → Frame → Locals → List String → Expr → List Value → Nat →
    Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, ce, f, ls, ps, body, x :: xs, i => do
    let keep ← ceval fuel ce f (bindParams ps x i ls) body
    let ys ← cevalFilter fuel ce f ls ps body xs (i + 1)
    match keep with
    | .bool true => .ok (x :: ys)
    | .bool false => .ok ys
    | _ => .error (.type "filter callback that is not a bool")

def cevalFields : Nat → CEnv → Frame → Locals → List (String × Expr) → Option (List Value) →
    List Field → Nat → Result (List Value)
  | 0, _, _, _, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, _, _, _, [], _ => .ok []
  | fuel + 1, ce, f, ls, written, base, fd :: fs, i => do
    let v ← match lookupField fd.name written with
      | .some e => ceval fuel ce f ls e
      | .none =>
        match base with
        | .some bs => (bs[i]?).elim (.error (.type "short record base")) .ok
        | .none => .error (.type s!"record without field `{fd.name}`")
    let vs ← cevalFields fuel ce f ls written base fs (i + 1)
    .ok (v :: vs)

end

/-! ## Actions -/

/-- A thunk: an expression with the frame and locals it is read in. -/
abbrev Thunk := Expr × Frame × Locals

def forceAll (fuel : Nat) (ce : CEnv) (ts : List Thunk) : Result (List Value) :=
  ts.mapM fun (e, f, ls) => ceval fuel ce f ls e

/-- The action a handler (or a call) names in a frame, with the
arguments curried on the way through `action` props, outermost first,
then `args`. -/
def resolveAction : Nat → CProgram → Frame → Locals → String → List Thunk →
    Result (Frame × String × List Thunk)
  | 0, _, _, _, _, _ => .error outOfFuel
  | _ + 1, p, .root, _, a, args =>
    if p.root.actions.any (·.name == a) then .ok (.root, a, args)
    else .error (.unbound s!"action `{a}`")
  | fuel + 1, p, f@(.inst c _ binds), _, a, args =>
    match p.component? c with
    | .none => .error (.unbound s!"component `{c}`")
    | .some C =>
      if C.actions.any (·.name == a) then .ok (f, a, args) else
      match lookupBind a binds with
      | .some (.var b, f', ls') => resolveAction fuel p f' ls' b args
      | .some (.call b curried, f', ls') =>
        resolveAction fuel p f' ls' b (curried.map (·, f', ls') ++ args)
      | .some _ => .error (.refused s!"`{a}` does not name an action")
      | .none => .error (.unbound s!"action `{a}`")

/-- What a component-level action body asks for: the root's effects (as
`Contract.Runtime` records them) and the writes to instances' states. -/
structure CEffects where
  flat : Effects := {}
  /-- (instance, its component, state, value), in statement order. -/
  inst : List (InstId × String × String × Value) := []
  deriving Inhabited

/-- The action `a` of a frame's component, its own calls made calls. -/
def _root_.Contract.Components.CProgram.actionIn (p : CProgram) : Frame → String → Option ActionDecl
  | .root, a => (ownCallsComponent p.root).actions.find? (·.name == a)
  | .inst c _ _, a => (p.component? c).bind fun C => (ownCallsComponent C).actions.find? (·.name == a)

/-- Run a block in a frame. In the root's frame it is `exec` over the
root alone. In an instance's frame an assignment writes the instance's
state; a call of the component's own action runs its statements here, in
the same frame, its parameters alone bound; and a command naming an
`action` prop or injected action is a call (LLP 1089): the action it names
runs its statements here, in its own frame, its parameters bound to the
curried arguments then the call's. Every statement reads the state the
action started with. -/
def cexec : Nat → CEnv → Frame → Locals → List Stmt → CEffects → Result CEffects
  | 0, _, _, _, _, _ => .error outOfFuel
  | fuel + 1, ce, .root, ls, ss, fx => do
    let flat ← exec fuel ce.root ls ss fx.flat
    pure { fx with flat }
  | _ + 1, _, .inst _ _ _, _, [], fx => .ok fx
  | fuel + 1, ce, f@(.inst c id _), ls, s :: rest, fx => do
    let C ← ce.comp c
    match s with
    | .letS x e => do
      let v ← ceval fuel ce f ls e
      cexec fuel ce f ((x, v) :: ls) rest fx
    | .assign t e => do
      let v ← ceval fuel ce f ls e
      if C.states.any (·.name == t) then
        cexec fuel ce f ls rest { fx with inst := fx.inst ++ [(id, c, t, v)] }
      else .error (.unbound t)
    | .command name args =>
      if (C.props ++ C.injects).any (fun pd => pd.name == name && pd.action) then do
        let (f', a, ts) ← resolveAction fuel ce.prog f ls name (args.map (·, f, ls))
        let vs ← forceAll fuel ce ts
        match ce.prog.actionIn f' a with
        | .none => .error (.unbound a)
        | .some ad =>
          if ad.params.length != vs.length then .error (.refused "call arity") else
          let fx ← cexec fuel ce f' ((ad.params.map (·.1)).zip vs).reverse ad.body fx
          cexec fuel ce f ls rest fx
      else do
        let vs ← cevalList fuel ce f ls args
        cexec fuel ce f ls rest { fx with flat := { fx.flat with commands := fx.flat.commands ++ [(name, vs)] } }
    | .send t src args => do
      let vs ← cevalList fuel ce f ls args
      cexec fuel ce f ls rest { fx with flat := { fx.flat with sends := fx.flat.sends ++ [(t, src, vs)] } }
    | .refresh t => cexec fuel ce f ls rest { fx with flat := { fx.flat with refreshes := fx.flat.refreshes ++ [t] } }
    | .call a args => do
      let vs ← cevalList fuel ce f ls args
      match ce.prog.actionIn f a with
      | .none => .error (.unbound a)
      | .some ad =>
        if ad.params.length != vs.length then .error (.refused "call arity") else
        let fx ← cexec fuel ce f ((ad.params.map (·.1)).zip vs).reverse ad.body fx
        cexec fuel ce f ls rest fx
    | .ifS cnd thn els => do
      let fx ← match ← ceval fuel ce f ls cnd with
        | .bool true => cexec fuel ce f ls thn fx
        | .bool false => cexec fuel ce f ls els fx
        | _ => .error (.type "`if` on a value that is not a bool")
      cexec fuel ce f ls rest fx
    | .matchS subj x sm nn => do
      let fx ← match ← ceval fuel ce f ls subj with
        | .some v => cexec fuel ce f ((x, v) :: ls) sm fx
        | .none => cexec fuel ce f ls nn fx
        | _ => .error (.type "`match` on a value that is not an option")
      cexec fuel ce f ls rest fx

/-! ## Rendering -/

/-- A resolved handler: the action, the frame it runs in, and its
argument thunks (curried first). -/
structure CHandler where
  event : String
  frame : Frame
  action : String
  args : List Thunk
  deriving Inhabited

structure CVNode where
  tag : String
  testId : Option String
  text : Option String
  handlers : List CHandler
  control : Option String := .none
  windowed : Bool := false
  children : List CVNode
  deriving Inhabited

/-- A provided binding in force: its name and thunk. -/
abbrev Provided := String × Expr × Frame × Locals

/-- A `slot` component's fill: the nodes under the use, with the use
site's frame, locals, providers and own fill. -/
inductive Fill where
  | mk (nodes : List CNode) (frame : Frame) (ls : Locals) (provides : List Provided)
      (outer : Option Fill)
  deriving Inhabited

structure RCx where
  /-- What evaluation reads: the store holds the instances on the way here. -/
  env : CEnv
  /-- The store before this render: an instance found there keeps its states. -/
  prev : Store
  frame : Frame
  /-- The component whose view is being rendered (named in the path). -/
  comp : String
  path : InstId
  provides : List Provided
  fill : Option Fill

def RCx.enter (cx : RCx) (node : Nat) (key : Value) (dup : Nat) : RCx :=
  { cx with path := cx.path ++ [{ comp := cx.comp, node, key, dup }] }

def nearest (x : String) (ps : List Provided) : Option (Expr × Frame × Locals) :=
  lookupBind x ps.reverse

def ctext (fuel : Nat) (ce : CEnv) (f : Frame) (ls : Locals) (tag : String) (pos : List Expr) :
    Result (Option String) :=
  match pos with
  | e :: _ =>
    if tag == "text" || tag == "tspan" || tag == "option" then do
      let v ← ceval fuel ce f ls e
      pure (Option.some (← v.display))
    else pure Option.none
  | [] => pure Option.none

def ctestId (fuel : Nat) (ce : CEnv) (f : Frame) (ls : Locals) (props : List (String × Expr)) :
    Result (Option String) :=
  match lookupField "testId" props with
  | .some e => do pure (Option.some (← (← ceval fuel ce f ls e).display))
  | .none => pure Option.none

/-- An instance's states: kept from the store before the render, or
initialized now in declaration order, each in the instance's frame (the
states before it already readable). -/
def instSlots (fuel : Nat) (cx : RCx) (C : CComponent) (f : Frame) (id : InstId) :
    Result (List (String × Value)) :=
  match cx.prev.find id with
  | .some s => .ok s
  | .none => do
    let mut s : List (String × Value) := []
    for st in C.states do
      let ce := { cx.env with store := (id, s) :: cx.env.store }
      let v ← ceval fuel ce f [] st.init
      if !conforms (rootProgram cx.env.prog) v st.ty then
        throw (.refused s!"slot `{st.name}` initialized with a value of the wrong type")
      s := s ++ [(st.name, v)]
    pure s

/-- Render a component's view. Returns the elements and the instances
live after it (their states, kept or initialized now). -/
def crender : Nat → RCx → Locals → List CNode → Store → Result (List CVNode × Store)
  | 0, _, _, _, _ => .error outOfFuel
  | _ + 1, _, _, [], live => .ok ([], live)
  | fuel + 1, cx, ls, n :: rest, live => do
    let (here, live) ← (match n with
      | .element tag pos props hs kids => do
        let text ← ctext fuel cx.env cx.frame ls tag pos
        let testId ← ctestId fuel cx.env cx.frame ls props
        let handlers ← hs.mapM fun (ev, a, args) => do
          let (f, act, ts) ← resolveAction fuel cx.env.prog cx.frame ls a (args.map (·, cx.frame, ls))
          pure ({ event := ev, frame := f, action := act, args := ts } : CHandler)
        let (kids, live) ← crender fuel cx ls kids live
        pure ([{ tag, testId, text, handlers,
                 control := elementControl tag props,
                 windowed := tag == "list" && (lookupField "virtualized" props matches .some (.bool true)),
                 children := kids : CVNode }], live)
      | .when id c thn els => do
        match ← ceval fuel cx.env cx.frame ls c with
        | .bool true => crender fuel (cx.enter id (.num 0) 0) ls thn live
        | .bool false => crender fuel (cx.enter id (.num 1) 0) ls els live
        | _ => .error (.type "`when` on a value that is not a bool")
      | .matchN id s x sm nn => do
        match ← ceval fuel cx.env cx.frame ls s with
        | .some v => crender fuel (cx.enter id (.num 0) 0) ((x, v) :: ls) sm live
        | .none => crender fuel (cx.enter id (.num 1) 0) ls nn live
        | _ => .error (.type "`match` on a value that is not an option")
      | .each id x ix list key body => do
        let items ← (← ceval fuel cx.env cx.frame ls list).asList
        crenderRows fuel cx ls id x ix key body items 0 [] live
      | .use id name args fill => do
        let C ← cx.env.comp name
        let path := cx.path ++ [{ comp := cx.comp, node := id }]
        let pbinds ← C.props.mapM fun pd =>
          match lookupField pd.name args with
          | .some e => .ok (pd.name, e, cx.frame, ls)
          | .none => .error (.refused s!"`{name}` needs `{pd.name}`")
        let ibinds ← C.injects.mapM fun pd =>
          match nearest pd.name cx.provides with
          | .some (e, f', ls') => .ok (pd.name, e, f', ls')
          | .none => .error (.refused s!"nothing provides `{pd.name}`")
        let f := Frame.inst name path (ibinds ++ pbinds)
        let slots ← instSlots fuel cx C f path
        let cx' : RCx := { cx with
          env := { cx.env with store := (path, slots) :: cx.env.store },
          frame := f, comp := name, path,
          provides := cx.provides ++ C.provides.map fun (x, e) => (x, e, f, []),
          fill := if C.slot then Option.some (.mk fill cx.frame ls cx.provides cx.fill) else Option.none }
        crender fuel cx' [] C.view (live ++ [(path, slots)])
      | .children id =>
        match cx.fill with
        | .none => .error (.refused "`children` outside a `slot` component")
        | .some (.mk nodes f ls' provides outer) =>
          let cname : String := (match f with
            | .root => cx.env.prog.root.name
            | .inst c _ _ => c)
          let step : Step := { comp := cx.comp, node := id }
          let cx' : RCx := { cx with frame := f, comp := cname, path := cx.path ++ [step],
                                     provides := provides, fill := outer }
          crender fuel cx' ls' nodes live : Result (List CVNode × Store))
    let (more, live) ← crender fuel cx ls rest live
    pure (here ++ more, live)
where
  crenderRows : Nat → RCx → Locals → Nat → String → Option String → Expr → List CNode →
      List Value → Nat → List Value → Store → Result (List CVNode × Store)
    | 0, _, _, _, _, _, _, _, _, _, _, _ => .error outOfFuel
    | _ + 1, _, _, _, _, _, _, _, [], _, _, live => .ok ([], live)
    | fuel + 1, cx, ls, id, x, ix, key, body, item :: items, i, seen, live => do
      let ls' := (x, item) :: ls
      let ls' := match ix with
        | .some n => (n, Value.num (F64.ofNat i)) :: ls'
        | .none => ls'
      let k ← rowKey (← ceval fuel cx.env cx.frame ls' key)
      let dup := (seen.filter (Value.same k ·)).length
      let (vs, live) ← crender fuel (cx.enter id k dup) ls' body live
      let (more, live) ← crenderRows fuel cx ls id x ix key body items (i + 1) (seen ++ [k]) live
      pure (vs ++ more, live)

/-! ## Configurations and steps -/

structure CConfig where
  slots : List (String × Value)
  settled : Settled
  store : Store
  view : List CVNode
  now : F64
  timers : List Timer
  poisoned : Bool := false
  commands : List (String × List Value) := []
  /-- The `then`s armed, as `Contract.Config.armed`. -/
  armed : List (String × F64) := []
  deriving Inhabited

def CConfig.empty : CConfig :=
  { slots := [], settled := {}, store := [], view := [], now := 0, timers := [] }

def rootEnv (p : CProgram) (slots : List (String × Value)) (st : Settled) (now : F64) : Env :=
  { prog := rootProgram p, slots, derives := st.derives, resources := st.resources, now }

/-- Render the root's view against the root's slots and settled values,
keeping the instances `prev` holds. -/
def renderRoot (p : CProgram) (slots : List (String × Value)) (st : Settled) (now : F64)
    (prev : Store) : Result (List CVNode × Store) :=
  let env : CEnv := { prog := p, root := rootEnv p slots st now, store := [] }
  crender fuel { env, prev, frame := .root, comp := p.root.name, path := [],
                 provides := p.root.provides.map fun (x, e) => (x, e, .root, []),
                 fill := .none } [] p.root.view []

/-- The declared type of an instance's state. -/
def instSlotTy (p : CProgram) (c x : String) : Ty :=
  (((p.component? c).bind fun C => C.states.find? (·.name == x)).map (·.ty)).getD .unknown

def applyInstWrites (store : Store) (ws : List (InstId × String × String × Value)) : Store :=
  ws.foldl (fun store (id, _, x, v) =>
    store.map fun (i, s) => if InstId.same i id then (i, setSlot s x v) else (i, s)) store

/-- Run an action in a frame as one commit. -/
def crunAction (p : CProgram) (o : Oracle) (c : CConfig) (f : Frame) (name : String)
    (args : List Value) : CConfig × Outcome :=
  let rp := rootProgram p
  let refuse (e : Err) := (c, Outcome.refused e)
  if c.poisoned then refuse (.refused "poisoned") else
  match p.actionIn f name with
  | .none => refuse (.unbound name)
  | .some a =>
    if a.params.length != args.length then refuse (.refused "arity") else
    if !((a.params.zip args).all fun ((_, t), v) => conforms rp v t) then
      refuse (.refused "an argument of the wrong type") else
    let ce : CEnv := { prog := p, root := rootEnv p c.slots c.settled c.now, store := c.store }
    let ls : Locals := ((a.params.map (·.1)).zip args).reverse
    match cexec fuel ce f ls a.body {} with
    | .error e => refuse e
    | .ok cfx =>
      let fx := cfx.flat
      let answered : Result (List (String × Value)) := fx.sends.mapM fun (m, src, vs) => do
        let v ← o.ask src vs
        let ty := ((rp.mutations.find? (·.name == m)).map (·.ty)).getD .unknown
        if !conforms rp v ty then throw (.refused s!"mutation `{m}` answered with the wrong shape")
        pure (m, Value.some v)
      match answered with
      | .error e => refuse e
      | .ok answered =>
        if !(fx.writes.all fun (x, v) => conforms rp v (slotTy rp x)) then
          refuse (.refused "a write of the wrong type") else
        if !(cfx.inst.all fun (_, comp, x, v) => conforms rp v (instSlotTy p comp x)) then
          refuse (.refused "a write of the wrong type") else
        let slots := (answered ++ fx.writes).foldl (fun s (x, v) => setSlot s x v) c.slots
        if !routerValid rp slots then refuse (.refused "an invalid router value") else
        let store := applyInstWrites c.store cfx.inst
        match settle rp o slots c.now c.settled fx.refreshes with
        | .error e => refuse e
        | .ok st =>
          match renderRoot p slots st c.now store with
          | .ok (view, live) =>
            ({ c with slots, settled := st, store := live, view,
                      commands := c.commands ++ fx.commands,
                      armed := armThens rp c.armed fx.sends c.now }, .ok)
          | .error e =>
            ({ c with slots, settled := st, store, poisoned := true,
                      commands := c.commands ++ fx.commands }, .poisoned e)

/-- Boot: the root's slots, its timers, settlement, then the root's view,
every instance initialized as it first renders. -/
def cboot (p : CProgram) (o : Oracle) : CConfig × Outcome :=
  let rp := rootProgram p
  let empty := CConfig.empty
  match initSlots rp with
  | .error e => (empty, .refused e)
  | .ok slots =>
    match startTimers rp slots with
    | .error e => (empty, .refused e)
    | .ok timers =>
      -- Queued sends and gated tasks (LLP 1092) are the flat semantics'
      -- alone: `difftest expansion` writes programs without them.
      if rp.mutations.any (·.queue) || rp.tasks.any (fun t => t.gate.isSome || t.key.isSome) then
        (empty, .refused (.unsupported "queued sends and gated tasks")) else
      match settle rp o slots 0 with
      | .error e => (empty, .refused e)
      | .ok st =>
        match renderRoot p slots st 0 [] with
        | .ok (view, live) => ({ slots, settled := st, store := live, view, now := 0, timers }, .ok)
        | .error e => (empty, .refused e)

def cfindTestId (id : String) : List CVNode → Option CVNode
  | [] => .none
  | n@{ children, .. } :: rest =>
    if n.testId == Option.some id then .some n
    else match cfindTestId id children with
      | .some m => .some m
      | .none => cfindTestId id rest

def cdispatch (p : CProgram) (o : Oracle) (c : CConfig) (target : String) (event : String)
    (payload : Option Value) : CConfig × Outcome :=
  if c.poisoned then (c, .refused (.refused "poisoned")) else
  match cfindTestId target c.view with
  | .none => (c, .refused (.refused s!"no element `{target}`"))
  | .some n =>
    match n.handlers.find? (·.event == event) with
    | .none => (c, .refused (.refused s!"`{target}` has no {event} handler"))
    | .some h =>
      if payload.isSome && n.control.isSome then
        (c, .refused (.unsupported s!"a payload for a `{n.control.getD ""}` control")) else
      let ce : CEnv := { prog := p, root := rootEnv p c.slots c.settled c.now, store := c.store }
      match forceAll fuel ce h.args with
      | .error e => (c, .refused e)
      | .ok vs => crunAction p o c h.frame h.action (vs ++ payload.toList)

/-- `Contract.advance`, with the root's actions run as above. -/
def cadvance (p : CProgram) (o : Oracle) (c : CConfig) (t : F64) : CConfig × Outcome :=
  let due (c : CConfig) : Option (Timer × Nat) :=
    (c.timers.zipIdx).foldl (fun best (tm, i) =>
      if tm.next ≤ t then
        match best with
        | .none => Option.some (tm, i)
        | .some (b, j) => if tm.next < b.next then Option.some (tm, i) else Option.some (b, j)
      else best) Option.none
  let pick (c : CConfig) : Option (String × F64 × String) :=
    match dueThen (rootProgram p) c.armed t, due c with
    | .some (m, w, a), .some (tm, _) => if w ≤ tm.next then Option.some (m, w, a) else Option.none
    | th, _ => th
  let finish (c : CConfig) : CConfig × Outcome := ({ c with now := if t > c.now then t else c.now }, .ok)
  let rec go : Nat → CConfig → CConfig × Outcome
    | 0, c =>
      match pick c, due c with
      | .none, .none => finish c
      | _, _ => (c, .refused (.refused "too many timer fires"))
    | n + 1, c =>
      if c.poisoned then (c, .refused (.refused "poisoned")) else
      match pick c with
      | .some (m, w, a) =>
        let c := { c with armed := c.armed.filter (·.1 != m), now := if c.now < w then w else c.now }
        match crunAction p o c .root a [] with
        | (c, .ok) => go n c
        | (c, out) => (c, out)
      | .none =>
      match due c with
      | .none => finish c
      | .some (tm, i) =>
        let timers := c.timers.set i tm.fired
        let c := { c with timers, now := tm.next }
        match crunAction p o c .root tm.action [] with
        | (c, .ok) => go n c
        | (c, out) => (c, out)
  if t < c.now then (c, .ok) else go timerFireLimit c

/-! ## Observation -/

partial def CVNode.toVNode (n : CVNode) : VNode :=
  { tag := n.tag, testId := n.testId, text := n.text, handlers := [], locals := [],
    control := n.control, windowed := n.windowed, rows := [],
    children := n.children.map CVNode.toVNode }

/-- The configuration as `Contract.Observe` prints it: the root's slots,
derives and resources, the commands, the view. -/
def CConfig.toConfig (c : CConfig) : Config :=
  { slots := c.slots, settled := c.settled, store := [], view := c.view.map CVNode.toVNode,
    now := c.now, timers := c.timers, poisoned := c.poisoned, commands := c.commands,
    armed := c.armed }

/-- `Contract.Observe.dispatchAt` for the unexpanded file: the work due by
`c.now` first, then the event. -/
def cdispatchAt (p : CProgram) (o : Oracle) (c : CConfig) (target event : String)
    (payload : Option Value) : CConfig × Outcome :=
  match cfindTestId target c.view with
  | .none => cdispatch p o c target event payload
  | .some n =>
    if (n.handlers.find? (·.event == event)).isNone then cdispatch p o c target event payload else
    match cadvance p o c c.now with
    | (c₁, .poisoned e) => (c₁, .poisoned e)
    | (c₁, o₁) =>
      match cdispatch p o c₁ target event payload with
      | (c₂, .ok) => (c₂, o₁)
      | (c₂, o₂) => (c₂, o₂)

def cstep (p : CProgram) (o : Oracle) (c : CConfig) : Observe.Event → CConfig × Outcome
  | .tap t => cdispatchAt p o c t "press" .none
  | .change t s => cdispatchAt p o c t "change" (.some (.str s))
  | .clock ms => cadvance p o c (c.now + ms)
  | .other t e v => cdispatchAt p o c t e v

/-- `Contract.Observe.run` for the unexpanded file. -/
def crun (p : CProgram) (o : Oracle) (events : List Observe.Event) : List String := Id.run do
  let rp := rootProgram p
  let (c, out) := cboot p o
  match out with
  | .ok => pure ()
  | _ => return ["== boot", Observe.outcome out]
  let mut acc := Observe.lines rp "boot" c.toConfig out 0
  let mut c := c
  for e in events do
    let before := c.commands.length
    let (c', out) := cstep p o c e
    acc := acc ++ Observe.lines rp e.label c'.toConfig out before
    c := c'
    match out with
    | .poisoned _ => return acc
    | _ => pure ()
  return acc

end Contract.CompSem
