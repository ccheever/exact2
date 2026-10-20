/-
`sortBy` (LLP 1116.016 §5): a list's items in a stable order of one key, a
number or a string, ascending or descending.

The order is core's `List.mergeSort` on the items decorated with their
keys. The runner (`lists::sort_by`) and the JS target (`x_sortBy`) run a
written-out bottom-up merge sort instead; any stable sort by a total
preorder gives one result, which is what `sortByKeys_spec`
(`Contract.SortFacts`) pins down: a permutation, sorted by key, keeping
equal keys in input order. The proofs are kept out of this file, which the
VM model (and so the extracted VM's build, at another Lean) imports.
-/
import Contract.Value

namespace Contract

/-- A `sortBy` key: a number as its order key (`F64.key`) with NaN read as
+0, so NaN, -0 and +0 tie; a string as its UTF-16 code units, compared as
`<` compares strings. -/
inductive SortKey where
  | num (k : Int)
  | str (us : List Nat)
  deriving DecidableEq, Repr

namespace SortKey

/-- The strict order: numbers by value, strings by code units (JavaScript's
`IsLessThan`). A number before a string, an order no well-typed program
asks for (`sortByKeys` refuses mixed keys). -/
def lt : SortKey → SortKey → Bool
  | .num a, .num b => decide (a < b)
  | .str a, .str b => Str.unitsLt a b
  | .num _, .str _ => true
  | .str _, .num _ => false

end SortKey

/-- A value's key: a number or a string, else none. -/
def sortKey : Value → Option SortKey
  | .num x => .some (.num (if x.isNaN then 0 else x.key))
  | .str s => .some (.str (Str.utf16Units s))
  | _ => .none

/-- Whether every key is a number, or every key a string. -/
def SortKey.sameKind : List SortKey → Bool
  | [] => true
  | .num _ :: ks => ks.all fun | .num _ => true | _ => false
  | .str _ :: ks => ks.all fun | .str _ => true | _ => false

/-- Item `a` may stay before item `b`: ascending, `b`'s key is not below
`a`'s; descending, not above. Equal keys are both ways, so the stable sort
keeps them in input order in both directions. -/
def sortLe (desc : Bool) (a b : Value × SortKey) : Bool :=
  if desc then !SortKey.lt a.2 b.2 else !SortKey.lt b.2 a.2

/-- The decorated items, sorted. -/
def sortPairs (desc : Bool) (ps : List (Value × SortKey)) : List (Value × SortKey) :=
  ps.mergeSort (sortLe desc)

/-- `sortBy` once its keys are computed: the roster's
`sortBy(list, keys, descending)`. -/
def sortByKeys (xs ks : List Value) (desc : Bool) : Result Value :=
  match ks.mapM sortKey with
  | .none => .error (.type "a `sortBy` key that is not a number or a string")
  | .some keys =>
    if xs.length = keys.length ∧ SortKey.sameKind keys = true then
      .ok (.list ((sortPairs desc (xs.zip keys)).map Prod.fst))
    else .error (.type "`sortBy` keys that do not match its items")

/-- Keys of one kind: every one a number, or every one a string. -/
def OneKind (ks : List Value) : Prop :=
  (∀ k ∈ ks, ∃ x, k = .num x) ∨ (∀ k ∈ ks, ∃ s, k = .str s)

end Contract
