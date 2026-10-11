/-
Substitution, backwards: a value of the substituted expression in the
target is a value of the expression in the source; and `subst_iff`, both
directions together (`Contract.ExpandSubst` has the forward direction and
the lemmas both use).
-/
import Contract.ExpandSubst

namespace Contract.ExpandSubst

open Contract Expand

/-! ## Substitution, backwards: a target value is a source value -/

/-- No replacement is a callback: `map(xs, f)` with `f` a prop would be
a call no rule evaluates in the source, but one in the target. -/
def MapNoArrow (s : Subst) : Prop := ∀ p ∈ s.map, ∀ ps b, p.2 ≠ .arrow ps b

theorem subst_record (s : Subst) (sh : String) (base : Option Expr) (fields : List (String × Expr)) :
    substExpr s (.record sh base fields) = .record sh (base.map (substExpr s)) (substFields s fields) := by
  cases base <;> rfl

theorem subst_eq_arrow {s : Subst} (hn : MapNoArrow s) {a : Expr} {ps' : List String} {b' : Expr}
    (h : substExpr s a = .arrow ps' b') :
    ∃ ps b, a = .arrow ps b ∧ ps' = (enterParams b ps s ps).1 ∧ b' = substExpr (enterParams b ps s ps).2 b := by
  cases a with
  | var x =>
    simp only [substExpr] at h
    split at h
    · cases h
    · rename_i r hg
      obtain ⟨k, hk⟩ := get_expr hg
      exact absurd h (hn _ hk _ _)
    · cases h
  | arrow ps b =>
    rw [subst_arrow] at h
    cases h
    exact ⟨ps, b, rfl, rfl, rfl⟩
  | matchOpt => simp only [subst_match] at h; cases h
  | letE => simp only [subst_let] at h; cases h
  | call n args =>
    simp only [substExpr] at h
    split at h
    · cases h
    · split at h <;> cases h
  | num => simp only [substExpr] at h; cases h
  | str => simp only [substExpr] at h; cases h
  | bool => simp only [substExpr] at h; cases h
  | none => simp only [substExpr] at h; cases h
  | list => simp only [substExpr] at h; cases h
  | some => simp only [substExpr] at h; cases h
  | template => simp only [substExpr] at h; cases h
  | member => simp only [substExpr] at h; cases h
  | record => rw [subst_record] at h; cases h
  | unary => simp only [substExpr] at h; cases h
  | binary => simp only [substExpr] at h; cases h
  | ternary => simp only [substExpr] at h; cases h
  | named => simp only [substExpr] at h; cases h
  | typed => simp only [substExpr] at h; cases h

theorem substList_eq_two {s : Subst} {args : List Expr} {a b : Expr} (h : substList s args = [a, b]) :
    ∃ x y, args = [x, y] ∧ substExpr s x = a ∧ substExpr s y = b := by
  match args, h with
  | [x, y], h => simp [substList] at h; exact ⟨x, y, rfl, h.1, h.2⟩
  | [], h => simp [substList] at h
  | [x], h => simp [substList] at h
  | _ :: _ :: _ :: _, h => simp [substList] at h

theorem lookupField_subst_some {s : Subst} {f : String} {w : List (String × Expr)} {e' : Expr}
    (h : lookupField f (substFields s w) = .some e') : ∃ e, lookupField f w = .some e ∧ substExpr s e = e' := by
  rw [lookupField_subst] at h
  cases hl : lookupField f w with
  | none => rw [hl] at h; cases h
  | some e => rw [hl] at h; cases h; exact ⟨e, rfl, rfl⟩

theorem lookupField_subst_none {s : Subst} {f : String} {w : List (String × Expr)}
    (h : lookupField f (substFields s w) = .none) : lookupField f w = .none := by
  rw [lookupField_subst] at h
  cases hl : lookupField f w with
  | none => rfl
  | some e => rw [hl] at h; cases h

theorem sb_var {e₁ e₂ : Env} {inFn Lf t v} {x : String} {s : Subst} {ls : Locals}
    (h : EvalR e₂ inFn Lf t v) (hf : inFn = false) (he : substExpr s (.var x) = t)
    (hA : ∀ y ∈ fv (.var x), Agree e₁ ls e₂ Lf s y) : EvalR e₁ false ls (.var x) v := by
  subst he hf
  exact var_iff.2 ((hA x (by simp [fv]) v).2 ((substVar e₂ Lf s x v).1 h))

set_option hygiene false in
/-- A substitution whose shape is not the one a rule needs. -/
local macro "contra_e" : tactic => `(tactic| first
  | (cases he)
  | (rw [subst_record] at he; cases he)
  | (rw [subst_match] at he; cases he)
  | (rw [subst_let] at he; cases he)
  | (rw [subst_arrow] at he; cases he)
  | (simp only [substExpr] at he; cases he)
  | (simp only [substExpr] at he; split at he <;> (try split at he) <;> cases he))

set_option maxHeartbeats 4000000 in
mutual
theorem sbE {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf t v} (h : EvalR e₂ inFn Lf t v) (hf : inFn = false)
    {e : Expr} {s : Subst} {ls : Locals} (he : substExpr s e = t) (hm : MapSmall s) (hn : MapNoArrow s)
    (hA : ∀ y ∈ fv e, Agree e₁ ls e₂ Lf s y) (hp : Plain e = true)
    (hh : ∀ n ∈ callHeads e, s.get n = .none) : EvalR e₁ false ls e v :=
  match h with
  | hd@(.num) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | num b => simp only [substExpr] at he; cases he; exact .num
    | _ => contra_e
  | hd@(.str) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | str b => simp only [substExpr] at he; cases he; exact .str
    | _ => contra_e
  | hd@(.bool) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | bool b => simp only [substExpr] at he; cases he; exact .bool
    | _ => contra_e
  | hd@(.none) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | none => exact .none
    | _ => contra_e
  | hd@(.list h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | list items =>
      simp only [substExpr] at he; injection he with he1
      exact .list (sbL hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
  | hd@(.some h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | some o =>
      simp only [substExpr] at he; injection he with he1
      exact .some (sbE hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
  | hd@(.template h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | template parts =>
      simp only [substExpr] at he; injection he with he1
      exact .template (sbD hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
  | hd@(.local _) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | _ => contra_e
  | hd@(.global _ _ _) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | _ => contra_e
  | hd@(.member h1 hi hv) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | member o f =>
      simp only [substExpr] at he; injection he with he1 hfld; subst hfld
      exact .member (sbE hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh)) (hs.fieldIndex _ _ ▸ hi) hv
    | _ => contra_e
  | hd@(.fn hfd ha hb) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' hargs
      subst hn'
      simp only [Plain, Bool.and_eq_true] at hp
      exact .fn (hs.symm.fns ▸ hfd)
        (sbL hs ha hf hargs hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
          (fun m hm' => hh m (by simp [callHeads, hm'])))
        (xferE hs.symm hb rfl)
    | _ => contra_e
  | hd@(.stdlib hfd ha hv) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' hargs
      subst hn'
      simp only [Plain, Bool.and_eq_true] at hp
      exact .stdlib (hs.symm.fns ▸ hfd)
        (sbL hs ha hf hargs hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
          (fun m hm' => hh m (by simp [callHeads, hm'])))
        (hs.symm.stdlib_eq _ _ ▸ hv)
    | _ => contra_e
  | hd@(.pendingSettled _ _ _) | hd@(.pendingOther _ _) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' _
      subst hn'
      simp [Plain] at hp
    | _ => contra_e
  | hd@(.failedSettled _ _ _) | hd@(.failedOther _ _) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' _
      subst hn'
      simp [Plain] at hp
    | _ => contra_e
  | hd@(.failureSettled _ _ _) | hd@(.failureOther _ _) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' _
      subst hn'
      simp [Plain] at hp
    | _ => contra_e
  | hd@(.map hfd hl hmap) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' he2
      subst hn'
      obtain ⟨x1, x2, rfl, h1, h2⟩ := substList_eq_two he2
      obtain ⟨ps, body, rfl, hps, hbody⟩ := subst_eq_arrow hn h2
      obtain ⟨hpl, hlen, hnc, hpb⟩ := plain_cb hp
      have hhb : ∀ m ∈ callHeads body, (enterParams body ps s ps).2.get m = .none := fun m hm' => by
        rw [enterParams_get body ps s ps (fun hmem => hnc m hmem hm')]
        exact hh m (by simp [callHeads, callHeadsList, hm'])
      have hmb : MapSmall (enterParams body ps s ps).2 := by
        intro r hr; rw [enterParams_map] at hr; exact hm r hr
      have hnb : MapNoArrow (enterParams body ps s ps).2 := by
        intro r hr; rw [enterParams_map] at hr; exact hn r hr
      exact .map (hs.symm.fns ▸ hfd)
        (sbE hs hl hf h1 hm hn (fun y hy => hA y (by simp [fv, fvList, hy])) hpl
          (fun m hm' => hh m (by simp [callHeads, callHeadsList, hm'])))
        (sbM hs hmap hf hbody.symm hmb hnb
          (fun x i => by
            rw [hps]
            exact agree_params hm hlen
              (fun y hy => hA y (by simp [fv, fvList] at hy ⊢; grind)) x i) hpb hhb)
    | _ => contra_e
  | hd@(.filter hfd hl hmap) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | call n args =>
      rw [subst_call (hh n (by simp [callHeads]))] at he
      injection he with hn' he2
      subst hn'
      obtain ⟨x1, x2, rfl, h1, h2⟩ := substList_eq_two he2
      obtain ⟨ps, body, rfl, hps, hbody⟩ := subst_eq_arrow hn h2
      obtain ⟨hpl, hlen, hnc, hpb⟩ := plain_cb hp
      have hhb : ∀ m ∈ callHeads body, (enterParams body ps s ps).2.get m = .none := fun m hm' => by
        rw [enterParams_get body ps s ps (fun hmem => hnc m hmem hm')]
        exact hh m (by simp [callHeads, callHeadsList, hm'])
      have hmb : MapSmall (enterParams body ps s ps).2 := by
        intro r hr; rw [enterParams_map] at hr; exact hm r hr
      have hnb : MapNoArrow (enterParams body ps s ps).2 := by
        intro r hr; rw [enterParams_map] at hr; exact hn r hr
      exact .filter (hs.symm.fns ▸ hfd)
        (sbE hs hl hf h1 hm hn (fun y hy => hA y (by simp [fv, fvList, hy])) hpl
          (fun m hm' => hh m (by simp [callHeads, callHeadsList, hm'])))
        (sbF hs hmap hf hbody.symm hmb hnb
          (fun x i => by
            rw [hps]
            exact agree_params hm hlen
              (fun y hy => hA y (by simp [fv, fvList] at hy ⊢; grind)) x i) hpb hhb)
    | _ => contra_e
  | hd@(.record hd' h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | record shape base fields =>
      cases base with
      | none =>
        simp only [substExpr, Option.map] at he
        injection he with hsh _ hfs
        subst hsh
        simp only [Plain, Bool.true_and] at hp
        exact .record (hs.symm.shape _ ▸ hd')
          (sbR hs h1 hf hfs hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp
            (fun m hm' => hh m (by simp [callHeads, hm'])))
      | some b => simp only [substExpr, Option.map] at he; cases he
    | _ => contra_e
  | hd@(.recordBase hb hd' h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | record shape base fields =>
      cases base with
      | none => simp only [substExpr, Option.map] at he; cases he
      | some b =>
        simp only [substExpr, Option.map] at he
        injection he with hsh hbase hfs
        injection hbase with hbase
        subst hsh
        simp only [Plain, Bool.and_eq_true] at hp
        exact .recordBase
          (sbE hs hb hf hbase hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
            (fun m hm' => hh m (by simp [callHeads, hm'])))
          (hs.symm.shape _ ▸ hd')
          (sbR hs h1 hf hfs hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
            (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.neg h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | unary op o =>
      simp only [substExpr] at he; injection he with hop he1; subst hop
      exact .neg (sbE hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
  | hd@(.not h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | unary op o =>
      simp only [substExpr] at he; injection he with hop he1; subst hop
      exact .not (sbE hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
  | hd@(.andFalse h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | binary op a b =>
      simp only [substExpr] at he; injection he with hop he1 he2; subst hop
      simp only [Plain, Bool.and_eq_true] at hp
      exact .andFalse (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.andTrue h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | binary op a b =>
      simp only [substExpr] at he; injection he with hop he1 he2; subst hop
      simp only [Plain, Bool.and_eq_true] at hp
      exact .andTrue (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.orTrue h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | binary op a b =>
      simp only [substExpr] at he; injection he with hop he1 he2; subst hop
      simp only [Plain, Bool.and_eq_true] at hp
      exact .orTrue (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.orFalse h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | binary op a b =>
      simp only [substExpr] at he; injection he with hop he1 he2; subst hop
      simp only [Plain, Bool.and_eq_true] at hp
      exact .orFalse (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.binop h1 h2 h3) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | binary op a b =>
      simp only [substExpr] at he; injection he with hop he1 he2; subst hop
      simp only [Plain, Bool.and_eq_true] at hp
      exact .binop (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun m hm' => hh m (by simp [callHeads, hm']))) h3
    | _ => contra_e
  | hd@(.ternaryTrue h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | ternary c a b =>
      simp only [substExpr] at he; injection he with he1 he2 he3
      simp only [Plain, Bool.and_eq_true] at hp
      exact .ternaryTrue (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.2
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.ternaryFalse h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | ternary c a b =>
      simp only [substExpr] at he; injection he with he1 he2 he3
      simp only [Plain, Bool.and_eq_true] at hp
      exact .ternaryFalse (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.1.1
        (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he3 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hp.2
        (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.matchSome h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | matchOpt subj x a b =>
      rw [subst_match] at he; injection he with he1 he2 he3 he4
      obtain ⟨hps, hnc, hpa, _⟩ := plain_match hp
      have hma : MapSmall (s.enter x (allNames a)).2 := by
        intro r hr; rw [enter_map] at hr; exact hm r hr
      have hna : MapNoArrow (s.enter x (allNames a)).2 := by
        intro r hr; rw [enter_map] at hr; exact hn r hr
      exact .matchSome
        (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hps
          (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he3 hma hna
          (by rw [← he2]; exact agree_enter hm (fun y hy hne => hA y (by simp [fv, hy, hne])) (fun y hy => .inr (fv_sub_all _ hy))) hpa
          (fun m hm' => by
            rw [enter_get_other s x _ (fun he => by subst he; exact hnc hm')]
            exact hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.matchNone h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | matchOpt subj x a b =>
      rw [subst_match] at he; injection he with he1 he2 he3 he4
      obtain ⟨hps, _, _, hpb⟩ := plain_match hp
      exact .matchNone
        (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hps
          (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he4 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hpb
          (fun m hm' => hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.letE h1 h2) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | letE x v0 body =>
      rw [subst_let] at he; injection he with he1 he2 he3
      obtain ⟨hpv, hnc, hpb⟩ := plain_let hp
      have hma : MapSmall (s.enter x (allNames body)).2 := by
        intro r hr; rw [enter_map] at hr; exact hm r hr
      have hna : MapNoArrow (s.enter x (allNames body)).2 := by
        intro r hr; rw [enter_map] at hr; exact hn r hr
      exact .letE
        (sbE hs h1 hf he2 hm hn (fun y hy => hA y (by simp [fv, fvOpt, hy])) hpv
          (fun m hm' => hh m (by simp [callHeads, hm'])))
        (sbE hs h2 hf he3 hma hna
          (by rw [← he1]; exact agree_enter hm (fun y hy hne => hA y (by simp [fv, hy, hne])) (fun y hy => .inr (fv_sub_all _ hy))) hpb
          (fun m hm' => by
            rw [enter_get_other s x _ (fun he => by subst he; exact hnc hm')]
            exact hh m (by simp [callHeads, hm'])))
    | _ => contra_e
  | hd@(.typed h1) => by
    cases e with
    | var x => exact sb_var hd hf he hA
    | typed o ty =>
      simp only [substExpr] at he; injection he with he1 _
      exact .typed (sbE hs h1 hf he1 hm hn (by simpa [fv] using hA)
        (by simpa [Plain] using hp) (by simpa [callHeads] using hh))
    | _ => contra_e
theorem sbL {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf ts vs} (h : ListR e₂ inFn Lf ts vs) (hf : inFn = false)
    {es : List Expr} {s : Subst} {ls : Locals} (he : substList s es = ts) (hm : MapSmall s)
    (hn : MapNoArrow s) (hA : ∀ y ∈ fvList es, Agree e₁ ls e₂ Lf s y) (hp : PlainList es = true)
    (hh : ∀ n ∈ callHeadsList es, s.get n = .none) : ListR e₁ false ls es vs :=
  match h with
  | .nil => by
    cases es with
    | nil => exact .nil
    | cons e es => simp only [substList] at he; cases he
  | .cons h1 h2 => by
    cases es with
    | nil => simp only [substList] at he; cases he
    | cons e es =>
      simp only [substList] at he
      injection he with he1 he2
      simp only [PlainList, Bool.and_eq_true] at hp
      exact .cons
        (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fvList, hy])) hp.1
          (fun m hm' => hh m (by simp [callHeadsList, hm'])))
        (sbL hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fvList, hy])) hp.2
          (fun m hm' => hh m (by simp [callHeadsList, hm'])))
theorem sbD {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf ts ss} (h : DisplaysR e₂ inFn Lf ts ss) (hf : inFn = false)
    {es : List Expr} {s : Subst} {ls : Locals} (he : substList s es = ts) (hm : MapSmall s)
    (hn : MapNoArrow s) (hA : ∀ y ∈ fvList es, Agree e₁ ls e₂ Lf s y) (hp : PlainList es = true)
    (hh : ∀ n ∈ callHeadsList es, s.get n = .none) : DisplaysR e₁ false ls es ss :=
  match h with
  | .nil => by
    cases es with
    | nil => exact .nil
    | cons e es => simp only [substList] at he; cases he
  | .cons h1 hd h2 => by
    cases es with
    | nil => simp only [substList] at he; cases he
    | cons e es =>
      simp only [substList] at he
      injection he with he1 he2
      simp only [PlainList, Bool.and_eq_true] at hp
      exact .cons
        (sbE hs h1 hf he1 hm hn (fun y hy => hA y (by simp [fvList, hy])) hp.1
          (fun m hm' => hh m (by simp [callHeadsList, hm']))) hd
        (sbD hs h2 hf he2 hm hn (fun y hy => hA y (by simp [fvList, hy])) hp.2
          (fun m hm' => hh m (by simp [callHeadsList, hm'])))
theorem sbM {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf ps' tb xs i ys} (h : MapR e₂ inFn Lf ps' tb xs i ys)
    (hf : inFn = false) {body : Expr} {s : Subst} {ls : Locals} {ps : List String}
    (he : substExpr s body = tb) (hm : MapSmall s) (hn : MapNoArrow s)
    (hA : ∀ x i, ∀ y ∈ fv body, Agree e₁ (bindParams ps x i ls) e₂ (bindParams ps' x i Lf) s y)
    (hp : Plain body = true) (hh : ∀ n ∈ callHeads body, s.get n = .none) :
    MapR e₁ false ls ps body xs i ys :=
  match h with
  | .nil => .nil
  | .cons h1 h2 => .cons (sbE hs h1 hf he hm hn (hA _ _) hp hh) (sbM hs h2 hf he hm hn hA hp hh)
theorem sbF {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf ps' tb xs i ys} (h : FilterR e₂ inFn Lf ps' tb xs i ys)
    (hf : inFn = false) {body : Expr} {s : Subst} {ls : Locals} {ps : List String}
    (he : substExpr s body = tb) (hm : MapSmall s) (hn : MapNoArrow s)
    (hA : ∀ x i, ∀ y ∈ fv body, Agree e₁ (bindParams ps x i ls) e₂ (bindParams ps' x i Lf) s y)
    (hp : Plain body = true) (hh : ∀ n ∈ callHeads body, s.get n = .none) :
    FilterR e₁ false ls ps body xs i ys :=
  match h with
  | .nil => .nil
  | .keep h1 h2 => .keep (sbE hs h1 hf he hm hn (hA _ _) hp hh) (sbF hs h2 hf he hm hn hA hp hh)
  | .drop h1 h2 => .drop (sbE hs h1 hf he hm hn (hA _ _) hp hh) (sbF hs h2 hf he hm hn hA hp hh)
theorem sbR {e₁ e₂ : Env} (hs : Same e₁ e₂) {inFn Lf tw b fs i vs} (h : FieldsR e₂ inFn Lf tw b fs i vs)
    (hf : inFn = false) {w : List (String × Expr)} {s : Subst} {ls : Locals} (he : substFields s w = tw)
    (hm : MapSmall s) (hn : MapNoArrow s) (hA : ∀ y ∈ fvFields w, Agree e₁ ls e₂ Lf s y)
    (hp : PlainFields w = true) (hh : ∀ n ∈ callHeadsFields w, s.get n = .none) :
    FieldsR e₁ false ls w b fs i vs :=
  match h with
  | .nil => .nil
  | .written hl h1 h2 => by
    rw [← he] at hl
    obtain ⟨e0, he0, he1⟩ := lookupField_subst_some hl
    obtain ⟨k, hk⟩ := lookupField_mem he0
    obtain ⟨m1, m2, m3⟩ := fields_mem hk
    exact .written he0 (sbE hs h1 hf he1 hm hn (fun y hy => hA y (fields_fv hk y hy)) (m2 hp) (fun n hn' => hh n (m3 n hn')))
      (sbR hs h2 hf he hm hn hA hp hh)
  | .base hl hb h2 => by
    rw [← he] at hl
    exact .base (lookupField_subst_none hl) hb (sbR hs h2 hf he hm hn hA hp hh)
end

/-- **Substitution commutes with evaluation.** In the fragment, when every
name of `e` reads in the source what its replacement reads in the target,
`e` has value `v` in the source exactly when its substitution has value
`v` in the target. -/
theorem subst_iff {e₁ e₂ : Env} (hs : Same e₁ e₂) {s : Subst} (hm : MapSmall s) (hn : MapNoArrow s)
    {e : Expr} {ls Lf : Locals} (hA : ∀ y ∈ fv e, Agree e₁ ls e₂ Lf s y) (hp : Plain e = true)
    (hh : ∀ n ∈ callHeads e, s.get n = .none) {v : Value} :
    EvalR e₁ false ls e v ↔ EvalR e₂ false Lf (substExpr s e) v :=
  ⟨fun h => sfE hs h rfl hm hA hp hh, fun h => sbE hs h rfl rfl hm hn hA hp hh⟩

end Contract.ExpandSubst
