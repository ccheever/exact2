/-
The compiler of `Contract.Lower` is correct for expressions: by induction
on its fuel, each construct's code meets `ExprSpec` given that its parts'
codes do.
-/
import Contract.LowerSpec

namespace Contract.Lower

open Vm

variable {fuel : Nat} {p : Program} {depth : Nat} {sc : Scope} {n : Nat} {c : Code} {t : STy}
  {env : Contract.Env} {inFn : Bool} {ls : Locals} {venv : Vm.Env} {L : List Value} {P : Code}

/-- A one-instruction expression that pushes `w`, always. -/
theorem spec_push {e i w ty} (hev : ∀ v, EvalR env inFn ls e v ↔ v = w) (hty : VTy env.prog.shapes w ty)
    (hex : ∀ pc S cbs fx, Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (w :: S) L cbs fx))) :
    ExprSpec env inFn ls venv P L e [i] ty := by
  intro pc S cbs fx hr
  refine ⟨fun v hv => ?_, fun _ => ⟨w, (hev w).mpr rfl⟩⟩
  obtain rfl := (hev v).mp hv
  exact ⟨hty, hr.step (hex pc S cbs fx)⟩

theorem case_lit (hc : compile (fuel + 1) p depth sc n e = .ok (c, t)) (_hx : Ctx env inFn ls venv L p sc n)
    (he : e = .num b ∨ e = .str s ∨ e = .bool bb ∨ e = .none ∨ e = .emptyList) :
    ExprSpec env inFn ls venv P L e c t := by
  rcases he with rfl | rfl | rfl | rfl | rfl <;> simp [compile] at hc <;> obtain ⟨rfl, rfl⟩ := hc
  · exact spec_push (w := .num (Float.ofBits b)) (fun v => ⟨fun h => by cases h; rfl, fun h => h ▸ .num⟩)
      (by simp [VTy]) (fun _ _ _ _ => by simp [Vm.exec])
  · exact spec_push (w := .str s) (fun v => ⟨fun h => by cases h; rfl, fun h => h ▸ .str⟩)
      (by simp [VTy]) (fun _ _ _ _ => by simp [Vm.exec])
  · exact spec_push (w := .bool bb) (fun v => ⟨fun h => by cases h; rfl, fun h => h ▸ .bool⟩)
      (by simp [VTy]) (fun _ _ _ _ => by simp [Vm.exec])
  · exact spec_push (w := .none) (fun v => ⟨fun h => by cases h; rfl, fun h => h ▸ .none⟩)
      (by simp [VTy]) (fun _ _ _ _ => by simp [Vm.exec])
  · exact spec_push (w := .list []) (fun v => ⟨fun h => by cases h; rfl, fun h => h ▸ .emptyList⟩)
      (by simp [VTy, VTyAll]) (fun _ _ _ _ => by simp [Vm.exec, popN])

theorem case_some (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.some e) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.some e) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨c0, t0⟩, h0, h1⟩ := hc
  simp at h1; obtain ⟨rfl, rfl⟩ := h1
  have ihe := ih.1 _ _ _ _ _ _ _ h0 _ _ _ _ _ P hx
  intro pc S cbs fx hr
  have ⟨fwd, bwd⟩ := ihe pc S cbs fx hr.left
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | some hw =>
      rename_i w
      obtain ⟨hty, hs⟩ := fwd _ hw
      refine ⟨by simpa [VTy] using hty, hs.trans ?_⟩
      have := hr.right.step (venv := venv) (S := w :: S) (fx := fx) (m' := M (pc + c0.length + 1) (.some w :: S) L cbs fx)
        (by simp [Vm.exec, pop1])
      simpa [Nat.add_assoc] using this
  · obtain ⟨w, hw⟩ := bwd hh
    exact ⟨_, .some hw⟩

/-- Code then one instruction that maps the value it left (`Some`, `Neg`,
`Not`, `Field`): `G` is the semantics' map, partial. -/
theorem spec_post {e e' c0 i t0 t'} {G : Value → Option Value}
    (ihe : ExprSpec env inFn ls venv P L e c0 t0)
    (hsem : ∀ v, EvalR env inFn ls e' v ↔ ∃ w, EvalR env inFn ls e w ∧ G w = some v)
    (hvm : ∀ q S cbs fx w, VTy env.prog.shapes w t0 →
      (∀ v, G w = some v → Vm.exec P.length venv i (M q (w :: S) L cbs fx) = .ok (.run (M (q + 1) (v :: S) L cbs fx))) ∧
      (G w = none → ∀ s, Vm.exec P.length venv i (M q (w :: S) L cbs fx) ≠ .ok s))
    (hty : ∀ w v, VTy env.prog.shapes w t0 → G w = some v → VTy env.prog.shapes v t') :
    ExprSpec env inFn ls venv P L e' (c0 ++ [i]) t' := by
  intro pc S cbs fx hr
  have ⟨fwd, bwd⟩ := ihe pc S cbs fx hr.left
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨w, hw, hg⟩ := (hsem v).mp hv
    obtain ⟨htw, hs⟩ := fwd w hw
    refine ⟨hty w v htw hg, hs.trans ?_⟩
    have := hr.right.step (venv := venv) ((hvm _ S cbs fx w htw).1 v hg)
    simpa [Nat.add_assoc] using this
  · obtain ⟨w, hw⟩ := bwd hh
    obtain ⟨htw, hs⟩ := fwd w hw
    obtain ⟨st, hst⟩ := hr.right.halts (hh.star hs)
    cases hg : G w with
    | none => exact absurd hst ((hvm _ S cbs fx w htw).2 hg st)
    | some v => exact ⟨v, (hsem v).mpr ⟨w, hw, hg⟩⟩

/-- Two codes then one instruction that combines what they left. -/
theorem spec_post2 {e a b ca cb i ta tb t'} {G : Value → Value → Option Value}
    (iha : ExprSpec env inFn ls venv P L a ca ta) (ihb : ExprSpec env inFn ls venv P L b cb tb)
    (hsem : ∀ v, EvalR env inFn ls e v ↔ ∃ va vb, EvalR env inFn ls a va ∧ EvalR env inFn ls b vb ∧ G va vb = some v)
    (hvm : ∀ q S cbs fx va vb, VTy env.prog.shapes va ta → VTy env.prog.shapes vb tb →
      (∀ v, G va vb = some v →
        Vm.exec P.length venv i (M q (vb :: va :: S) L cbs fx) = .ok (.run (M (q + 1) (v :: S) L cbs fx))) ∧
      (G va vb = none → ∀ s, Vm.exec P.length venv i (M q (vb :: va :: S) L cbs fx) ≠ .ok s))
    (hty : ∀ va vb v, VTy env.prog.shapes va ta → VTy env.prog.shapes vb tb → G va vb = some v →
      VTy env.prog.shapes v t') :
    ExprSpec env inFn ls venv P L e (ca ++ cb ++ [i]) t' := by
  intro pc S cbs fx hr
  have hra := hr.left.left
  have hrb := hr.left.right
  have hri : Room P [i] (pc + ca.length + cb.length) cbs L := by simpa [Nat.add_assoc] using hr.right
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨va, vb, hva, hvb, hg⟩ := (hsem v).mp hv
    obtain ⟨hta, hsa⟩ := (iha pc S cbs fx hra).1 va hva
    obtain ⟨htb, hsb⟩ := (ihb _ (va :: S) cbs fx hrb).1 vb hvb
    refine ⟨hty va vb v hta htb hg, hsa.trans (hsb.trans ?_)⟩
    have := hri.step (venv := venv) ((hvm _ S cbs fx va vb hta htb).1 v hg)
    simpa [Nat.add_assoc] using this
  · obtain ⟨va, hva⟩ := (iha pc S cbs fx hra).2 hh
    obtain ⟨hta, hsa⟩ := (iha pc S cbs fx hra).1 va hva
    have hh1 := hh.star hsa
    obtain ⟨vb, hvb⟩ := (ihb _ (va :: S) cbs fx hrb).2 hh1
    obtain ⟨htb, hsb⟩ := (ihb _ (va :: S) cbs fx hrb).1 vb hvb
    obtain ⟨st, hst⟩ := hri.halts (hh1.star hsb)
    cases hg : G va vb with
    | none => exact absurd hst ((hvm _ S cbs fx va vb hta htb).2 hg st)
    | some v => exact ⟨v, (hsem v).mpr ⟨va, vb, hva, hvb, hg⟩⟩

/-- A one-instruction read of a cell that may be empty. -/
theorem spec_cell {e i ty} {o : Option (Option Value)}
    (hev : ∀ v, EvalR env inFn ls e v ↔ o = some (some v)) (hty : ∀ v, o = some (some v) → VTy env.prog.shapes v ty)
    (hok : ∀ pc S cbs fx w, o = some (some w) →
      Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (w :: S) L cbs fx)))
    (hno : ∀ pc S cbs fx, (∀ w, o ≠ some (some w)) → ∀ s, Vm.exec P.length venv i (M pc S L cbs fx) ≠ .ok s) :
    ExprSpec env inFn ls venv P L e [i] ty := by
  intro pc S cbs fx hr
  refine ⟨fun v hv => ⟨hty v ((hev v).mp hv), hr.step (hok pc S cbs fx v ((hev v).mp hv))⟩, fun hh => ?_⟩
  obtain ⟨st, hst⟩ := hr.halts hh
  by_cases hw : ∃ w, o = some (some w)
  · obtain ⟨w, hw⟩ := hw; exact ⟨w, (hev w).mpr hw⟩
  · exact absurd hst (hno pc S cbs fx (fun w h => hw ⟨w, h⟩) st)

theorem case_var (hc : compile (fuel + 1) p depth sc n (.var x) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.var x) c t := by
  simp only [compile] at hc
  split at hc
  · rename_i r t0 hl
    simp at hc; obtain ⟨rfl, rfl⟩ := hc
    have ho := hx.agree x r t0 hl
    have local_like : ∀ {w : Value} {i : Instr}, lookup x ls = some w → VTy env.prog.shapes w t0 →
        (∀ pc S cbs fx, Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (w :: S) L cbs fx))) →
        ExprSpec env inFn ls venv P L (.var x) [i] t0 := fun hl' hty hex =>
      spec_push (fun v => by rw [evalR_var]; simp [hl']; exact eq_comm) hty hex
    have global_like : ∀ {o : Option (Option Value)} {i : Instr}, lookup x ls = none → inFn = false →
        GlobalOk env.prog.shapes (env.global x) o t0 →
        (∀ pc S cbs fx w, o = some (some w) →
          Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (w :: S) L cbs fx))) →
        (∀ pc S cbs fx, (∀ w, o ≠ some (some w)) → ∀ s, Vm.exec P.length venv i (M pc S L cbs fx) ≠ .ok s) →
        ExprSpec env inFn ls venv P L (.var x) [i] t0 := fun hl' hin hg hok hno =>
      spec_cell (fun v => by rw [evalR_var]; simp [hl', hin]; exact hg.1 v) hg.2 hok hno
    cases r <;> simp only [RefOk] at ho <;> simp only [refInstr]
    case slot j =>
      obtain ⟨hl', hin, hg⟩ := ho
      exact global_like hl' hin hg (fun _ _ _ _ w h => by simp [Vm.exec, h])
        (fun _ _ _ _ h s => by
          simp only [Vm.exec]; split <;> simp_all)
    case derive j =>
      obtain ⟨hl', hin, hg⟩ := ho
      exact global_like hl' hin hg (fun _ _ _ _ w h => by simp [Vm.exec, h])
        (fun _ _ _ _ h s => by
          simp only [Vm.exec]; split <;> simp_all)
    case resource j =>
      obtain ⟨hl', hin, hg, _⟩ := ho
      exact global_like hl' hin hg (fun _ _ _ _ w h => by simp [Vm.exec, h])
        (fun _ _ _ _ h s => by
          simp only [Vm.exec]; split <;> simp_all)
    case mutation s k =>
      obtain ⟨hl', hin, hg, _⟩ := ho
      exact global_like hl' hin hg (fun _ _ _ _ w h => by simp [Vm.exec, h])
        (fun _ _ _ _ h s => by
          simp only [Vm.exec]; split <;> simp_all)
    case «local» j =>
      obtain ⟨w, hl', hL, hty⟩ := ho
      exact local_like hl' hty (fun _ _ _ _ => by simp [Vm.exec, hL])
    case param j =>
      obtain ⟨w, hl', hL, hty⟩ := ho
      exact local_like hl' hty (fun _ _ _ _ => by simp [Vm.exec, hL])
    case item d =>
      obtain ⟨w, hl', hL, hty⟩ := ho
      exact local_like hl' hty (fun _ _ _ _ => by simp [Vm.exec, hL])
    case bound d =>
      obtain ⟨w, hl', hL, hty⟩ := ho
      exact local_like hl' hty (fun _ _ _ _ => by simp [Vm.exec, hL])
    case index d =>
      obtain ⟨k, hl', hL, hty⟩ := ho
      exact local_like hl' hty (fun _ _ _ _ => by simp [Vm.exec, hL])
  · cases hc

/-- The parts' specifications, from the induction hypothesis. -/
theorem ih_expr (ih : AllOk fuel) {e c t} (h : compile fuel p depth sc n e = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L e c t :=
  ih.1 _ _ _ _ _ _ _ h _ _ _ _ _ P hx

theorem case_neg (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.unary .neg e) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.unary .neg e) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨c0, t0⟩, h0, h1⟩ := hc
  simp at h1; obtain ⟨rfl, rfl⟩ := h1
  refine spec_post (G := fun w => match w with | .num x => some (.num (-x)) | _ => none)
    (ih_expr ih h0 hx) (fun v => ⟨fun h => ?_, fun ⟨w, hw, hg⟩ => ?_⟩) (fun q S cbs fx w _ => ?_)
    (fun w v _ hg => ?_)
  · cases h; exact ⟨_, by assumption, rfl⟩
  · cases w <;> simp at hg; subst hg; exact .neg hw
  · cases w <;> simp [Vm.exec, pop1]
  · cases w <;> simp at hg; subst hg; simp [VTy]

theorem case_not (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.unary .not e) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.unary .not e) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨c0, t0⟩, h0, h1⟩ := hc
  simp at h1; obtain ⟨rfl, rfl⟩ := h1
  refine spec_post (G := fun w => match w with | .bool b => some (.bool !b) | _ => none)
    (ih_expr ih h0 hx) (fun v => ⟨fun h => ?_, fun ⟨w, hw, hg⟩ => ?_⟩) (fun q S cbs fx w _ => ?_)
    (fun w v _ hg => ?_)
  · cases h; exact ⟨_, by assumption, rfl⟩
  · cases w <;> simp at hg; subst hg; exact .not hw
  · cases w <;> simp [Vm.exec, pop1]
  · cases w <;> simp at hg; subst hg; simp [VTy]

theorem case_member (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.member e f) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.member e f) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨c0, t0⟩, h0, h1⟩ := hc
  cases t0
  case record s =>
    simp only at h1
    split at h1
    case h_2 => cases h1
    rename_i sh hsh
    split at h1
    case h_2 => cases h1
    rename_i i hi
    split at h1
    case h_2 => cases h1
    rename_i fd hfd
    simp at h1; obtain ⟨rfl, rfl⟩ := h1
    have hp := hx.prog
    have hfi : env.fieldIndex s f = some i := by
      simp [Env.fieldIndex, Env.shape, hp, hsh, hi]
    refine spec_post (G := fun w => match w with
        | .record s' fs => (env.fieldIndex s' f).bind (fs[·]?) | _ => none)
      (ih_expr ih h0 hx) (fun v => ⟨fun h => ?_, fun ⟨w, hw, hg⟩ => ?_⟩) (fun q S cbs fx w hw => ?_)
      (fun w v hw hg => ?_)
    · cases h with
      | member h1 h2 h3 => exact ⟨_, h1, by simp [h2, h3]⟩
    · cases w <;> simp only [reduceCtorEq] at hg
      rw [Option.bind_eq_some_iff] at hg
      obtain ⟨j, hj, hv⟩ := hg
      exact .member hw hj hv
    · cases w <;> simp [VTy] at hw
      rename_i s' fs
      obtain ⟨rfl, _⟩ := hw
      cases hfs : fs[i]? <;> simp [Vm.exec, pop1, hfi, hfs]
    · cases w <;> simp [VTy] at hw
      rename_i s' fs
      obtain ⟨rfl, d, hd, hts⟩ := hw
      simp only [hfi, Option.bind_some] at hg
      rw [hp, hsh] at hd; cases hd
      exact VTys.get hts hg (by simp [hfd])
  all_goals simp at h1

/-- The opcode of a strict operator, on its operands. -/
theorem exec_binary {op ta i t} {P : Code} {venv q va vb S L cbs fx} (hb : binInstr op ta = .ok (i, t)) :
    Vm.exec P.length venv i (M q (vb :: va :: S) L cbs fx) =
      match binary i va vb with
      | .ok v => .ok (.run (M (q + 1) (v :: S) L cbs fx))
      | .error e => .error e := by
  cases op <;> simp [binInstr] at hb
  case add =>
    split at hb
    · simp at hb; obtain ⟨rfl, rfl⟩ := hb
      cases h : binary .concat va vb <;> simp [Vm.exec, pop2, h, bind, Except.bind]
    · split at hb
      · simp at hb; obtain ⟨rfl, rfl⟩ := hb
        cases h : binary .add va vb <;> simp [Vm.exec, pop2, h, bind, Except.bind]
      · cases hb
  all_goals (obtain ⟨rfl, rfl⟩ := hb; cases h : binary _ va vb <;> simp [Vm.exec, pop2, h, bind, Except.bind])

theorem case_binary (ih : AllOk fuel) (hop : op ≠ .and ∧ op ≠ .or)
    (hc : compile (fuel + 1) p depth sc n (.binary op a b) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.binary op a b) c t := by
  have hc' : (do
      let (ca, ta) ← compile fuel p depth sc n a
      let (cb, _) ← compile fuel p depth sc n b
      let (i, t) ← binInstr op ta
      Except.ok (ca ++ cb ++ [i], t) : Except String (Code × STy)) = .ok (c, t) := by
    cases op <;> simp_all [compile]
  simp only [Except.bind_ok_iff] at hc'
  obtain ⟨⟨ca, ta⟩, ha, ⟨cb, tb⟩, hb, ⟨i, t'⟩, hi, h1⟩ := hc'
  simp at h1; obtain ⟨rfl, rfl⟩ := h1
  rw [← List.append_assoc]
  refine spec_post2 (G := fun va vb => (binop op va vb).toOption) (ih_expr ih ha hx) (ih_expr ih hb hx)
    (fun v => ⟨fun h => ?_, fun ⟨va, vb, h1, h2, hg⟩ => ?_⟩) (fun q S cbs fx va vb hva _ => ?_)
    (fun va vb v hva _ hg => ?_)
  · cases h
    case andFalse | andTrue | orTrue | orFalse => simp at hop
    case binop h1 h2 h3 => exact ⟨_, _, h1, h2, by simp [h3, Except.toOption]⟩
  · refine .binop h1 h2 ?_
    cases hv : binop op va vb <;> simp [hv, Except.toOption] at hg; rw [hg]
  · have hbo := binary_ok (vb := vb) hi hva
    rw [exec_binary hi]
    constructor
    · intro v hg
      have : binop op va vb = .ok v := by
        cases hv : binop op va vb <;> simp [hv, Except.toOption] at hg; rw [hg]
      rw [hbo.2 v this]
    · intro hg s hs
      cases hv : binary i va vb with
      | error e => rw [hv] at hs; cases hs
      | ok v =>
        have := (hbo.1 v hv).1
        simp [this, Except.toOption] at hg
  · have : binop op va vb = .ok v := by
      cases hv : binop op va vb <;> simp [hv, Except.toOption] at hg; rw [hg]
    cases hv : binary i va vb with
    | error e => rw [(binary_ok (vb := vb) hi hva).2 v this] at hv; cases hv
    | ok w =>
      have h2 := (binary_ok (vb := vb) hi hva).1 w hv
      have : w = v := by have := h2.1.symm.trans this; cases this; rfl
      subst this
      exact h2.2

theorem case_typed (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.typed e ty) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.typed e ty) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨c0, t0⟩, h0, h1⟩ := hc
  simp at h1; obtain ⟨rfl, rfl⟩ := h1
  intro pc S cbs fx hr
  have ⟨fwd, bwd⟩ := ih_expr ih h0 hx pc S cbs fx hr
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | typed h => obtain ⟨h1, h2⟩ := fwd _ h; exact ⟨vty_refine h1, h2⟩
  · obtain ⟨v, hv⟩ := bwd hh; exact ⟨v, .typed hv⟩

theorem case_let (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.letE x e body) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.letE x e body) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨cv, tv⟩, hv0, ⟨cb, tb⟩, hb0, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  intro pc S cbs fx hr
  have hr' : Room P (cv ++ (.bindLocal :: (cb ++ [.dropLocal]))) pc cbs L := by simpa using hr
  have hrv := hr'.left
  have hrr := hr'.right
  have hrb := hrr.tail.left
  have hrd := hrr.tail.right
  have hl := hr.locals
  have ihv := ih_expr (P := P) ih hv0 hx
  have body_at : ∀ w, VTy env.prog.shapes w tv →
      ExprSpec env inFn ((x, w) :: ls) venv P (L ++ [w]) body cb tb :=
    fun w hw => ih_expr ih hb0 (hx.push hw)
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | letE h1 h2 =>
      rename_i w
      obtain ⟨htw, hs1⟩ := (ihv pc S cbs fx hrv).1 w h1
      obtain ⟨htv, hs2⟩ := (body_at w htw _ S cbs fx (hrb.locals_append [w])).1 v h2
      refine ⟨htv, hs1.trans ((hrr.step exec_bindLocal).trans (hs2.trans ?_))⟩
      have := (hrd.locals_append [w]).step (venv := venv) (S := v :: S) (fx := fx) (exec_drop hl)
      refine Star.pc this ?_
      simp; omega
  · obtain ⟨w, h1⟩ := (ihv pc S cbs fx hrv).2 hh
    obtain ⟨htw, hs1⟩ := (ihv pc S cbs fx hrv).1 w h1
    have hh2 := (hh.star hs1).star (hrr.step exec_bindLocal)
    obtain ⟨v, h2⟩ := (body_at w htw _ S cbs fx (hrb.locals_append [w])).2 hh2
    exact ⟨v, .letE h1 h2⟩

theorem case_ternary (ih : AllOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.ternary e a b) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.ternary e a b) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨cc, tc⟩, hc0, ⟨ca, ta⟩, ha0, ⟨cb, tb⟩, hb0, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  intro pc S cbs fx hr
  have hr' : Room P (cc ++ (.jumpIfFalse (ca.length + 1) :: (ca ++ (.jump cb.length :: cb)))) pc cbs L := by
    simpa using hr
  have hrc := hr'.left
  have hrj := hr'.right
  have hra := hrj.tail.left
  have hrk := hrj.tail.right
  have hrb := hrk.tail
  have ihc := ih_expr (P := P) ih hc0 hx pc S cbs fx hrc
  have iha := ih_expr (P := P) ih ha0 hx (pc + cc.length + 1) S cbs fx hra
  have ihb := ih_expr (P := P) ih hb0 hx (pc + cc.length + 1 + (ca.length + 1)) S cbs fx hrb
  have jf : Vm.exec P.length venv (.jumpIfFalse (ca.length + 1)) (M (pc + cc.length) (.bool false :: S) L cbs fx) =
      .ok (.run (M (pc + cc.length + 1 + (ca.length + 1)) S L cbs fx)) :=
    exec_jumpIfFalse_false hr.top hr.fits (by simp; omega)
  have jk : ∀ u, Vm.exec P.length venv (.jump cb.length) (M (pc + cc.length + 1 + ca.length) (u :: S) L cbs fx) =
      .ok (.run (M (pc + cc.length + 1 + ca.length + 1 + cb.length) (u :: S) L cbs fx)) :=
    fun u => exec_jump hr.top hr.fits (by simp; omega)
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | ternaryTrue h1 h2 =>
      obtain ⟨_, hs1⟩ := ihc.1 _ h1
      obtain ⟨htv, hs2⟩ := iha.1 v h2
      refine ⟨vty_join_left htv, hs1.trans ((hrj.step exec_jumpIfFalse_true).trans (hs2.trans ?_))⟩
      exact Star.pc (hrk.step (jk v)) (by simp; omega)
    | ternaryFalse h1 h2 =>
      obtain ⟨_, hs1⟩ := ihc.1 _ h1
      obtain ⟨htv, hs2⟩ := ihb.1 v h2
      refine ⟨vty_join_right htv, hs1.trans ((hrj.step jf).trans (Star.pc hs2 ?_))⟩
      simp; omega
  · obtain ⟨w, h1⟩ := ihc.2 hh
    obtain ⟨_, hs1⟩ := ihc.1 w h1
    have hh1 := hh.star hs1
    obtain ⟨st, hst⟩ := hrj.halts hh1
    obtain ⟨bv, rfl⟩ := exec_jumpIfFalse_bool hst
    cases bv
    · obtain ⟨v, h2⟩ := ihb.2 (hh1.star (hrj.step jf))
      exact ⟨v, .ternaryFalse h1 h2⟩
    · obtain ⟨v, h2⟩ := iha.2 (hh1.star (hrj.step exec_jumpIfFalse_true))
      exact ⟨v, .ternaryTrue h1 h2⟩

theorem getElem_last {L : List Value} {w : Value} : (L ++ [w])[L.length]? = some w := by simp

theorem case_and (ih : AllOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.binary .and a b) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.binary .and a b) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ta⟩, ha0, ⟨cb, tb⟩, hb0, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  obtain rfl := hx.len
  intro pc S cbs fx hr
  have hl := hr.locals
  have r0 := hr.left.left.right
  have r4 := hr.right
  have iha := ih_expr (P := P) ih ha0 hx pc S cbs fx hr.left.left.left
  have ihb := ih_expr (P := P) ih hb0 hx _ S cbs fx hr.left.right
  have pre := fun w => (r0.step (venv := venv) (S := w :: S) (fx := fx) exec_bindLocal).trans
    ((r0.tail.locals_append [w]).step (venv := venv) (fx := fx) (exec_loadLocal getElem_last))
  have jt := (r0.tail.tail.locals_append [.bool true]).step (venv := venv) (S := .bool true :: S) (fx := fx)
    exec_jumpIfFalse_true
  have jf := (r0.tail.tail.locals_append [.bool false]).step (venv := venv) (S := .bool false :: S) (fx := fx)
    (exec_jumpIfFalse_false hr.top hr.fits (by simp; omega))
  have d1 := (r0.tail.tail.tail.locals_append [.bool true]).step (venv := venv) (S := S) (fx := fx)
    (exec_drop hl)
  have jm := fun v => r4.step (venv := venv) (S := v :: S) (fx := fx)
    (exec_jump hr.top hr.fits (by simp; omega))
  have l2 := (r4.tail.locals_append [.bool false]).step (venv := venv) (S := S) (fx := fx)
    (exec_loadLocal getElem_last)
  have d2 := (r4.tail.tail.locals_append [.bool false]).step (venv := venv) (S := .bool false :: S) (fx := fx)
    (exec_drop hl)
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | andFalse h1 =>
      obtain ⟨_, hs1⟩ := iha.1 _ h1
      refine ⟨vty_join_left (by simp [VTy]), hs1.trans ((pre _).trans (jf.join (l2.join (d2.join .refl ?_) ?_) ?_))⟩
      all_goals (simp <;> omega)
    | andTrue h1 h2 =>
      obtain ⟨_, hs1⟩ := iha.1 _ h1
      obtain ⟨htv, hs2⟩ := ihb.1 v h2
      refine ⟨vty_join_right htv, hs1.trans ((pre _).trans (jt.trans (d1.join (hs2.join ((jm v).join .refl ?_) ?_) ?_)))⟩
      all_goals (simp <;> omega)
    | binop _ _ h3 => simp [binop] at h3
  · obtain ⟨w, h1⟩ := iha.2 hh
    obtain ⟨_, hs1⟩ := iha.1 w h1
    have hh1 := (hh.star hs1).star (pre w)
    obtain ⟨st, hst⟩ := (r0.tail.tail.locals_append [w]).halts hh1
    obtain ⟨bv, rfl⟩ := exec_jumpIfFalse_bool hst
    cases bv
    · exact ⟨_, .andFalse h1⟩
    · have hh2 := (hh1.star jt).star d1
      obtain ⟨v, h2⟩ := ihb.2 (by simpa [Nat.add_assoc] using hh2)
      exact ⟨v, .andTrue h1 h2⟩

theorem case_or (ih : AllOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.binary .or a b) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.binary .or a b) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ta⟩, ha0, ⟨cb, tb⟩, hb0, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  obtain rfl := hx.len
  intro pc S cbs fx hr
  have hl := hr.locals
  have r0 := hr.left.left.right
  have r4 := hr.right
  have iha := ih_expr (P := P) ih ha0 hx pc S cbs fx hr.left.left.left
  have ihb := ih_expr (P := P) ih hb0 hx _ S cbs fx hr.left.right
  have pre := fun w => (r0.step (venv := venv) (S := w :: S) (fx := fx) exec_bindLocal).trans
    ((r0.tail.locals_append [w]).step (venv := venv) (fx := fx) (exec_loadLocal getElem_last))
  have nt := fun bv => (r0.tail.tail.locals_append [.bool bv]).step (venv := venv) (S := .bool bv :: S) (fx := fx)
    (exec_not (b := bv))
  have jt := (r0.tail.tail.tail.locals_append [.bool false]).step (venv := venv) (S := .bool true :: S) (fx := fx)
    exec_jumpIfFalse_true
  have jf := (r0.tail.tail.tail.locals_append [.bool true]).step (venv := venv) (S := .bool false :: S) (fx := fx)
    (exec_jumpIfFalse_false hr.top hr.fits (by simp; omega))
  have d1 := (r0.tail.tail.tail.tail.locals_append [.bool false]).step (venv := venv) (S := S) (fx := fx)
    (exec_drop hl)
  have jm := fun v => r4.step (venv := venv) (S := v :: S) (fx := fx)
    (exec_jump hr.top hr.fits (by simp; omega))
  have l2 := (r4.tail.locals_append [.bool true]).step (venv := venv) (S := S) (fx := fx)
    (exec_loadLocal getElem_last)
  have d2 := (r4.tail.tail.locals_append [.bool true]).step (venv := venv) (S := .bool true :: S) (fx := fx)
    (exec_drop hl)
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | orTrue h1 =>
      obtain ⟨_, hs1⟩ := iha.1 _ h1
      refine ⟨vty_join_left (by simp [VTy]),
        hs1.trans ((pre _).trans ((nt true).trans (jf.join (l2.join (d2.join .refl ?_) ?_) ?_)))⟩
      all_goals (simp <;> omega)
    | orFalse h1 h2 =>
      obtain ⟨_, hs1⟩ := iha.1 _ h1
      obtain ⟨htv, hs2⟩ := ihb.1 v h2
      refine ⟨vty_join_right htv,
        hs1.trans ((pre _).trans ((nt false).trans (jt.trans (d1.join (hs2.join ((jm v).join .refl ?_) ?_) ?_))))⟩
      all_goals (simp <;> omega)
    | binop _ _ h3 => simp [binop] at h3
  · obtain ⟨w, h1⟩ := iha.2 hh
    obtain ⟨_, hs1⟩ := iha.1 w h1
    have hh1 := (hh.star hs1).star (pre w)
    obtain ⟨st, hst⟩ := (r0.tail.tail.locals_append [w]).halts hh1
    obtain ⟨bv, rfl⟩ := exec_not_bool hst
    cases bv
    · have hh2 := ((hh1.star (nt false)).star jt).star d1
      obtain ⟨v, h2⟩ := ihb.2 (by simpa [Nat.add_assoc] using hh2)
      exact ⟨v, .orFalse h1 h2⟩
    · exact ⟨_, .orTrue h1⟩

/-- The type a `match` binds. -/
theorem vty_inner {sh w ts} (h : VTy sh (.some w) ts) :
    VTy sh w ts.inner := by
  cases ts <;> simp [VTy, STy.inner] at h ⊢ <;> first | exact h | exact VTy.top'

theorem case_match (ih : AllOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.matchOpt s x a b) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.matchOpt s x a b) c t := by
  simp only [compile, Except.bind_ok_iff] at hc
  obtain ⟨⟨cs, ts⟩, hs0, ⟨ca, ta⟩, ha0, ⟨cb, tb⟩, hb0, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  have hn := hx.len
  intro pc S cbs fx hr
  have hl := hr.locals
  have r0 := hr.left.left.left.right
  have r1 := hr.left.right
  have ihs := ih_expr (P := P) ih hs0 hx pc S cbs fx hr.left.left.left.left
  have iha := fun w (hw : VTy env.prog.shapes w ts.inner) =>
    ih_expr (P := P) ih ha0 (hx.push (x := x) hw) _ S cbs fx (hr.left.left.right.locals_append [w])
  have ihb := ih_expr (P := P) ih hb0 hx _ S cbs fx hr.right
  have js := fun w => r0.step (venv := venv) (S := .some w :: S) (fx := fx) (exec_jumpIfNone_some (w := w))
  have jn := r0.step (venv := venv) (S := .none :: S) (fx := fx) (exec_jumpIfNone_none hr.top hr.fits (by simp; omega))
  have uw := fun w => r0.tail.step (venv := venv) (S := .some w :: S) (fx := fx) (exec_unwrap (w := w))
  have bl := fun w => r0.tail.tail.step (venv := venv) (S := w :: S) (fx := fx) exec_bindLocal
  have dl := fun w v => (r1.locals_append [w]).step (venv := venv) (S := v :: S) (fx := fx) (exec_drop hl)
  have jm := fun v => r1.tail.step (venv := venv) (S := v :: S) (fx := fx)
    (exec_jump hr.top hr.fits (by simp; omega))
  have pp := r1.tail.tail.step (venv := venv) (S := .none :: S) (fx := fx) (exec_pop (w := .none))
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv with
    | matchSome h1 h2 =>
      rename_i w
      obtain ⟨htw, hs1⟩ := ihs.1 _ h1
      have hti := vty_inner htw
      obtain ⟨htv, hs2⟩ := (iha w hti).1 v (by subst hn; exact h2)
      refine ⟨vty_join_left htv, hs1.trans ((js w).trans ((uw w).trans ((bl w).join
        (hs2.join ((dl w v).join ((jm v).join .refl ?_) ?_) ?_) ?_)))⟩
      all_goals (simp <;> omega)
    | matchNone h1 h2 =>
      obtain ⟨_, hs1⟩ := ihs.1 _ h1
      obtain ⟨htv, hs2⟩ := ihb.1 v h2
      refine ⟨vty_join_right htv, hs1.trans (jn.join (pp.join (hs2.join .refl ?_) ?_) ?_)⟩
      all_goals (simp <;> omega)
  · obtain ⟨w, h1⟩ := ihs.2 hh
    obtain ⟨htw, hs1⟩ := ihs.1 w h1
    have hh1 := hh.star hs1
    obtain ⟨st, hst⟩ := r0.halts hh1
    rcases exec_jumpIfNone_opt hst with rfl | ⟨u, rfl⟩
    · have hh2 := ((hh1.star jn).pc (by simp; omega)).star pp
      obtain ⟨v, h2⟩ := ihb.2 (hh2.pc (by simp; omega))
      exact ⟨v, .matchNone h1 h2⟩
    · have hh2 := ((hh1.star (js u)).star (uw u)).star (bl u)
      obtain ⟨v, h2⟩ := (iha u (vty_inner htw)).2 (hh2.pc (by simp; omega))
      exact ⟨v, .matchSome h1 (by subst hn; exact h2)⟩

end Contract.Lower
