/-
Navigation tab selection (runner/src/instance/tabs.rs and head.rs).
The renderers supply the bound properties and the surviving panel identities;
selection opens waiting panels until a made route holds its root's key.
-/
import Contract.Eval

namespace Contract

/-- A view site's path, including each enclosing arm or keyed row. -/
abbrev TabId := List (Nat × String × Nat)

def tabKey : Value → String
  | .num n => "n" ++ toString n.toBits
  | .str s => "s" ++ s
  | .bool b => if b then "true" else "false"
  | _ => ""

/-- The attributes that tab selection reads. -/
def tabProps (props : List (String × Expr)) : List (String × Expr) :=
  props.filter fun (name, _) =>
    ["role", "id", "navigationKey", "navigationBack", "aria-controls", "aria-selected"].contains name

/-- A literal through the ascriptions typed component expansion inserts. -/
def tabLiteral : Expr → Expr
  | .typed e _ => tabLiteral e
  | e => e

structure TabInfo where
  path : TabId := []
  parent : Option TabId := .none
  props : List (String × Value) := []
  panel : Bool := false
  opened : Bool := true
  deriving Inhabited

def TabInfo.str (n : TabInfo) (name : String) : Option String :=
  match lookup name n.props with
  | .some (.str s) => .some s
  | _ => .none

structure TabCx where
  path : TabId := []
  site : Nat := 0
  parent : Option TabId := .none
  /-- The nearest ancestor binding `navigationKey` also binds `navigationBack`. -/
  tabs : Bool := false
  opened : List TabId := []
  /-- Every panel is built as it is made (a page rendered ahead of its
  runtime: `Runner::build_every_tab`), so none waits. -/
  every : Bool := false
  deriving Inhabited

def TabCx.enter (cx : TabCx) (key : Value := .unit) (dup : Nat := 0) : TabCx :=
  { cx with path := cx.path ++ [(cx.site, tabKey key, dup)], site := 0 }

def TabCx.next (cx : TabCx) : TabCx := { cx with site := cx.site + 1 }

def TabCx.element (cx : TabCx) (props : List (String × Expr))
    (values : List (String × Value)) : TabInfo :=
  let path := (cx.enter).path
  let panel := cx.tabs && ((lookupField "role" props).map tabLiteral matches .some (.str "tabpanel"))
  { path, parent := cx.parent, props := values, panel,
    opened := !panel || cx.every || cx.opened.contains path }

def TabCx.children (cx : TabCx) (props : List (String × Expr)) : TabCx :=
  let inner := cx.enter
  { inner with
    parent := .some inner.path,
    tabs := if (lookupField "navigationKey" props).isSome
      then (lookupField "navigationBack" props).isSome else cx.tabs }

namespace Tabs

abbrev Index := List TabInfo

def children (index : Index) (path : TabId) : Index :=
  index.filter (·.parent == .some path)

def rootOf (index : Index) (panel : TabInfo) : Option TabInfo :=
  let rec go : Nat → Option TabId → Option TabInfo
    | 0, _ => .none
    | _ + 1, .none => .none
    | fuel + 1, .some path =>
      match index.find? (·.path == path) with
      | .none => .none
      | .some n =>
        if (n.str "navigationKey").isSome then
          if (n.str "navigationBack").isSome then .some n else .none
        else go fuel n.parent
  go (index.length + 1) panel.parent

/-- The first tablist with named panels, in tab order. A route's tablist
belongs to that route, as in `head.rs` `tab_panels`. -/
def panels (index : Index) (root : TabInfo) : Index := Id.run do
  let rec scan : Nat → Index → Index
    | 0, _ => []
    | _ + 1, [] => []
    | fuel + 1, n :: rest =>
      let kids := if (n.str "navigationKey").isNone && n.str "role" != .some "tablist"
        then children index n.path else []
      n :: scan fuel (kids ++ rest)
  let zone := scan (index.length + 1) (children index root.path)
  let candidates := zone.filter (·.str "role" == .some "tabpanel")
  for list in zone.filter (·.str "role" == .some "tablist") do
    let named := (children index list.path).filterMap fun tab =>
      if tab.str "role" != .some "tab" then .none else
      (tab.str "aria-controls").bind fun name => candidates.find? (·.str "id" == .some name)
    if !named.isEmpty then return named
  return []

/-- At most one waiting panel per navigation root, plus every panel no
root's tablist names. Repeating this after rendering the opened contents
also opens all panels when none of their routes holds the root's key. -/
def toOpen (index : Index) : List TabId := Id.run do
  let named := index.filterMap fun n =>
    if (lookup "aria-selected" n.props matches .some (.bool true)) then n.str "aria-controls" else .none
  let mut roots : List TabId := []
  let mut opened : List TabId := []
  for w in index.filter (fun n => n.panel && !n.opened) do
    let root := rootOf index w
    let ps := root.map (panels index) |>.getD []
    match root with
    | .none => opened := opened ++ [w.path]
    | .some root =>
      if !ps.any (·.path == w.path) then
        opened := opened ++ [w.path]
      else if !roots.contains root.path then
        roots := roots ++ [root.path]
        let held := ps.any fun p =>
          (children index p.path).any (·.str "navigationKey" == root.str "navigationKey")
        if !held then
          let waiting := ps.filter (fun p => p.panel && !p.opened)
          let chosen := (waiting.find? fun p => (p.str "id").any named.contains).orElse fun _ => waiting.head?
          opened := opened ++ (chosen.map (·.path)).toList
  return opened

def visited (index : Index) : List TabId :=
  (index.filter fun n => n.panel && n.opened).map (·.path)

end Tabs
end Contract
