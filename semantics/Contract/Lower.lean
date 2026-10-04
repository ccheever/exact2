/-
The compiler from the semantics' expressions and statements to VM code,
mirroring `contract/lower/src/expr.rs` and `stmts.rs` instruction for
instruction: names resolved through a scope to slot, derive, resource,
parameter and local indices (the Rust `Scope`), short-circuit `and`/`or`
through a local, `match` through `JumpIfNone`/`Unwrap`/`BindLocal`,
templates through `toString` and `Concat`, records with a base bound as a
local, `fn` calls expanded inline with their arguments as locals, and
`map`/`filter` callbacks inline after their opcode.

Like the Rust compiler, this one carries a static type to choose
instructions (`Concat` or `Add`, a field's index, whether a template part
needs `toString`). Its types are its own (`STy`): sound by construction
(`Contract.LowerProof` proves every value an expression has is of its
static type), so it refuses (`.error`) where it cannot know, where the Rust
compiler trusts its checker.
-/
import Contract.Vm
import Contract.Runtime

namespace Contract.Lower

open Vm

/-- A static type. `top` says nothing; `bot` has no values (the item type
of `[]`, the inner type of `none`). -/
inductive STy where
  | top
  | bot
  | number
  | bool
  | string
  | unit
  | option (t : STy)
  | list (t : STy)
  | record (shape : String)
  deriving Repr, Inhabited, DecidableEq

/-- A declared type. An open type (`unknown`) says nothing. -/
def STy.ofTy : Ty → STy
  | .number => .number
  | .bool => .bool
  | .string => .string
  | .unit => .unit
  | .option t => .option (STy.ofTy t)
  | .list t => .list (STy.ofTy t)
  | .record s => .record s
  | .unknown => .top

/-- The least type both are (a ternary's, a `match`'s). -/
def STy.join : STy → STy → STy
  | .bot, t => t
  | t, .bot => t
  | .option a, .option b => .option (STy.join a b)
  | .list a, .list b => .list (STy.join a b)
  | a, b => if a = b then a else .top

/-- `a` is a subtype of `b`: every value of `a` is one of `b`. -/
def STy.sub : STy → STy → Bool
  | .bot, _ => true
  | _, .top => true
  | .option a, .option b => STy.sub a b
  | .list a, .list b => STy.sub a b
  | a, b => decide (a = b)

/-- What an option of this type holds. -/
def STy.inner : STy → STy
  | .option t => t
  | _ => .top

/-- What a list of this type holds. -/
def STy.item : STy → STy
  | .list t => t
  | _ => .top

/-- The declared type where the inferred one is a subtype of it (a `fn`'s
parameter and result, an ascription), else the inferred one. -/
def STy.refine (inferred declared : STy) : STy :=
  if inferred.sub declared then declared else inferred

/-- What a name resolves to. -/
inductive Ref where
  | slot (i : Nat)
  | derive (i : Nat)
  | resource (i : Nat)
  /-- A mutation: its slot and its row. -/
  | mutation (slot m : Nat)
  | param (i : Nat)
  | item (depth : Nat)
  | index (depth : Nat)
  | bound (depth : Nat)
  | local (i : Nat)
  deriving Repr, BEq, Inhabited, DecidableEq

/-- Names in scope, innermost first. -/
abbrev Scope := List (String × Ref × STy)

def scopeLookup (x : String) : Scope → Option (Ref × STy)
  | [] => .none
  | (y, r, t) :: rest => if x == y then .some (r, t) else scopeLookup x rest

/-- Where the plan put each declaration: name to row index. -/
structure Layout where
  slots : List (String × Nat) := []
  derives : List (String × Nat) := []
  resources : List (String × Nat) := []
  /-- A mutation's slot and row. -/
  mutations : List (String × Nat × Nat) := []
  deriving Inhabited

def find? {α} (x : String) : List (String × α) → Option α
  | [] => .none
  | (y, a) :: rest => if x == y then .some a else find? x rest

/-- The component's scope: states, derives, resources, mutations, in the
checker's order. -/
def globalScope (p : Program) (L : Layout) : Scope :=
  (p.states.filterMap fun s => (find? s.name L.slots).map fun i => (s.name, Ref.slot i, STy.ofTy s.ty)) ++
  (p.derives.filterMap fun d => (find? d.name L.derives).map fun i => (d.name, Ref.derive i, STy.ofTy d.ty)) ++
  (p.resources.filterMap fun r => (find? r.name L.resources).map fun i => (r.name, Ref.resource i, STy.ofTy r.ty)) ++
  (p.mutations.filterMap fun m => (find? m.name L.mutations).map fun (s, k) =>
    (m.name, Ref.mutation s k, STy.option (STy.ofTy m.ty)))

/-- The roster as the compiler knows it: each entry's name. -/
def roster : List String :=
  ["now", "formatTime", "length", "isEmpty", "toString", "floor", "max", "min", "open", "push",
   "replace", "back", "select", "go", "stack", "top", "depth", "params", "searchParam",
   "encodeURIComponent", "encodeRouteSegment", "includes", "trim", "first", "t", "map",
   "filter", "join", "formatDate", "formatNumber", "frame", "measure", "at", "startsWith",
   "endsWith", "slice", "replaceAll", "toLowerCase"]

/-- A roster entry's result type. -/
def rosterTy (f : String) (args : List STy) : STy :=
  if f = "now" ∨ f = "length" ∨ f = "floor" ∨ f = "max" ∨ f = "min" then .number
  else if f = "isEmpty" ∨ f = "includes" ∨ f = "startsWith" ∨ f = "endsWith" then .bool
  else if f = "toString" ∨ f = "trim" ∨ f = "encodeURIComponent" ∨ f = "join" ∨ f = "slice"
    ∨ f = "replaceAll" ∨ f = "toLowerCase" then .string
  else if f = "first" ∨ f = "at" then
    match args with
    | .list t :: _ => .option t
    | _ => .top
  else .top

def refInstr : Ref → Instr
  | .slot i => .loadSlot i
  | .derive i => .loadDerive i
  | .resource i => .loadResource i
  | .mutation s _ => .loadSlot s
  | .param i => .loadParam i
  | .item d => .loadItem d
  | .index d => .loadIndex d
  | .bound d => .loadBound d
  | .local i => .loadLocal i

/-- The strict binary operators' opcodes; `+` is `Concat` after a string
and `Add` after a number. -/
def binInstr (op : BinOp) (ta : STy) : Except String (Instr × STy) :=
  match op with
  | .add => if ta = .string then .ok (.concat, .string) else if ta = .number then .ok (.add, .number)
    else .error "`+` of an operand that is not known to be a number or a string"
  | .sub => .ok (.sub, .number) | .mul => .ok (.mul, .number) | .div => .ok (.div, .number)
  | .rem => .ok (.rem, .number)
  | .eq => .ok (.eq, .bool) | .ne => .ok (.ne, .bool) | .lt => .ok (.lt, .bool)
  | .le => .ok (.le, .bool) | .gt => .ok (.gt, .bool) | .ge => .ok (.ge, .bool)
  | .and | .or => .error "short-circuit"

/-- A callback's parameters as locals `n` (the item) and `n + 1` (its
index), innermost first as `bindParams` binds them. -/
def bindScope (ps : List String) (n : Nat) (item : STy) (sc : Scope) : Scope :=
  match ps with
  | [] => sc
  | [p] => (p, .local n, item) :: sc
  | p :: q :: _ => (q, .local (n + 1), .number) :: (p, .local n, item) :: sc

/-- A `fn`'s parameters as the locals from `n`, the last first (as the
semantics binds them, `zip … |>.reverse`). -/
def fnEntries : List String → Nat → List STy → Scope
  | p :: ps, n, t :: ts => (p, .local n, t) :: fnEntries ps (n + 1) ts
  | _, _, _ => []

def fnScope (ps : List String) (n : Nat) (ts : List STy) : Scope := (fnEntries ps n ts).reverse

/-- The templates whose parts are all literal: one string. -/
def literalParts : List Expr → Option String
  | [] => .some ""
  | .str s :: rest => (literalParts rest).map (s ++ ·)
  | _ => .none

/-- The deepest `fn` expansion the Rust compiler admits. -/
def fnDepthLimit : Nat := 32

mutual

/-- Compile `e` in scope `sc` with `n` locals in force: its code and its
static type. `fuel` bounds the recursion (a `fn` body is not a subterm);
`depth` counts the `fn` expansions around `e`. -/
def compile : Nat → Program → Nat → Scope → Nat → Expr → Except String (Code × STy)
  | 0, _, _, _, _, _ => .error "out of fuel"
  | fuel + 1, p, depth, sc, n, e =>
  match e with
  | .num b => .ok ([.num b], .number)
  | .str s => .ok ([.str s], .string)
  | .bool b => .ok ([.bool b], .bool)
  | .none => .ok ([.none], .option .bot)
  | .emptyList => .ok ([.list 0], .list .bot)
  | .some e => do
    let (c, t) ← compile fuel p depth sc n e
    .ok (c ++ [.some], .option t)
  | .template parts =>
    match literalParts parts with
    | .some s => .ok ([.str s], .string)
    | .none => do
      let c ← compileParts fuel p depth sc n true parts
      .ok (c, .string)
  | .var x =>
    match scopeLookup x sc with
    | .some (r, t) => .ok ([refInstr r], t)
    | .none => .error s!"unknown name `{x}`"
  | .member e f => do
    let (c, t) ← compile fuel p depth sc n e
    match t with
    | .record s =>
      match p.shapes.find? (·.name == s) with
      | .some sh =>
        match sh.fields.findIdx? (·.name == f) with
        | .some i =>
          match sh.fields[i]? with
          | .some fd => .ok (c ++ [.field i], STy.ofTy fd.ty)
          | .none => .error "field"
        | .none => .error s!"`{s}` has no field `{f}`"
      | .none => .error s!"unknown shape `{s}`"
    | _ => .error "a member of a value not known to be a record"
  | .call name args =>
    match p.fns.find? (·.name == name) with
    | .some fd =>
      if depth > fnDepthLimit then .error s!"`{name}` expands too deeply" else
      if args.length ≠ fd.params.length then .error "arity" else do
      let (ca, ts) ← compileBind fuel p depth sc n args
      let ts := (ts.zip fd.params).map fun (t, (_, d)) => t.refine (STy.ofTy d)
      let (cb, tb) ← compile fuel p (depth + 1) (fnScope (fd.params.map (·.1)) n ts)
        (n + args.length) fd.body
      .ok (ca ++ cb ++ List.replicate args.length .dropLocal, tb.refine (STy.ofTy fd.ret))
    | .none =>
      match name, args with
      | "map", [l, .arrow ps body] => do
        if ps.length > 2 then .error "a callback takes the item and its index" else
        let (cl, tl) ← compile fuel p depth sc n l
        let item := tl.item
        let (cb, tb) ← compile fuel p depth (bindScope ps n item sc) (n + 2) body
        .ok (cl ++ [.map cb.length] ++ cb, .list tb)
      | "filter", [l, .arrow ps body] => do
        if ps.length > 2 then .error "a callback takes the item and its index" else
        let (cl, tl) ← compile fuel p depth sc n l
        let item := tl.item
        let (cb, _) ← compile fuel p depth (bindScope ps n item sc) (n + 2) body
        .ok (cl ++ [.filter cb.length] ++ cb, .list item)
      | "pending", [.var x] =>
        match scopeLookup x sc with
        | .some (.resource i, _) => .ok ([.pendingResource i], .bool)
        | .some (.mutation _ k, _) => .ok ([.pendingMutation k], .bool)
        | _ => .error s!"`{x}` is not a resource or a mutation"
      | "failed", [.var x] =>
        match scopeLookup x sc with
        | .some (.resource i, _) => .ok ([.failedResource i], .bool)
        | _ => .error s!"`{x}` is not a resource"
      | _, _ =>
        if name = "map" ∨ name = "filter" ∨ name = "pending" ∨ name = "failed" ∨ name = "t"
          ∨ name = "path" then .error s!"`{name}` as written"
        else if !roster.contains name then .error s!"`{name}` is not in the roster"
        else do
          let (c, ts) ← compileArgs fuel p depth sc n args
          .ok (c ++ [.call name args.length], rosterTy name ts)
  | .record shape base fields =>
    match p.shapes.find? (·.name == shape) with
    | .none => .error s!"unknown shape `{shape}`"
    | .some decl =>
      match base with
      | .none => do
        let (c, ok) ← compileFields fuel p depth sc n fields .none shape decl.fields 0
        .ok (c ++ [.record shape decl.fields.length], if ok then .record shape else .top)
      | .some b => do
        let (cb, tb) ← compile fuel p depth sc n b
        match tb with
        | .record s' => do
          let (c, ok) ← compileFields fuel p depth sc (n + 1) fields (.some (n, s')) shape decl.fields 0
          .ok (cb ++ [.bindLocal] ++ c ++ [.record shape decl.fields.length, .dropLocal],
               if ok then .record shape else .top)
        | _ => .error "a record base not known to be a record"
  | .unary .neg e => do
    let (c, _) ← compile fuel p depth sc n e
    .ok (c ++ [.neg], .number)
  | .unary .not e => do
    let (c, _) ← compile fuel p depth sc n e
    .ok (c ++ [.not], .bool)
  | .binary .and a b => do
    let (ca, _) ← compile fuel p depth sc n a
    let (cb, tb) ← compile fuel p depth sc n b
    .ok (ca ++ [.bindLocal, .loadLocal n, .jumpIfFalse (cb.length + 2), .dropLocal] ++ cb ++
         [.jump 2, .loadLocal n, .dropLocal], STy.join .bool tb)
  | .binary .or a b => do
    let (ca, _) ← compile fuel p depth sc n a
    let (cb, tb) ← compile fuel p depth sc n b
    .ok (ca ++ [.bindLocal, .loadLocal n, .not, .jumpIfFalse (cb.length + 2), .dropLocal] ++ cb ++
         [.jump 2, .loadLocal n, .dropLocal], STy.join .bool tb)
  | .binary op a b => do
    let (ca, ta) ← compile fuel p depth sc n a
    let (cb, _) ← compile fuel p depth sc n b
    let (i, t) ← binInstr op ta
    .ok (ca ++ cb ++ [i], t)
  | .ternary c a b => do
    let (cc, _) ← compile fuel p depth sc n c
    let (ca, ta) ← compile fuel p depth sc n a
    let (cb, tb) ← compile fuel p depth sc n b
    .ok (cc ++ [.jumpIfFalse (ca.length + 1)] ++ ca ++ [.jump cb.length] ++ cb, STy.join ta tb)
  | .matchOpt s x a b => do
    let (cs, ts) ← compile fuel p depth sc n s
    let inner := ts.inner
    let (ca, ta) ← compile fuel p depth ((x, .local n, inner) :: sc) (n + 1) a
    let (cb, tb) ← compile fuel p depth sc n b
    .ok (cs ++ [.jumpIfNone (ca.length + 4), .unwrap, .bindLocal] ++ ca ++
         [.dropLocal, .jump (cb.length + 1), .pop] ++ cb, STy.join ta tb)
  | .arrow _ _ => .error "an arrow outside `map` or `filter`"
  | .letE x v body => do
    let (cv, tv) ← compile fuel p depth sc n v
    let (cb, tb) ← compile fuel p depth ((x, .local n, tv) :: sc) (n + 1) body
    .ok (cv ++ [.bindLocal] ++ cb ++ [.dropLocal], tb)
  | .named _ _ => .error "a named argument outside a record or command"
  | .typed e ty => do
    let (c, t) ← compile fuel p depth sc n e
    .ok (c, t.refine (STy.ofTy ty))

/-- Arguments, each left on the stack. -/
def compileArgs : Nat → Program → Nat → Scope → Nat → List Expr → Except String (Code × List STy)
  | 0, _, _, _, _, _ => .error "out of fuel"
  | _ + 1, _, _, _, _, [] => .ok ([], [])
  | fuel + 1, p, depth, sc, n, e :: es => do
    let (c, t) ← compile fuel p depth sc n e
    let (cs, ts) ← compileArgs fuel p depth sc n es
    .ok (c ++ cs, t :: ts)

/-- A `fn`'s arguments, each bound as the next local. -/
def compileBind : Nat → Program → Nat → Scope → Nat → List Expr → Except String (Code × List STy)
  | 0, _, _, _, _, _ => .error "out of fuel"
  | _ + 1, _, _, _, _, [] => .ok ([], [])
  | fuel + 1, p, depth, sc, n, e :: es => do
    let (c, t) ← compile fuel p depth sc n e
    let (cs, ts) ← compileBind fuel p depth sc (n + 1) es
    .ok (c ++ [.bindLocal] ++ cs, t :: ts)

/-- A template's parts from the first (`first`): each displayed, the
string so far and it concatenated. -/
def compileParts : Nat → Program → Nat → Scope → Nat → Bool → List Expr → Except String Code
  | 0, _, _, _, _, _, _ => .error "out of fuel"
  | _ + 1, _, _, _, _, _, [] => .ok []
  | fuel + 1, p, depth, sc, n, first, e :: es => do
    let (c, t) ← compile fuel p depth sc n e
    let c := if t = .string then c else c ++ [.call "toString" 1]
    let c := if first then c else c ++ [.concat]
    let cs ← compileParts fuel p depth sc n false es
    .ok (c ++ cs)

/-- A record's fields from the `i`th declared: the one written, else the
base's (local `b`, a record of shape `s'`). Whether every field's value is
known to be of its declared type. -/
def compileFields : Nat → Program → Nat → Scope → Nat → List (String × Expr) →
    Option (Nat × String) → String → List Field → Nat → Except String (Code × Bool)
  | 0, _, _, _, _, _, _, _, _, _ => .error "out of fuel"
  | _ + 1, _, _, _, _, _, _, _, [], _ => .ok ([], true)
  | fuel + 1, p, depth, sc, n, written, base, shape, f :: fs, i => do
    let (c, ok) ← match lookupField f.name written with
      | .some e => do
        let (c, t) ← compile fuel p depth sc n e
        pure (c, t.sub (STy.ofTy f.ty))
      | .none =>
        match base with
        | .some (b, s') => pure ([Instr.loadLocal b, .field i], s' == shape)
        | .none => .error s!"no field `{f.name}`"
    let (cs, oks) ← compileFields fuel p depth sc n written base shape fs (i + 1)
    .ok (c ++ cs, ok && oks)

end

/-- The slot a state's or a mutation's name writes. -/
def slotOf (L : Layout) (t : String) : Option Nat :=
  match find? t L.slots with
  | .some i => .some i
  | .none => (find? t L.mutations).map (·.1)

/-- The semantics' effects with names resolved as the plan did. -/
def lowerFx (L : Layout) (fx : Contract.Effects) : Vm.Effects :=
  { writes := fx.writes.map fun (t, v) => ((slotOf L t).getD 0, v)
    rowWrites := fx.rowWrites.map fun (t, v) => ((slotOf L t).getD 0, v)
    commands := fx.commands
    sends := fx.sends.map fun (m, s, vs) => (((find? m L.mutations).map (·.2)).getD 0, s, vs)
    refreshes := fx.refreshes.map fun r => (find? r L.resources).getD 0 }

mutual

/-- Compile a block: each `let` bound as a local for the statements after
it, every one dropped where the block ends. -/
def compileBlock : Nat → Program → Layout → Scope → Nat → List Stmt → Except String Code
  | 0, _, _, _, _, _ => .error "out of fuel"
  | _ + 1, _, _, _, _, [] => .ok []
  | fuel + 1, p, L, sc, n, s :: rest =>
    match s with
    | .letS x e => do
      let (c, t) ← compile fuel p 0 sc n e
      let cr ← compileBlock fuel p L ((x, .local n, t) :: sc) (n + 1) rest
      .ok (c ++ [.bindLocal] ++ cr ++ [.dropLocal])
    | s => do
      let c ← compileStmt fuel p L sc n s
      let cr ← compileBlock fuel p L sc n rest
      .ok (c ++ cr)

def compileStmt : Nat → Program → Layout → Scope → Nat → Stmt → Except String Code
  | 0, _, _, _, _, _ => .error "out of fuel"
  | fuel + 1, p, L, sc, n, s =>
    match s with
    | .letS _ _ => .error "a `let` outside a block"
    | .assign t e => do
      let (c, _) ← compile fuel p 0 sc n e
      match slotOf L t with
      | .some j => .ok (c ++ [.storeSlot j])
      | .none => .error s!"`{t}` is not a state or a mutation"
    | .send t src args => do
      let (c, _) ← compileArgs fuel p 0 sc n args
      match find? t L.mutations with
      | .some (_, k) => .ok (c ++ [.send k src args.length])
      | .none => .error s!"`{t}` is not a mutation"
    | .refresh t =>
      match find? t L.resources with
      | .some r => .ok [.refresh r]
      | .none => .error s!"`{t}` is not a resource"
    | .command name args => do
      let (c, _) ← compileArgs fuel p 0 sc n args
      .ok (c ++ [.command name args.length])
    | .ifS c thn els => do
      let (cc, _) ← compile fuel p 0 sc n c
      let ct ← compileBlock fuel p L sc n thn
      let ce ← compileBlock fuel p L sc n els
      .ok (cc ++ [.jumpIfFalse (ct.length + 1)] ++ ct ++ [.jump ce.length] ++ ce)
    | .matchS subj x sm nn => do
      let (cs, ts) ← compile fuel p 0 sc n subj
      let inner := ts.inner
      let csm ← compileBlock fuel p L ((x, .local n, inner) :: sc) (n + 1) sm
      let cnn ← compileBlock fuel p L sc n nn
      .ok (cs ++ [.jumpIfNone (csm.length + 4), .unwrap, .bindLocal] ++ csm ++
           [.dropLocal, .jump (cnn.length + 1), .pop] ++ cnn)
    -- A call (LLP 1089 D9), expanded as the Rust compiler expands it, with
    -- no call opcode: each argument bound as the next local, at the type
    -- its code has (a `let`'s), then the callee's body in a scope of those
    -- locals and the component's names alone (the caller's locals are
    -- still on the stack, out of reach), every local dropped at its end.
    | .call a args =>
      match p.actions.find? (·.name == a) with
      | .none => .error s!"`{a}` is not an action"
      | .some ad =>
        if args.length ≠ ad.params.length then .error "arity" else do
        let (ca, ts) ← compileBind fuel p 0 sc n args
        let cb ← compileBlock fuel p L (fnScope (ad.params.map (·.1)) n ts ++ globalScope p L)
          (n + args.length) ad.body
        .ok (ca ++ cb ++ List.replicate args.length .dropLocal)

end

/-- Enough fuel for any program the Rust compiler admits. -/
def fuel : Nat := 1 <<< 20

/-- An action's parameters, the last first. -/
def paramScope (ps : List (String × Ty)) : Scope :=
  (ps.zipIdx.map fun ((x, t), i) => (x, Ref.param i, STy.ofTy t)).reverse

/-- A body in the component's scope (a derive, a slot initializer, a
resource argument), ending in `Return`. -/
def compileBody (p : Program) (L : Layout) (e : Expr) : Except String Code := do
  let (c, _) ← compile fuel p 0 (globalScope p L) 0 e
  .ok (c ++ [.ret])

/-- An action's body, ending in `Return`. -/
def compileAction (p : Program) (L : Layout) (a : ActionDecl) : Except String Code := do
  let c ← compileBlock fuel p L (paramScope a.params ++ globalScope p L) 0 a.body
  .ok (c ++ [.ret])

end Contract.Lower
