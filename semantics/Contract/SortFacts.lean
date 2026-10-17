/-
What `sortBy` computes (LLP 1116.016 §5), proved of `Contract.Sort`'s
definitions with core's merge sort lemmas: the key order is a strict total
order (NaN, -0 and +0 one key; strings by UTF-16 code units), so the sort's
answer is a permutation of the items, sorted by key in the direction asked,
with equal keys in input order.
-/
import Contract.Sort
import Contract.Types
import Contract.Big

namespace Contract

/-! ## The key order is a strict total order -/

namespace Str

theorem unitsLt_irrefl : ∀ a : List Nat, unitsLt a a = false
  | [] => rfl
  | x :: xs => by simp [unitsLt, unitsLt_irrefl xs]

theorem unitsLt_trans : ∀ a b c : List Nat, unitsLt a b = true → unitsLt b c = true → unitsLt a c = true
  | [], [], _, h, _ => by simp [unitsLt] at h
  | [], _ :: _, [], _, h => by simp [unitsLt] at h
  | [], _ :: _, _ :: _, _, _ => rfl
  | _ :: _, [], _, h, _ => by simp [unitsLt] at h
  | _ :: _, _ :: _, [], _, h => by simp [unitsLt] at h
  | x :: xs, y :: ys, z :: zs, h1, h2 => by
    simp only [unitsLt, Bool.or_eq_true, decide_eq_true_eq, Bool.and_eq_true, beq_iff_eq] at h1 h2 ⊢
    rcases h1 with h1 | ⟨rfl, h1⟩ <;> rcases h2 with h2 | ⟨rfl, h2⟩
    · left; omega
    · left; exact h1
    · left; exact h2
    · right; exact ⟨rfl, unitsLt_trans xs ys zs h1 h2⟩

theorem unitsLt_trichotomy : ∀ a b : List Nat, unitsLt a b = true ∨ a = b ∨ unitsLt b a = true
  | [], [] => by simp
  | [], _ :: _ => by simp [unitsLt]
  | _ :: _, [] => by simp [unitsLt]
  | x :: xs, y :: ys => by
    simp only [unitsLt, Bool.or_eq_true, decide_eq_true_eq, Bool.and_eq_true, beq_iff_eq,
      List.cons.injEq]
    rcases Nat.lt_trichotomy x y with h | rfl | h
    · exact .inl (.inl h)
    · rcases unitsLt_trichotomy xs ys with h | h | h
      · exact .inl (.inr ⟨rfl, h⟩)
      · exact .inr (.inl ⟨rfl, h⟩)
      · exact .inr (.inr (.inr ⟨rfl, h⟩))
    · exact .inr (.inr (.inl h))

end Str

namespace SortKey

theorem lt_irrefl (a : SortKey) : lt a a = false := by
  cases a <;> simp [lt, Str.unitsLt_irrefl]

theorem lt_trans {a b c : SortKey} : lt a b = true → lt b c = true → lt a c = true := by
  cases a <;> cases b <;> cases c <;> simp [lt] <;> first | omega | exact Str.unitsLt_trans _ _ _

theorem lt_trichotomy (a b : SortKey) : lt a b = true ∨ a = b ∨ lt b a = true := by
  cases a <;> cases b <;> simp [lt]
  · omega
  · exact Str.unitsLt_trichotomy _ _

theorem lt_cotrans {a b c : SortKey} (h : lt a c = true) : lt a b = true ∨ lt b c = true := by
  rcases lt_trichotomy b c with h1 | rfl | h1
  · exact .inr h1
  · exact .inl h
  · exact .inl (lt_trans h h1)

end SortKey

theorem sortLe_trans (desc : Bool) (a b c : Value × SortKey) :
    sortLe desc a b = true → sortLe desc b c = true → sortLe desc a c = true := by
  cases desc <;> simp only [sortLe, Bool.false_eq_true, ↓reduceIte, Bool.not_eq_true'] <;> intro h1 h2
  · cases h : SortKey.lt c.2 a.2
    · rfl
    · rcases SortKey.lt_cotrans (b := b.2) h with h3 | h3 <;> simp_all
  · cases h : SortKey.lt a.2 c.2
    · rfl
    · rcases SortKey.lt_cotrans (b := b.2) h with h3 | h3 <;> simp_all

theorem sortLe_total (desc : Bool) (a b : Value × SortKey) :
    (sortLe desc a b || sortLe desc b a) = true := by
  have asym : ¬ (SortKey.lt a.2 b.2 = true ∧ SortKey.lt b.2 a.2 = true) := fun ⟨h1, h2⟩ => by
    have := SortKey.lt_trans h1 h2; rw [SortKey.lt_irrefl] at this; cases this
  cases desc <;> cases h1 : SortKey.lt a.2 b.2 <;> cases h2 : SortKey.lt b.2 a.2 <;> simp_all [sortLe]

/-- Two items whose keys are equal may stand either way round. -/
theorem sortLe_of_key_eq (desc : Bool) {a b : Value × SortKey} (h : a.2 = b.2) :
    sortLe desc a b = true := by
  cases desc <;> simp [sortLe, h, SortKey.lt_irrefl]

/-! ## What `sortBy` computes -/

/-- The sorted pairs are a permutation of the input. -/
theorem sortPairs_perm (desc : Bool) (ps : List (Value × SortKey)) : (sortPairs desc ps).Perm ps :=
  List.mergeSort_perm ps _

/-- Sorted: every pair of the output is in key order (ascending, or
descending under `desc`). -/
theorem sortPairs_sorted (desc : Bool) (ps : List (Value × SortKey)) :
    (sortPairs desc ps).Pairwise (fun a b => sortLe desc a b = true) :=
  List.pairwise_mergeSort (sortLe_trans desc) (sortLe_total desc) ps

/-- Stable: items of equal keys keep their input order. Any sublist of the
input whose keys are all equal is a sublist of the output. -/
theorem sortPairs_stable (desc : Bool) {ps c : List (Value × SortKey)} (hc : c.Sublist ps)
    (heq : c.Pairwise (fun a b => a.2 = b.2)) : c.Sublist (sortPairs desc ps) :=
  List.sublist_mergeSort (sortLe_trans desc) (sortLe_total desc)
    (heq.imp (sortLe_of_key_eq desc)) hc

/-- `sortBy` answers the first components of a list of (item, key) pairs
that is a permutation of the items zipped with their keys, sorted by key,
and in which every run of equal keys is in input order. -/
theorem sortByKeys_spec {xs ks : List Value} {desc : Bool} {v : Value}
    (h : sortByKeys xs ks desc = .ok v) :
    ∃ (keys : List SortKey) (ps : List (Value × SortKey)), ks.mapM sortKey = .some keys ∧ xs.length = keys.length ∧ v = .list (ps.map Prod.fst) ∧
      ps.Perm (xs.zip keys) ∧ ps.Pairwise (fun a b => sortLe desc a b = true) ∧
      ∀ c : List (Value × SortKey), c.Sublist (xs.zip keys) → c.Pairwise (fun a b => a.2 = b.2) → c.Sublist ps := by
  unfold sortByKeys at h
  split at h
  · cases h
  · rename_i keys hk
    split at h
    · rename_i hl
      cases h
      exact ⟨keys, _, hk, hl.1, rfl, sortPairs_perm desc _, sortPairs_sorted desc _,
        fun c hc heq => sortPairs_stable desc hc heq⟩
    · cases h

/-- `sortBy` answers a permutation of its items. -/
theorem sortByKeys_perm {xs ks : List Value} {desc : Bool} {ys : List Value}
    (h : sortByKeys xs ks desc = .ok (.list ys)) : ys.Perm xs := by
  obtain ⟨keys, ps, -, hl, hv, hp, -, -⟩ := sortByKeys_spec h
  cases hv
  have := hp.map Prod.fst
  rwa [List.map_fst_zip (by omega)] at this

/-! ## When `sortBy` answers -/

theorem mapM_sortKey_num : ∀ (ks : List Value), (∀ k ∈ ks, ∃ x, k = .num x) →
    ∃ keys, ks.mapM sortKey = .some keys ∧ keys.length = ks.length ∧ ∀ q ∈ keys, ∃ i, q = SortKey.num i
  | [], _ => ⟨[], rfl, rfl, by simp⟩
  | k :: ks, h => by
    obtain ⟨x, rfl⟩ := h k (by simp)
    obtain ⟨keys, hm, hl, hall⟩ := mapM_sortKey_num ks (fun k hk => h k (by simp [hk]))
    refine ⟨SortKey.num (if x.isNaN then 0 else x.key) :: keys, by simp [List.mapM_cons, sortKey, hm], by simp [hl], ?_⟩
    intro q hq
    simp only [List.mem_cons] at hq
    rcases hq with rfl | hq
    · exact ⟨_, rfl⟩
    · exact hall q hq

theorem mapM_sortKey_str : ∀ (ks : List Value), (∀ k ∈ ks, ∃ s, k = .str s) →
    ∃ keys, ks.mapM sortKey = .some keys ∧ keys.length = ks.length ∧ ∀ q ∈ keys, ∃ us, q = SortKey.str us
  | [], _ => ⟨[], rfl, rfl, by simp⟩
  | k :: ks, h => by
    obtain ⟨x, rfl⟩ := h k (by simp)
    obtain ⟨keys, hm, hl, hall⟩ := mapM_sortKey_str ks (fun k hk => h k (by simp [hk]))
    refine ⟨SortKey.str (Str.utf16Units x) :: keys, by simp [List.mapM_cons, sortKey, hm], by simp [hl], ?_⟩
    intro q hq
    simp only [List.mem_cons] at hq
    rcases hq with rfl | hq
    · exact ⟨_, rfl⟩
    · exact hall q hq

theorem mapM_sortKey_one (ks : List Value) (h : OneKind ks) :
    ∃ keys, ks.mapM sortKey = .some keys ∧ keys.length = ks.length ∧ SortKey.sameKind keys = true := by
  rcases h with h | h
  · obtain ⟨keys, hm, hl, hall⟩ := mapM_sortKey_num ks h
    refine ⟨keys, hm, hl, ?_⟩
    match keys, hall with
    | [], _ => rfl
    | q :: qs, hall =>
      obtain ⟨i, rfl⟩ := hall q (by simp)
      simp only [SortKey.sameKind, List.all_eq_true]
      intro r hr; obtain ⟨j, rfl⟩ := hall r (by simp [hr]); rfl
  · obtain ⟨keys, hm, hl, hall⟩ := mapM_sortKey_str ks h
    refine ⟨keys, hm, hl, ?_⟩
    match keys, hall with
    | [], _ => rfl
    | q :: qs, hall =>
      obtain ⟨i, rfl⟩ := hall q (by simp)
      simp only [SortKey.sameKind, List.all_eq_true]
      intro r hr; obtain ⟨j, rfl⟩ := hall r (by simp [hr]); rfl

/-- Keys of one kind, one for each item: `sortBy` answers a list. -/
theorem sortByKeys_ok {xs ks : List Value} {d : Bool} (hl : xs.length = ks.length) (hk : OneKind ks) :
    ∃ ys, sortByKeys xs ks d = .ok (.list ys) := by
  obtain ⟨keys, hm, hkl, hs⟩ := mapM_sortKey_one ks hk
  unfold sortByKeys
  rw [hm]
  simp only
  rw [ite_eq_left_of_eq_true _ _ (eq_true ⟨by omega, hs⟩)]
  exact ⟨_, rfl⟩

/-- Keys of a `sortBy` key type are of one kind. -/
theorem oneKind_of_valTys {p : Program} {ks : List Value} {k : Ty} (hks : ValTys p ks k)
    (hk : k.sortKey = true) : OneKind ks := by
  cases k <;> simp [Ty.sortKey] at hk
  · exact .inl fun v hv => by
      have := hks.mem v hv; cases v <;> simp [ValTy] at this; exact ⟨_, rfl⟩
  · exact .inr fun v hv => by
      have := hks.mem v hv; cases v <;> simp [ValTy] at this; exact ⟨_, rfl⟩
  · exact .inl fun v hv => absurd (hks.mem v hv) ValTy.unknown

theorem evalMap_length : ∀ {n env inFn ls ps body xs i ys},
    evalMap n env inFn ls ps body xs i = .ok ys → xs.length = ys.length
  | 0, _, _, _, _, _, _, _, _, h => by simp [evalMap] at h
  | _ + 1, _, _, _, _, _, [], _, _, h => by simp [evalMap] at h; subst h; rfl
  | n + 1, env, inFn, ls, ps, body, x :: xs, i, ys, h => by
    simp only [evalMap, Except.bind_ok_iff] at h
    obtain ⟨y, -, ys', h2, h3⟩ := h
    simp at h3; subst h3
    simp [evalMap_length h2]

end Contract
