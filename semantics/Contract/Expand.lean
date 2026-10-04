/-
Component expansion in Lean, mirroring `contract_syntax::inline`
(contract/syntax/src/inline.rs, inline/subst.rs, inline/derives.rs,
inline/calls.rs) step for step, names and numbering included, so that
`difftest expansion` can compare its output with the Rust expander's
structurally and `Contract.ExpandProof` can relate it to the
component-level semantics (`Contract.CompSem`).

  * `Subst`: capture-avoiding substitution (subst.rs): a binder whose name
    a replacement mentions is renamed `x@k`.
  * `renameNodes`: a child's view binders renamed `x#n` (inline.rs).
  * `resolvedDerives`: a child's derives resolved through one another,
    a dependency read twice on every path bound once by a `let` (derives.rs).
  * `inlineNodes`: uses inlined, props substituted, injects filled from
    the nearest `provide`, fills put at `children`, a child's states and
    actions lifted into the root as `name#n` with their owners, props
    captured as hidden action parameters `@capture:n:i` (inline.rs).
  * `ownCalls`, `callMarked`, `resolveCalls`, `hygiene`: action calls
    (calls.rs, LLP 1089). A call stays a `call`, as `contract lean` emits
    it; its caller's parameters and binders are renamed as Rust's hygiene
    renames them.
-/
import Contract.Components

namespace Contract.Expand

open Contract Components

/-! ## Maps -/

/-- A map by name, as Rust's `BTreeMap` is used here: insertion replaces. -/
abbrev SMap := List (String × Expr)

def SMap.get? (m : SMap) (k : String) : Option Expr := (m.find? (·.1 == k)).map (·.2)

def SMap.insert (m : SMap) (k : String) (v : Expr) : SMap :=
  if m.any (·.1 == k) then m.map fun (a, b) => if a == k then (a, v) else (a, b)
  else m ++ [(k, v)]

def SMap.erase (m : SMap) (k : String) : SMap := m.filter (·.1 != k)

/-! ## Names in expressions -/

mutual
/-- The names free in an expression, a call's head and a record's shape
included (subst.rs `free_names`). -/
def freeNames (bound : List String) : Expr → List String
  | .var n => if bound.contains n then [] else [n]
  | .call n args => (if bound.contains n then [] else [n]) ++ freeNamesList bound args
  | .record s base fields =>
    (if bound.contains s then [] else [s]) ++
    (match base with | .some b => freeNames bound b | .none => []) ++ freeNamesFields bound fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => freeNames bound o
  | .binary _ a b => freeNames bound a ++ freeNames bound b
  | .ternary a b c => freeNames bound a ++ freeNames bound b ++ freeNames bound c
  | .matchOpt s x a b => freeNames bound s ++ freeNames bound b ++ freeNames (x :: bound) a
  | .letE x v b => freeNames bound v ++ freeNames (x :: bound) b
  | .arrow ps b => freeNames (ps ++ bound) b
  | .template parts => freeNamesList bound parts
  | .list items => freeNamesList bound items
  | .num _ | .str _ | .bool _ | .none => []
def freeNamesList (bound : List String) : List Expr → List String
  | [] => []
  | e :: es => freeNames bound e ++ freeNamesList bound es
def freeNamesFields (bound : List String) : List (String × Expr) → List String
  | [] => []
  | (_, e) :: es => freeNames bound e ++ freeNamesFields bound es
end

mutual
/-- Every name written in `e`, free or bound: a reference, a call's head,
a record's shape, a binder (subst.rs `occurs` is membership here). -/
def allNames : Expr → List String
  | .var n => [n]
  | .call n args => n :: allNamesList args
  | .record s base fields =>
    s :: ((match base with | .some b => allNames b | .none => []) ++ allNamesFields fields)
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => allNames o
  | .binary _ a b => allNames a ++ allNames b
  | .ternary a b c => allNames a ++ allNames b ++ allNames c
  | .matchOpt s x a b => x :: (allNames s ++ allNames a ++ allNames b)
  | .letE x v b => x :: (allNames v ++ allNames b)
  | .arrow ps b => ps ++ allNames b
  | .template parts => allNamesList parts
  | .list items => allNamesList items
  | .num _ | .str _ | .bool _ | .none => []
def allNamesList : List Expr → List String
  | [] => []
  | e :: es => allNames e ++ allNamesList es
def allNamesFields : List (String × Expr) → List String
  | [] => []
  | (_, e) :: es => allNames e ++ allNamesFields es
end

mutual
/-- Every name a statement writes (subst.rs `stmt_occurs`): an assignment's
target, a binder, the names in its expressions (not a command's name). -/
def stmtAllNames : Stmt → List String
  | .assign t e => t :: allNames e
  | .letS n e => n :: allNames e
  | .command _ args | .send _ _ args | .call _ args => allNamesList args
  | .refresh _ => []
  | .ifS c a b => allNames c ++ stmtsAllNames a ++ stmtsAllNames b
  | .matchS s x a b => x :: (allNames s ++ stmtsAllNames a ++ stmtsAllNames b)
def stmtsAllNames : List Stmt → List String
  | [] => []
  | s :: ss => stmtAllNames s ++ stmtsAllNames ss
end

/-! ## Substitution (subst.rs) -/

inductive Repl where
  | name (s : String)
  | expr (e : Expr)

def replOf : Expr → Repl
  | .var n => .name n
  | e => .expr e

/-- Replacements by name, applied under the binders in force (innermost
first). A binder hides its name's replacement; a renamed one replaces its
name by the new spelling. -/
structure Subst where
  map : SMap
  binders : List (String × Option String) := []
  /-- Whether a call's head is replaced too. -/
  calls : Bool := true
  /-- Record constructors: a call naming one keeps its head. -/
  records : List String := []

namespace Subst

def get (s : Subst) (n : String) : Option Repl :=
  match s.binders.find? (·.1 == n) with
  | .some (_, spelled) => spelled.map .name
  | .none => (s.map.get? n).map replOf

def free (s : Subst) : List String := s.map.foldl (fun acc (_, v) => acc ++ freeNames [] v) []

def introduced (s : Subst) (n : String) : Bool := s.binders.any (·.2 == Option.some n)

/-- A name longer than every name in `taken`: none of them. -/
def beyond (var : String) (taken : List String) : String :=
  var ++ "@" ++ String.ofList (List.replicate (taken.foldl (fun m t => max m t.length) 0 + 1) '_')

/-- The first `var@k` (k ≥ 1) not in `taken`; `fuel` bounds the search
(enough fuel never runs out: at most `taken.length` are taken). -/
def fresh (var : String) (taken : List String) : Nat → Nat → String
  | 0, _ => beyond var taken
  | fuel + 1, k =>
    let c := s!"{var}@{k}"
    if taken.contains c then fresh var taken fuel (k + 1) else c

/-- Enter a binder `var` whose scope writes the names `names`: its spelling
and the substitution under it. Renamed whenever a replacement mentions
`var` (or an enclosing binder was renamed to it), to the first `var@k`
neither the scope, a replacement nor an enclosing binder spells. -/
def enter (s : Subst) (var : String) (names : List String) : String × Subst :=
  let fr := s.free
  let spellings := s.binders.filterMap (·.2)
  let spelled :=
    if fr.contains var || s.introduced var then
      let taken := names ++ fr ++ spellings
      fresh var taken (taken.length + 1) 1
    else var
  (spelled, { s with binders := (var, if spelled != var then Option.some spelled else Option.none) :: s.binders })

end Subst

/-- Enter the parameters of an arrow one by one. -/
def enterParams (body : Expr) (all : List String) : Subst → List String → List String × Subst
  | s, [] => ([], s)
  | s, p :: ps =>
    let (sp, s) := s.enter p (allNames body ++ all)
    let (rest, s) := enterParams body all s ps
    (sp :: rest, s)

mutual
/-- Substitute (subst.rs `subst_expr`). A call whose head is replaced by
a call `f(a…)` becomes `f(a…, args)`. -/
def substExpr (s : Subst) : Expr → Expr
  | e@(.var n) =>
    match s.get n with
    | .some (.name m) => .var m
    | .some (.expr r) => r
    | .none => e
  | .call n args =>
    let args := substList s args
    if !s.calls || s.records.contains n then .call n args else
    match s.get n with
    | .some (.name f) => .call f args
    | .some (.expr (.call f first)) => .call f (first ++ args)
    | _ => .call n args
  | .record sh base fields =>
    .record sh (match base with | .some b => .some (substExpr s b) | .none => .none)
      (substFields s fields)
  | .member o f => .member (substExpr s o) f
  | .typed o t => .typed (substExpr s o) t
  | .named n o => .named n (substExpr s o)
  | .some o => .some (substExpr s o)
  | .unary op o => .unary op (substExpr s o)
  | .binary op a b => .binary op (substExpr s a) (substExpr s b)
  | .ternary a b c => .ternary (substExpr s a) (substExpr s b) (substExpr s c)
  | .matchOpt subj x a b =>
    let (x', s') := s.enter x (allNames a)
    .matchOpt (substExpr s subj) x' (substExpr s' a) (substExpr s b)
  | .letE x v b =>
    let (x', s') := s.enter x (allNames b)
    .letE x' (substExpr s v) (substExpr s' b)
  | .arrow ps b =>
    let (ps', s') := enterParams b ps s ps
    .arrow ps' (substExpr s' b)
  | .template parts => .template (substList s parts)
  | .list items => .list (substList s items)
  | e@(.num _) | e@(.str _) | e@(.bool _) | e@.none => e
def substList (s : Subst) : List Expr → List Expr
  | [] => []
  | e :: es => substExpr s e :: substList s es
def substFields (s : Subst) : List (String × Expr) → List (String × Expr)
  | [] => []
  | (n, e) :: es => (n, substExpr s e) :: substFields s es
end

/-- `e` with `map` substituted, calls included (subst.rs `substituted`). -/
def substituted (records : List String) (map : SMap) (e : Expr) : Expr :=
  if map.isEmpty then e else substExpr { map, records } e

/-- Renamed locals: every reference, never a call's head (subst.rs
`renamed_locals`). -/
def renamedLocals (map : SMap) (e : Expr) : Expr :=
  if map.isEmpty then e else substExpr { map, calls := false } e

mutual
/-- A block's names substituted (subst.rs `subst_stmts`): assignment
targets renamed by `names`, expressions through `s`; a `let` binds for the
rest of its block and is renamed apart when a replacement mentions it. -/
def substStmts (s : Subst) (names : List (String × String)) : List Stmt → List Stmt
  | [] => []
  | st :: rest =>
    match st with
    | .letS x e =>
      let e := substExpr s e
      let (x', s') := s.enter x (stmtsAllNames rest)
      .letS x' e :: substStmts s' names rest
    | .assign t e =>
      .assign ((names.find? (·.1 == t)).map (·.2) |>.getD t) (substExpr s e) :: substStmts s names rest
    | .command n args => .command n (substList s args) :: substStmts s names rest
    -- A call's callee by its lifted name, its arguments in the caller's
    -- scope (subst.rs: the body is the callee's, which `contract lean`
    -- does not emit).
    | .call a args =>
      .call ((names.find? (·.1 == a)).map (·.2) |>.getD a) (substList s args) :: substStmts s names rest
    | .send t src args => .send t src (substList s args) :: substStmts s names rest
    | .refresh t => .refresh t :: substStmts s names rest
    | .ifS c a b => .ifS (substExpr s c) (substStmts s names a) (substStmts s names b) :: substStmts s names rest
    | .matchS subj x a b =>
      let (x', s') := s.enter x (stmtsAllNames a)
      .matchS (substExpr s subj) x' (substStmts s' names a) (substStmts s names b) ::
        substStmts s names rest
end

/-! ## Expansion state -/

/-- What owns a lifted state (inline.rs `Owner`). -/
inductive Owner where
  | root
  | instance
  | arm (tag arm : Nat)
  deriving Inhabited, BEq

structure Ctx where
  counter : Nat := 0
  nextTag : Nat := 1
  depth : Nat := 0
  /-- Provided bindings in force, outermost first. -/
  provides : List (String × Expr) := []
  /-- The fill in force: inside a `slot` component's view. -/
  fill : Option (List Node) := .none
  /-- The region arms around the site, innermost first. -/
  arms : List Owner := []
  extraStates : List StateDecl := []
  extraActions : List ActionDecl := []
  deriving Inhabited

abbrev ExM := StateT Ctx (Except String)

def refuse {α} (why : String) : ExM α := throw why

/-! ## Derives (derives.rs) -/

/-- A callback's parameters renamed `p@bk`, counting on. -/
def freshenParams : List String → StateM Nat (SMap × List String)
  | [] => pure ([], [])
  | p :: ps => do
    modify (· + 1)
    let k ← get
    let name := s!"{p}@b{k}"
    let (m, ns) ← freshenParams ps
    pure ((p, .var name) :: m, name :: ns)

mutual
/-- Every `match` binder and callback parameter renamed `x@bk`, each
distinct (derives.rs `freshen`). -/
def freshen (records : List String) : Expr → StateM Nat Expr
  | .matchOpt subj x a b => do
    modify (· + 1)
    let k ← get
    let name := s!"{x}@b{k}"
    let a ← freshen records a
    let a := substituted records [(x, .var name)] a
    let subj ← freshen records subj
    let b ← freshen records b
    pure (.matchOpt subj name a b)
  | .arrow ps body => do
    let (renamed, names) ← freshenParams ps
    let body ← freshen records body
    pure (.arrow names (substituted records renamed body))
  | .some o => return .some (← freshen records o)
  | .unary op o => return .unary op (← freshen records o)
  | .member o f => return .member (← freshen records o) f
  | .named n o => return .named n (← freshen records o)
  | .typed o t => return .typed (← freshen records o) t
  | .binary op a b => do
    let a ← freshen records a
    let b ← freshen records b
    pure (.binary op a b)
  | .ternary a b c => do
    let a ← freshen records a
    let b ← freshen records b
    let c ← freshen records c
    pure (.ternary a b c)
  | .letE x v b => do
    let v ← freshen records v
    let b ← freshen records b
    pure (.letE x v b)
  | .call n args => return .call n (← freshenList records args)
  | .record s base fields => do
    let base ← match base with
      | .some b => do pure (Option.some (← freshen records b))
      | .none => pure Option.none
    let fields ← freshenFields records fields
    pure (.record s base fields)
  | .template parts => return .template (← freshenList records parts)
  | .list items => return .list (← freshenList records items)
  | e@(.num _) | e@(.str _) | e@(.bool _) | e@.none | e@(.var _) => pure e
def freshenList (records : List String) : List Expr → StateM Nat (List Expr)
  | [] => pure []
  | e :: es => do
    let e ← freshen records e
    let es ← freshenList records es
    pure (e :: es)
def freshenFields (records : List String) : List (String × Expr) → StateM Nat (List (String × Expr))
  | [] => pure []
  | (n, e) :: es => do
    let e ← freshen records e
    let es ← freshenFields records es
    pure ((n, e) :: es)
end

mutual
/-- The derives `e` reads directly: by name or by a call's head. -/
def dependencies (idx : String → Option Nat) : Expr → List Nat
  | .var n => (idx n).toList
  | .call n args => (idx n).toList ++ dependenciesList idx args
  | .record s base fields =>
    (idx s).toList ++ (match base with | .some b => dependencies idx b | .none => []) ++
      dependenciesFields idx fields
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => dependencies idx o
  | .binary _ a b => dependencies idx a ++ dependencies idx b
  | .ternary a b c => dependencies idx a ++ dependencies idx b ++ dependencies idx c
  | .matchOpt s _ a b => dependencies idx s ++ dependencies idx a ++ dependencies idx b
  | .letE _ v b => dependencies idx v ++ dependencies idx b
  | .arrow _ b => dependencies idx b
  | .template parts => dependenciesList idx parts
  | .list items => dependenciesList idx items
  | .num _ | .str _ | .bool _ | .none => []
def dependenciesList (idx : String → Option Nat) : List Expr → List Nat
  | [] => []
  | e :: es => dependencies idx e ++ dependenciesList idx es
def dependenciesFields (idx : String → Option Nat) : List (String × Expr) → List Nat
  | [] => []
  | (_, e) :: es => dependencies idx e ++ dependenciesFields idx es
end

/-- How often each derive is read (0, 1, or 2 for more) and whether on
every path. -/
structure Summary where
  reads : List Nat
  always : List Bool
  deriving Inhabited

def Summary.new (n : Nat) : Summary := { reads := List.replicate n 0, always := List.replicate n false }

def Summary.addSometimes (a b : Summary) : Summary :=
  { a with reads := (a.reads.zip b.reads).map fun (x, y) => min (x + y) 2 }

def Summary.add (a b : Summary) : Summary :=
  let s := a.addSometimes b
  { s with always := (a.always.zip b.always).map fun (x, y) => x || y }

def Summary.branches (out a b : Summary) : Summary :=
  let s := (out.addSometimes a).addSometimes b
  { s with always := ((s.always.zip a.always).zip b.always).map fun ((o, x), y) => o || (x && y) }

mutual
/-- Summarize `e` (derives.rs `summarize`). -/
def summarize (n : Nat) (idx : String → Option Nat) (sums : List Summary) : Expr → Summary
  | .var name =>
    match idx name with
    | .some d =>
      let s := (Summary.new n).add (sums.getD d (Summary.new n))
      { reads := s.reads.set d (min (s.reads.getD d 0 + 1) 2), always := s.always.set d true }
    | .none => Summary.new n
  | .binary .and a b | .binary .or a b =>
    ((Summary.new n).add (summarize n idx sums a)).addSometimes (summarize n idx sums b)
  | .ternary c a b =>
    ((Summary.new n).add (summarize n idx sums c)).branches (summarize n idx sums a)
      (summarize n idx sums b)
  | .matchOpt s _ a b =>
    ((Summary.new n).add (summarize n idx sums s)).branches (summarize n idx sums a)
      (summarize n idx sums b)
  | .arrow _ b => (Summary.new n).addSometimes (summarize n idx sums b)
  | .some o | .unary _ o | .member o _ | .named _ o | .typed o _ =>
    (Summary.new n).add (summarize n idx sums o)
  | .binary _ a b => ((Summary.new n).add (summarize n idx sums a)).add (summarize n idx sums b)
  | .letE _ v b => ((Summary.new n).add (summarize n idx sums v)).add (summarize n idx sums b)
  | .call _ args => summarizeList n idx sums (Summary.new n) args
  | .record _ base fields =>
    let acc := match base with
      | .some b => (Summary.new n).add (summarize n idx sums b)
      | .none => Summary.new n
    summarizeFields n idx sums acc fields
  | .template parts => summarizeList n idx sums (Summary.new n) parts
  | .list items => summarizeList n idx sums (Summary.new n) items
  | .num _ | .str _ | .bool _ | .none => Summary.new n
def summarizeList (n : Nat) (idx : String → Option Nat) (sums : List Summary) (acc : Summary) :
    List Expr → Summary
  | [] => acc
  | e :: es => summarizeList n idx sums (acc.add (summarize n idx sums e)) es
def summarizeFields (n : Nat) (idx : String → Option Nat) (sums : List Summary) (acc : Summary) :
    List (String × Expr) → Summary
  | [] => acc
  | (_, e) :: es => summarizeFields n idx sums (acc.add (summarize n idx sums e)) es
end

/-- What `place` reads of the component's derives. -/
structure DCx where
  n : Nat
  idx : String → Option Nat
  names : List String
  bodies : List Expr
  sums : List Summary
  rank : List Nat

/-- `e` with every derive it reads resolved (derives.rs `Cx::place`);
`bound` are the derives an enclosing `let` binds; `whole` where a derive
may first qualify to be bound. `fuel` bounds the recursion (the expression
and the derive bodies placed within it), never reached. -/
def place (cx : DCx) : Nat → List Nat → Bool → Expr → Expr
  | 0, _, _, e => e
  | fuel + 1, bound, whole, e =>
    let (lets, bound') :=
      if whole then
        let s := summarize cx.n cx.idx cx.sums e
        let shared := ((List.range cx.n).filter fun d =>
          s.reads.getD d 0 ≥ 2 && s.always.getD d false && !bound.contains d).mergeSort
            fun a b => cx.rank.getD a 0 ≤ cx.rank.getD b 0
        shared.foldl (fun (lets, bnd) d =>
          (lets ++ [(d, place cx fuel bnd true (cx.bodies.getD d .none))], bnd ++ [d])) ([], bound)
      else ([], bound)
    let body := match e with
      | .var name =>
        match cx.idx name with
        | .some d => if bound'.contains d then e else place cx fuel bound' true (cx.bodies.getD d .none)
        | .none => e
      | .binary .and a b => .binary .and (place cx fuel bound' false a) (place cx fuel bound' true b)
      | .binary .or a b => .binary .or (place cx fuel bound' false a) (place cx fuel bound' true b)
      | .ternary c a b =>
        .ternary (place cx fuel bound' false c) (place cx fuel bound' true a) (place cx fuel bound' true b)
      | .matchOpt s x a b =>
        .matchOpt (place cx fuel bound' false s) x (place cx fuel bound' true a) (place cx fuel bound' true b)
      | .arrow ps b => .arrow ps (place cx fuel bound' true b)
      | .some o => .some (place cx fuel bound' false o)
      | .unary op o => .unary op (place cx fuel bound' false o)
      | .member o f => .member (place cx fuel bound' false o) f
      | .named n o => .named n (place cx fuel bound' false o)
      | .typed o t => .typed (place cx fuel bound' false o) t
      | .binary op a b => .binary op (place cx fuel bound' false a) (place cx fuel bound' false b)
      | .letE x v b => .letE x (place cx fuel bound' false v) (place cx fuel bound' false b)
      | .call f args => .call f (args.map (place cx fuel bound' false))
      | .record s base fields =>
        .record s (base.map (place cx fuel bound' false)) (fields.map fun (n, x) => (n, place cx fuel bound' false x))
      | .template parts => .template (parts.map (place cx fuel bound' false))
      | .list items => .list (items.map (place cx fuel bound' false))
      | e => e
    lets.foldr (fun (d, v) body => .letE (cx.names.getD d "") v body) body

/-! ### Names a component reads outside its derives -/

mutual
def exprNames : Expr → List String
  | .var n => [n]
  | .call n args => n :: exprNamesList args
  | .record s base fields =>
    s :: ((match base with | .some b => exprNames b | .none => []) ++ exprNamesFields fields)
  | .member o _ | .named _ o | .typed o _ | .some o | .unary _ o => exprNames o
  | .binary _ a b => exprNames a ++ exprNames b
  | .ternary a b c => exprNames a ++ exprNames b ++ exprNames c
  | .matchOpt s _ a b => exprNames s ++ exprNames a ++ exprNames b
  | .letE _ v b => exprNames v ++ exprNames b
  | .arrow _ b => exprNames b
  | .template parts => exprNamesList parts
  | .list items => exprNamesList items
  | .num _ | .str _ | .bool _ | .none => []
def exprNamesList : List Expr → List String
  | [] => []
  | e :: es => exprNames e ++ exprNamesList es
def exprNamesFields : List (String × Expr) → List String
  | [] => []
  | (_, e) :: es => exprNames e ++ exprNamesFields es
end

mutual
def stmtNames : Stmt → List String
  | .assign _ e | .letS _ e => exprNames e
  | .command _ args | .send _ _ args | .call _ args => exprNamesList args
  | .refresh _ => []
  | .ifS c a b => exprNames c ++ stmtsNames a ++ stmtsNames b
  | .matchS s _ a b => exprNames s ++ stmtsNames a ++ stmtsNames b
def stmtsNames : List Stmt → List String
  | [] => []
  | s :: ss => stmtNames s ++ stmtsNames ss
end

mutual
def nodeNames : CNode → List String
  | .element _ pos props hs kids =>
    exprNamesList pos ++ exprNamesFields props ++ handlerNames hs ++ nodesNames kids
  | .use _ _ args kids => exprNamesFields args ++ nodesNames kids
  | .when _ c a b => exprNames c ++ nodesNames a ++ nodesNames b
  | .each _ _ _ l k body => exprNames l ++ exprNames k ++ nodesNames body
  | .matchN _ s _ a b => exprNames s ++ nodesNames a ++ nodesNames b
  | .children _ => []
def nodesNames : List CNode → List String
  | [] => []
  | n :: ns => nodeNames n ++ nodesNames ns
def handlerNames : List (String × String × List Expr) → List String
  | [] => []
  | (_, a, args) :: hs => a :: exprNamesList args ++ handlerNames hs
end

/-- Every name the component's view, states, provides and actions write. -/
def readOutsideDerives (c : CComponent) : List String :=
  nodesNames c.view ++ (c.states.flatMap fun s => exprNames s.init) ++
    (c.provides.flatMap fun (_, e) => exprNames e) ++ (c.actions.flatMap fun a => stmtsNames a.body)

/-- Dependency order, dependencies first; a cycle is refused. -/
def visitOrder (reads : List (List Nat)) (names : List String) : Nat → Nat → List Nat × List Nat →
    Except String (List Nat × List Nat)
  | 0, _, acc => .ok acc
  | fuel + 1, i, (state, order) =>
    match state.getD i 0 with
    | 2 => .ok (state, order)
    | 1 => .error s!"cannot resolve `{names.getD i ""}`: it depends on itself through other derives"
    | _ => do
      let state := state.set i 1
      let deps := ((reads.getD i []).eraseDups).mergeSort (· ≤ ·)
      let (state, order) ← deps.foldlM (fun acc d => visitOrder reads names fuel d acc) (state, order)
      pure (state.set i 2, order ++ [i])

/-- The derives of `c` that its view, states, provides or actions read,
each resolved (derives.rs `resolved_derives`). -/
def resolvedDerives (records : List String) (c : CComponent) : Except String (List (String × Expr)) := do
  let names := c.derives.map (·.name)
  if names.eraseDups.length != names.length then throw "a derive declared twice"
  if c.derives.isEmpty then return []
  let n := c.derives.length
  let idx : String → Option Nat := fun x => names.idxOf? x
  let (bodies, _) := (c.derives.foldlM (fun acc d => do
      let b ← freshen records d.body
      pure (acc ++ [b])) [] : StateM Nat (List Expr)).run 0
  let reads := bodies.map (dependencies idx)
  let (_, order) ← (List.range n).foldlM (fun acc i => visitOrder reads names (n + 1) i acc)
    (List.replicate n 0, [])
  let rank := (List.range n).map fun i => order.idxOf i
  let sums := order.foldl (fun sums i => sums.set i (summarize n idx sums (bodies.getD i .none)))
    (List.replicate n (Summary.new n))
  let cx : DCx := { n, idx, names, bodies, sums, rank }
  let read := readOutsideDerives c
  pure ((c.derives.zipIdx.filter fun (d, _) => read.contains d.name).map fun (d, i) =>
    (d.name, place cx (1 <<< 20) [] true (bodies.getD i .none)))

/-! ## Calls (calls.rs, and inline.rs `with_captures`, `call_marked`)

`contract lean` emits a call by its callee and whole argument list, never
the body Rust expanded (LLP 1089 D9), so only what decides those is
transcribed here: which statements are calls, the callee's lifted name,
the arguments, and the names hygiene draws. -/

/-- A prop or inject call between lifting and `resolveCalls`. -/
def callMark : String := "@call:"

/-- Whether a statement of `body` is `name(…)`. -/
def commandsIn (name : String) : List Stmt → Bool
  | [] => false
  | .command n _ :: rest => n == name || commandsIn name rest
  | .ifS _ a b :: rest | .matchS _ _ a b :: rest => commandsIn name a || commandsIn name b || commandsIn name rest
  | _ :: rest => commandsIn name rest

/-- A lifted body whose same-component calls pass the instance's captures
first (inline.rs `with_captures`). -/
def withCaptures (captures : List String) : List Stmt → List Stmt
  | [] => []
  | .call a args :: rest => .call a (captures.map .var ++ args) :: withCaptures captures rest
  | .ifS c a b :: rest => .ifS c (withCaptures captures a) (withCaptures captures b) :: withCaptures captures rest
  | .matchS s x a b :: rest =>
    .matchS s x (withCaptures captures a) (withCaptures captures b) :: withCaptures captures rest
  | st :: rest => st :: withCaptures captures rest

/-- A lifted body whose calls of an action prop or injected action, at
every position, are marked with the action it named, its curried
arguments first (inline.rs `call_marked`). -/
def callMarked (called : List (String × String × List Expr)) : List Stmt → List Stmt
  | [] => []
  | .command n args :: rest =>
    (match called.find? (·.1 == n) with
      | .some (_, target, held) => .call (callMark ++ target) (held ++ args)
      | .none => .command n args) :: callMarked called rest
  | .ifS c a b :: rest => .ifS c (callMarked called a) (callMarked called b) :: callMarked called rest
  | .matchS s x a b :: rest => .matchS s x (callMarked called a) (callMarked called b) :: callMarked called rest
  | st :: rest => st :: callMarked called rest

/-- Every marked call pointed at the action it names (calls.rs `resolve`). -/
def resolveCalls (actions : List ActionDecl) : List Stmt → Except String (List Stmt)
  | [] => pure []
  | .call a args :: rest => do
    let a := if a.startsWith callMark then (a.drop callMark.length).toString else a
    if !actions.any (·.name == a) then throw s!"`{a}` is not an action"
    pure (.call a args :: (← resolveCalls actions rest))
  | .ifS c x y :: rest => do
    pure (.ifS c (← resolveCalls actions x) (← resolveCalls actions y) :: (← resolveCalls actions rest))
  | .matchS s v x y :: rest => do
    pure (.matchS s v (← resolveCalls actions x) (← resolveCalls actions y) :: (← resolveCalls actions rest))
  | st :: rest => do pure (st :: (← resolveCalls actions rest))

def hasCall : List Stmt → Bool
  | [] => false
  | .call .. :: _ => true
  | .ifS _ a b :: rest | .matchS _ _ a b :: rest => hasCall a || hasCall b || hasCall rest
  | _ :: rest => hasCall rest

abbrev TailM := StateT Nat (Except String)

def values (records : List String) (map : SMap) : Subst := { map, calls := false, records }

/-- Every `let` and `match` binding renamed `x@bk`, its reads with it; a
call's arguments are renamed with the rest (calls.rs `apart`). `fuel`
bounds the walk (a block's statements, nested ones included). -/
def apart (records : List String) : Nat → List Stmt → TailM (List Stmt)
  | 0, _ => throw "apart: out of fuel"
  | _ + 1, [] => pure []
  | fuel + 1, .letS x e :: rest => do
    modify (· + 1)
    let k ← get
    let renamed := s!"{x}@b{k}"
    let rest := substStmts (values records [(x, .var renamed)]) [] rest
    pure (.letS renamed e :: (← apart records fuel rest))
  | fuel + 1, .ifS c a b :: rest => do
    let a ← apart records fuel a
    let b ← apart records fuel b
    pure (.ifS c a b :: (← apart records fuel rest))
  | fuel + 1, .matchS s x a b :: rest => do
    modify (· + 1)
    let k ← get
    let renamed := s!"{x}@b{k}"
    let a := substStmts (values records [(x, .var renamed)]) [] a
    let a ← apart records fuel a
    let b ← apart records fuel b
    pure (.matchS s renamed a b :: (← apart records fuel rest))
  | fuel + 1, st :: rest => do pure (st :: (← apart records fuel rest))

mutual
def stmtSize : Stmt → Nat
  | .ifS _ a b | .matchS _ _ a b => 1 + stmtsSize a + stmtsSize b
  | _ => 1
def stmtsSize : List Stmt → Nat
  | [] => 1
  | s :: ss => stmtSize s + stmtsSize ss
end

/-- The `let`s and `match` bindings of a block, its arms' included, a
call's not (calls.rs `drawn`'s `binders`). -/
def binders : List Stmt → Nat
  | [] => 0
  | .letS .. :: rest => 1 + binders rest
  | .ifS _ a b :: rest => binders a + binders b + binders rest
  | .matchS _ _ a b :: rest => 1 + binders a + binders b + binders rest
  | _ :: rest => binders rest

/-- The names LLP 1017 §11's resolver drew for the calls in a block
(calls.rs `drawn`): one for each call's parameters, one for each binder of
the callee's statements, and its own calls' in turn. A call that does not
expand (its arity refused) drew one. -/
def drawn (actions : List ActionDecl) : Nat → List Stmt → Nat
  | 0, _ => 0
  | _ + 1, [] => 0
  | fuel + 1, .ifS _ a b :: rest | fuel + 1, .matchS _ _ a b :: rest =>
    drawn actions fuel a + drawn actions fuel b + drawn actions fuel rest
  | fuel + 1, .call a args :: rest =>
    let here := match actions.find? (·.name == a) with
      | .some callee =>
        if callee.params.length == args.length then 1 + binders callee.body + drawn actions fuel callee.body else 1
      | .none => 1
    here + drawn actions fuel rest
  | fuel + 1, _ :: rest => drawn actions fuel rest

/-- Rename every caller's own parameters `p@ck` and binders `x@bk` apart
(calls.rs `hygiene`), `k` drawn as the tail call drew it. -/
def hygiene (records : List String) (actions : List ActionDecl) : TailM (List ActionDecl) := do
  let fuel := actions.foldl (fun n a => n + stmtsSize a.body) 1
  actions.mapM fun a => do
    if !hasCall a.body then return a
    modify (· + 1)
    let k ← get
    let ren := fun (p : String) => if p.contains '@' then p else s!"{p}@c{k}"
    let map : SMap := (a.params.filter fun (p, _) => !p.contains '@').map fun (p, _) => (p, .var (ren p))
    let params := a.params.map fun (p, t) => (ren p, t)
    let body := substStmts (values records map) [] a.body
    let body ← apart records (stmtsSize body) body
    modify (· + drawn actions fuel body)
    pure { a with params, body }

/-! ## Inlining (inline.rs) -/

/-- Rename every name a child's view binds with the use's number
(inline.rs `rename_nodes`): `each` and `match` binders become `x#n`. -/
def renameNodes (n : Nat) : SMap → List CNode → List CNode
  | _, [] => []
  | map, nd :: rest =>
    let here := match nd with
      | .element tag pos props hs kids =>
        .element tag (pos.map (renamedLocals map)) (props.map fun (a, e) => (a, renamedLocals map e))
          (hs.map fun (ev, a, args) =>
            -- A handler without arguments is a name (an `Ident`), renamed as one.
            let a := if args.isEmpty then
                match renamedLocals map (.var a) with
                | .var b => b
                | _ => a
              else a
            (ev, a, args.map (renamedLocals map)))
          (renameNodes n map kids)
      | .use id name args kids =>
        .use id name (args.map fun (a, e) => (a, renamedLocals map e)) (renameNodes n map kids)
      | .children id => .children id
      | .when id c a b => .when id (renamedLocals map c) (renameNodes n map a) (renameNodes n map b)
      | .each id x ix l k body =>
        let inner := map.insert x (.var s!"{x}#{n}")
        let inner := match ix with
          | .some i => inner.insert i (.var s!"{i}#{n}")
          | .none => inner
        .each id s!"{x}#{n}" (ix.map fun i => s!"{i}#{n}") (renamedLocals map l)
          (renamedLocals inner k) (renameNodes n inner body)
      | .matchN id subj x a b =>
        let inner := map.insert x (.var s!"{x}#{n}")
        .matchN id (renamedLocals map subj) s!"{x}#{n}" (renameNodes n inner a) (renameNodes n map b)
    here :: renameNodes n map rest

mutual
/-- Whether `e` holds a `none` or `[]` outside a `typed`: a leaf whose
element type only a declaration can say. -/
def untypedLeaf : Expr → Bool
  | .none => true
  | .list items => items.isEmpty || untypedLeafList items
  | .typed .. | .num _ | .str _ | .bool _ | .var _ | .arrow .. => false
  | .template parts => untypedLeafList parts
  | .some x | .member x _ | .named _ x | .unary _ x => untypedLeaf x
  | .call _ args => untypedLeafList args
  | .record _ base fields =>
    (match base with | .some b => untypedLeaf b | .none => false) || untypedLeafFields fields
  | .binary _ a b => untypedLeaf a || untypedLeaf b
  | .ternary a b c => untypedLeaf a || untypedLeaf b || untypedLeaf c
  | .matchOpt s _ a b => untypedLeaf s || untypedLeaf a || untypedLeaf b
  | .letE _ v b => untypedLeaf v || untypedLeaf b
def untypedLeafList : List Expr → Bool
  | [] => false
  | e :: es => untypedLeaf e || untypedLeafList es
def untypedLeafFields : List (String × Expr) → Bool
  | [] => false
  | (_, e) :: es => untypedLeaf e || untypedLeafFields es
end

/-- A handler's action and arguments after substitution, as Rust
substitutes the attribute's value (an `Ident` without arguments, a `Call`
with). -/
def substHandler (s : Subst) (a : String) (args : List Expr) : ExM (String × List Expr) :=
  if args.isEmpty then
    match s.get a with
    | .some (.name f) => pure (f, [])
    | .some (.expr (.call f first)) => pure (f, first)
    | .some (.expr _) => refuse s!"a handler names an action (`{a}`)"
    | .none => pure (a, [])
  else
    let args := substList s args
    if !s.calls || s.records.contains a then pure (a, args) else
    match s.get a with
    | .some (.name f) => pure (f, args)
    | .some (.expr (.call f first)) => pure (f, first ++ args)
    | _ => pure (a, args)

def lifted (x : String) (n : Nat) : String := s!"{x}#{n}"

/-- Insert each pair in order: a later one replaces an earlier one. -/
def insertAll (m : SMap) : List (String × Expr) → SMap
  | [] => m
  | (k, v) :: kvs => insertAll (m.insert k v) kvs

/-- A use's props, each argument substituted in the use site's scope (a
`none` or `[]` in it typed by the prop's declaration). -/
def propArgs (s : Subst) (c : CComponent) (args : List (String × Expr)) :
    Except String (List (String × Expr)) :=
  c.props.mapM fun pd =>
    match (args.find? (·.1 == pd.name)).map (·.2) with
    | .some a =>
      let v := substExpr s a
      .ok (pd.name, if pd.declared && untypedLeaf v then .typed v pd.ty else v)
    | .none => .error s!"`{c.name}` needs `{pd.name}`"

/-- A use's injects, each the nearest provider's binding. -/
def injectArgs (provides : List (String × Expr)) (c : CComponent) :
    Except String (List (String × Expr)) :=
  c.injects.mapM fun pd =>
    match provides.reverse.find? (·.1 == pd.name) with
    | .some (_, e) => .ok (pd.name, e)
    | .none => .error s!"`{c.name}` injects `{pd.name}`, and no component above this use provides it"

/-- The first part of a use's substitution (inline.rs `child_subst`): its
props, then its injects, then its states as their lifted names `x#n`. -/
def baseMap (s : Subst) (c : CComponent) (args provides : List (String × Expr)) (n : Nat) :
    Except String SMap := do
  let pa ← propArgs s c args
  let ia ← injectArgs provides c
  pure (insertAll (insertAll (insertAll [] pa) ia) (c.states.map fun st => (st.name, .var (lifted st.name n))))

/-- The derives, each resolved expression substituted against `m`, inserted. -/
def withDerives (records : List String) (m : SMap) (derives : List (String × Expr)) : SMap :=
  insertAll m (derives.map fun (d, e) => (d, substExpr { map := m, records } e))

/-- Enter arm `arm` of the region tagged `tag`. -/
def inArm {α} (tag arm : Nat) (m : ExM α) : ExM α := do
  modify fun ctx => { ctx with arms := .arm tag arm :: ctx.arms }
  let out ← m
  modify fun ctx => { ctx with arms := ctx.arms.tail }
  pure out

def freshTag : ExM Nat := do
  let ctx ← get
  set { ctx with nextTag := ctx.nextTag + 1 }
  pure ctx.nextTag

mutual
/-- The flat program's nodes for a view, `p`'s uses inlined. `fuel`
bounds the nesting (Rust refuses past depth 32). -/
def inlineNodes (p : CProgram) : Nat → Subst → List CNode → ExM (List Node)
  | 0, _, _ => refuse "out of fuel"
  | _ + 1, _, [] => pure []
  | fuel + 1, s, nd :: rest => do
    let here ← inlineNode p fuel s nd
    let more ← inlineNodes p fuel s rest
    pure (here ++ more)

def inlineNode (p : CProgram) : Nat → Subst → CNode → ExM (List Node)
  | 0, _, _ => refuse "out of fuel"
  | fuel + 1, s, nd =>
    match nd with
    | .element tag pos props hs kids => do
      let hs ← hs.mapM fun (ev, a, args) => do
        let (a, args) ← substHandler s a args
        pure (ev, a, args)
      let kids ← inlineNodes p fuel s kids
      pure [Node.element tag (substList s pos) (substFields s props) hs kids]
    | .when _ c a b => do
      let tag ← freshTag
      let a ← inArm tag 0 (inlineNodes p fuel s a)
      let b ← inArm tag 1 (inlineNodes p fuel s b)
      pure [Node.when tag (substExpr s c) a b]
    | .each _ x ix l k body => do
      let tag ← freshTag
      let body ← inArm tag 0 (inlineNodes p fuel s body)
      pure [Node.each tag x ix (substExpr s l) (substExpr s k) body]
    | .matchN _ subj x a b => do
      let tag ← freshTag
      let a ← inArm tag 0 (inlineNodes p fuel s a)
      let b ← inArm tag 1 (inlineNodes p fuel s b)
      pure [Node.matchN tag (substExpr s subj) x a b]
    | .children _ => do
      match (← get).fill with
      | .some fill => pure fill
      | .none => refuse "`children` belongs in a component that declares `slot`"
    | .use _ name args fill => do
      if (← get).depth > 32 then refuse s!"component `{name}` nests too deeply (a cycle?)"
      let .some c := (if p.root.name == name then Option.some p.root else p.component? name)
        | refuse s!"unknown component `{name}`"
      modify fun ctx => { ctx with counter := ctx.counter + 1 }
      let ctx ← get
      let n := ctx.counter
      let owner := ctx.arms.head?.getD .instance
      let names : List (String × String) :=
        c.states.map (fun st => (st.name, lifted st.name n)) ++
        c.actions.map (fun a => (a.name, lifted a.name n))
      let nameOf := fun (x : String) => ((names.find? (·.1 == x)).map (·.2)).getD x
      let mut child : SMap ← match baseMap s c args ctx.provides n with
        | .ok m => pure m
        | .error e => refuse e
      -- Props and injects (not `action` props), closure-converted into
      -- hidden action parameters.
      let valueProps := (c.props ++ c.injects).filter (!·.action)
      let mut captures : List ((String × Ty) × Expr × String) :=
        valueProps.zipIdx.map fun (pd, i) =>
          ((s!"@capture:{n}:{i}", pd.ty), (child.get? pd.name).getD .none, pd.name)
      let mut called : List (String × String × List Expr) := []
      for (pd, pi) in (c.props ++ c.injects).zipIdx do
        let used := pd.action && !hostCommands.contains pd.name &&
          c.actions.any fun a => commandsIn pd.name a.body
        if !used then continue
        let .some v := child.get? pd.name | continue
        let (target, curried) ← match v with
          | .var "?" => continue
          | .var t => pure (t, [])
          | .call t cs => pure (t, cs)
          | _ => refuse s!"`{pd.name}` is called by an action, so it must name an action"
        let mut held := []
        for (arg, j) in curried.zipIdx do
          let hidden := s!"@capture:{n}:a{pi}:{j}"
          held := held ++ [Expr.var hidden]
          captures := captures ++ [((hidden, .unknown), arg, "")]
        called := called ++ [(pd.name, target, held)]
      for a in c.actions do
        child := child.insert a.name (.call (nameOf a.name) (captures.map (·.2.1)))
      let derives ← match resolvedDerives p.records c with
        | .ok ds => pure ds
        | .error e => refuse e
      let cs : Subst := { map := withDerives p.records child derives, records := p.records }
      let owner' : Option (Nat × Nat) := match owner with
        | .arm t a => Option.some (t, a)
        | _ => Option.none
      for st in c.states do
        modify fun ctx => { ctx with extraStates := ctx.extraStates ++
          [{ name := nameOf st.name, ty := st.ty, init := substExpr cs st.init, owner := owner',
             late := owner == .instance }] }
      for a in c.actions do
        let mut act : SMap := []
        for ((param, _), _, src) in captures do
          if src != "" then act := act.insert src (.var param)
        for st in c.states do
          act := act.insert st.name (.var (nameOf st.name))
        let abase := act
        for (d, e) in derives do
          act := act.insert d (substExpr { map := abase, records := p.records } e)
        for (q, _) in a.params do
          act := act.erase q
        let body := callMarked called
          (withCaptures (captures.map (·.1.1)) (substStmts { map := act, records := p.records } names a.body))
        modify fun ctx => { ctx with extraActions := ctx.extraActions ++
          [{ name := nameOf a.name, params := captures.map (·.1) ++ a.params, body }] }
      if !fill.isEmpty && !c.slot then
        refuse s!"`{name}` declares no `slot`, so nothing can be indented under it"
      let fill' ← if c.slot then do pure (Option.some (← inlineNodes p fuel s fill)) else pure Option.none
      let renamed := renameNodes n [] c.view
      let outer ← get
      let provided : List (String × Expr) := c.provides.map fun (x, e) => (x, substExpr cs e)
      set { outer with fill := fill', depth := outer.depth + 1, provides := outer.provides ++ provided }
      let body ← inlineNodes p fuel cs renamed
      modify fun ctx => { ctx with fill := outer.fill, depth := outer.depth, provides := outer.provides }
      pure body
end

/-- The flat program `p` expands to (inline.rs `expand`): the root's own
declarations (the router's slot first), the inlined view, every lifted
state and action, every call resolved and its caller's names drawn apart
(LLP 1089). -/
def expand (p : CProgram) : Except String Program := do
  let p := ownCallsProgram p
  let root := p.root
  let routerState : List StateDecl := match p.router with
    | .some x => [{ name := x, ty := .record "Router", init := .none }]
    | .none => []
  let s0 : Subst := { map := [], records := p.records }
  let provides := root.provides.map fun (x, e) => (x, substExpr s0 e)
  let (view, ctx) ← (inlineNodes p (1 <<< 20) s0 root.view).run { provides }
  let actions := root.actions ++ ctx.extraActions
  let actions ← actions.mapM fun a => do pure { a with body := ← resolveCalls actions a.body }
  let (actions, _) ← (hygiene p.records actions).run 0
  pure { shapes := p.shapes, fns := p.fns,
         states := routerState ++ root.states ++ ctx.extraStates,
         derives := root.derives, resources := root.resources, mutations := root.mutations,
         actions, tasks := root.tasks, view, routes := p.routes, router := p.router }

end Contract.Expand
