/-
The compiler is correct for action bodies: a compiled block, run on the VM
in a corresponding state, asks for exactly the effects `exec` records (with
names resolved as the plan did, `lowerFx`), and runs to its end exactly when
`exec` has an outcome.
-/
import Contract.LowerCorrect

namespace Contract.Lower

open Vm

/-- What a body may write, as the VM holds it: every slot a name resolves
to is in the action's `writes`, owned exactly when the name is a row
slot, and holds an instance exactly when the semantics has the row; every
mutation's slot is writable. -/
structure Writes (env : Contract.Env) (venv : Vm.Env) (Lay : Layout) : Prop where
  slot : ∀ t j, slotOf Lay t = some j → venv.writable.contains j = true ∧
    (isRootState env.prog t = true → venv.owned[j]? = some false) ∧
    (isRootState env.prog t = false → venv.owned[j]? = some true ∧ isRowState env.prog t = true ∧
      ((lookup t env.rows).isSome = true ↔ ∃ v, venv.slots[j]? = some (some v)))
  send : ∀ t s k, find? t Lay.mutations = some (s, k) →
    ∃ s', venv.mutationSlots[k]? = some s' ∧ venv.writable.contains s' = true

/-- The code of a block: from the effects so far, those `exec` adds. -/
def BlockSpec (env : Contract.Env) (ls : Locals) (venv : Vm.Env) (P : Code) (L : List Value)
    (Lay : Layout) (ss : List Stmt) (c : Code) : Prop :=
  ∀ pc S cbs fx0, Room P c pc cbs L →
    (∀ fx1, ExecR env ls ss fx0 fx1 →
      Star P venv (M pc S L cbs (lowerFx Lay fx0)) (M (pc + c.length) S L cbs (lowerFx Lay fx1))) ∧
    (Halts P venv (M pc S L cbs (lowerFx Lay fx0)) → ∃ fx1, ExecR env ls ss fx0 fx1)

def BlockOk (fuel : Nat) : Prop :=
  ∀ p Lay sc n ss c, compileBlock fuel p Lay sc n ss = .ok c →
  ∀ env ls venv L P, Ctx env false ls venv L p sc n → Writes env venv Lay →
  BlockSpec env ls venv P L Lay ss c

def StmtOk (fuel : Nat) : Prop :=
  ∀ p Lay sc n s c, compileStmt fuel p Lay sc n s = .ok c →
  ∀ env ls venv L P, Ctx env false ls venv L p sc n → Writes env venv Lay →
  BlockSpec env ls venv P L Lay [s] c

/-- A statement other than `let`, then the rest of its block. -/
theorem execR_cons {env ls s rest fx0 fx2} (hs : ∀ x e, s ≠ .letS x e) :
    ExecR env ls (s :: rest) fx0 fx2 ↔ ∃ fx1, ExecR env ls [s] fx0 fx1 ∧ ExecR env ls rest fx1 fx2 := by
  constructor
  · intro h
    match h with
    | .letS _ _ => exact absurd rfl (hs _ _)
    | .assignRoot h1 h2 h3 => exact ⟨_, .assignRoot h1 h2 .nil, h3⟩
    | .assignRow h1 h2 h3 h4 h5 => exact ⟨_, .assignRow h1 h2 h3 h4 .nil, h5⟩
    | .command h1 h2 => exact ⟨_, .command h1 .nil, h2⟩
    | .send h1 h2 => exact ⟨_, .send h1 .nil, h2⟩
    | .refresh h1 => exact ⟨_, .refresh .nil, h1⟩
    | .ifTrue h1 h2 h3 => exact ⟨_, .ifTrue h1 h2 .nil, h3⟩
    | .ifFalse h1 h2 h3 => exact ⟨_, .ifFalse h1 h2 .nil, h3⟩
    | .matchSome h1 h2 h3 => exact ⟨_, .matchSome h1 h2 .nil, h3⟩
    | .matchNone h1 h2 h3 => exact ⟨_, .matchNone h1 h2 .nil, h3⟩
  · rintro ⟨fx1, hs1, hr⟩
    match hs1 with
    | .letS _ _ => exact absurd rfl (hs _ _)
    | .assignRoot a b .nil => exact .assignRoot a b hr
    | .assignRow a b c d .nil => exact .assignRow a b c d hr
    | .command a .nil => exact .command a hr
    | .send a .nil => exact .send a hr
    | .refresh .nil => exact .refresh hr
    | .ifTrue a b .nil => exact .ifTrue a b hr
    | .ifFalse a b .nil => exact .ifFalse a b hr
    | .matchSome a b .nil => exact .matchSome a b hr
    | .matchNone a b .nil => exact .matchNone a b hr

variable {fuel : Nat} {p : Program} {Lay : Layout} {sc : Scope} {n : Nat} {c : Code}
  {env : Contract.Env} {ls : Locals} {venv : Vm.Env} {L : List Value} {P : Code}

theorem ih_e {e c t} (h : compile fuel p 0 sc n e = .ok (c, t)) (hx : Ctx env false ls venv L p sc n) :
    ExprSpec env false ls venv P L e c t := (allOk fuel).1 _ _ _ _ _ _ _ h _ _ _ _ _ P hx

theorem ih_args {es c ts} (h : compileArgs fuel p 0 sc n es = .ok (c, ts)) (hx : Ctx env false ls venv L p sc n) :
    ArgsSpec env false ls venv P L es c ts := (allOk fuel).2.1 _ _ _ _ _ _ _ h _ _ _ _ _ P hx

theorem stmt_assign (hc : compileStmt (fuel + 1) p Lay sc n (.assign t e) = .ok c)
    (hx : Ctx env false ls venv L p sc n) (hw : Writes env venv Lay) :
    BlockSpec env ls venv P L Lay [.assign t e] c := by
  simp only [compileStmt, Except.bind_ok_iff] at hc
  obtain ⟨⟨ce, te⟩, he, h1⟩ := hc
  simp only at h1
  split at h1
  · rename_i j hj
    simp only [Except.ok.injEq] at h1; subst h1
    obtain ⟨hwr, hroot, hrow⟩ := hw.slot t j hj
    intro pc S cbs fx0 hr
    have ihe := ih_e (P := P) he hx pc S cbs (lowerFx Lay fx0) hr.left
    have hstore : ∀ v, Vm.exec P.length venv (.storeSlot j) (M (pc + ce.length) (v :: S) L cbs (lowerFx Lay fx0)) =
        if isRootState env.prog t = true then .ok (.run (M (pc + ce.length + 1) S L cbs (lowerFx Lay (fx0.write t v))))
        else if (lookup t env.rows).isSome = true then
          .ok (.run (M (pc + ce.length + 1) S L cbs (lowerFx Lay (fx0.rowWrite t v))))
        else .error .badScope := by
      intro v
      have hwr' : j ∈ venv.writable := by simpa using hwr
      by_cases hr0 : isRootState env.prog t = true
      · simp [Vm.exec, hwr, hwr', pop1, hroot hr0, hr0, lowerFx, Effects.write, hj]
      · simp only [Bool.not_eq_true] at hr0
        obtain ⟨ho, _, hiff⟩ := hrow hr0
        by_cases hrw : (lookup t env.rows).isSome = true
        · obtain ⟨w, hw'⟩ := hiff.mp hrw
          simp [Vm.exec, hwr, hwr', pop1, ho, hr0, hrw, hw', lowerFx, Effects.rowWrite, hj]
        · have : ∀ w, venv.slots[j]? ≠ some (some w) := fun w h => hrw (hiff.mpr ⟨w, h⟩)
          simp only [Bool.not_eq_true] at hrw
          simp [Vm.exec, hwr, hwr', pop1, ho, hr0, hrw]
          cases hsj : venv.slots[j]? with
          | none => simp [bind, Except.bind]
          | some o => cases o <;> simp_all [bind, Except.bind]
    refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
    · cases hex with
      | assignRoot h1 h2 h3 =>
        cases h3
        obtain ⟨_, hs⟩ := ihe.1 _ h1
        exact hs.trans (Star.pc (hr.right.step (by rw [hstore, if_pos h2])) (by simp <;> omega))
      | assignRow h1 h2 h3 h4 h5 =>
        cases h5
        obtain ⟨_, hs⟩ := ihe.1 _ h1
        exact hs.trans (Star.pc (hr.right.step (by rw [hstore, if_neg (by simp [h2]), if_pos h4])) (by simp <;> omega))
    · obtain ⟨v, h1⟩ := ihe.2 hh
      obtain ⟨_, hs⟩ := ihe.1 v h1
      obtain ⟨st, hst⟩ := hr.right.halts (hh.star hs)
      rw [hstore] at hst
      by_cases hr0 : isRootState env.prog t = true
      · exact ⟨_, .assignRoot h1 hr0 .nil⟩
      · rw [if_neg hr0] at hst
        simp only [Bool.not_eq_true] at hr0
        by_cases hrw : (lookup t env.rows).isSome = true
        · exact ⟨_, .assignRow h1 hr0 (hrow hr0).2.1 hrw .nil⟩
        · rw [if_neg hrw] at hst; cases hst
  · cases h1

theorem stmt_send (hc : compileStmt (fuel + 1) p Lay sc n (.send t src args) = .ok c)
    (hx : Ctx env false ls venv L p sc n) (hw : Writes env venv Lay) :
    BlockSpec env ls venv P L Lay [.send t src args] c := by
  simp only [compileStmt, Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ts⟩, ha, h1⟩ := hc
  simp only at h1
  split at h1
  · rename_i s k hk
    simp only [Except.ok.injEq] at h1; subst h1
    obtain ⟨s', hs', hwr⟩ := hw.send t s k hk
    intro pc S cbs fx0 hr
    have iha := ih_args (P := P) ha hx pc S cbs (lowerFx Lay fx0) hr.left
    have hsend : ∀ vs, vs.length = args.length →
        Vm.exec P.length venv (.send k src args.length) (M (pc + ca.length) (vs.reverse ++ S) L cbs (lowerFx Lay fx0)) =
          .ok (.run (M (pc + ca.length + 1) S L cbs (lowerFx Lay (fx0.send t src vs)))) := by
      intro vs hl
      rw [← hl]
      have hwr' : s' ∈ venv.writable := by simpa using hwr
      simp [Vm.exec, hs', hwr, hwr', popN, lowerFx, Effects.send, hk, show ¬ (vs.length + S.length < vs.length) by omega]
    refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
    · cases hex with
      | send h1 h2 =>
        cases h2
        obtain ⟨_, hs⟩ := iha.1 _ h1
        exact hs.trans (Star.pc (hr.right.step (hsend _ (ListR.length h1))) (by simp <;> omega))
    · obtain ⟨vs, h1⟩ := iha.2 hh
      exact ⟨_, .send h1 .nil⟩
  · cases h1

theorem stmt_refresh (hc : compileStmt (fuel + 1) p Lay sc n (.refresh t) = .ok c) :
    BlockSpec env ls venv P L Lay [.refresh t] c := by
  simp only [compileStmt] at hc
  split at hc
  · rename_i r hr0
    simp only [Except.ok.injEq] at hc; subst hc
    intro pc S cbs fx0 hr
    refine ⟨fun fx1 hex => ?_, fun _ => ⟨_, .refresh .nil⟩⟩
    cases hex with
    | refresh h1 =>
      cases h1
      exact hr.step (by simp [Vm.exec, lowerFx, Effects.refresh, hr0])
  · cases hc

theorem stmt_command (hc : compileStmt (fuel + 1) p Lay sc n (.command name args) = .ok c)
    (hx : Ctx env false ls venv L p sc n) :
    BlockSpec env ls venv P L Lay [.command name args] c := by
  simp only [compileStmt] at hc
  split at hc
  · cases hc
  simp only [Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ts⟩, ha, h1⟩ := hc
  simp only [Except.ok.injEq] at h1; subst h1
  intro pc S cbs fx0 hr
  have iha := ih_args (P := P) ha hx pc S cbs (lowerFx Lay fx0) hr.left
  refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
  · cases hex with
    | command h1 h2 =>
      cases h2
      rename_i vs
      obtain ⟨_, hs⟩ := iha.1 _ h1
      have hl := ListR.length h1
      refine hs.trans (Star.pc (hr.right.step
        (m' := M (pc + ca.length + 1) S L cbs (lowerFx Lay (fx0.command name vs))) ?_) (by simp <;> omega))
      rw [← hl]
      simp [Vm.exec, popN, lowerFx, Effects.command, show ¬ (vs.length + S.length < vs.length) by omega]
  · obtain ⟨vs, h1⟩ := iha.2 hh
    exact ⟨_, .command h1 .nil⟩

theorem stmt_if (ihb : BlockOk fuel) (hc : compileStmt (fuel + 1) p Lay sc n (.ifS e thn els) = .ok c)
    (hx : Ctx env false ls venv L p sc n) (hw : Writes env venv Lay) :
    BlockSpec env ls venv P L Lay [.ifS e thn els] c := by
  simp only [compileStmt, Except.bind_ok_iff] at hc
  obtain ⟨⟨cc, tc⟩, hc0, ct, ht0, ce, he0, h1⟩ := hc
  simp only [Except.ok.injEq] at h1; subst h1
  intro pc S cbs fx0 hr
  have ihc := ih_e (P := P) hc0 hx pc S cbs (lowerFx Lay fx0) hr.left.left.left.left
  have iht := fun fx => ihb _ _ _ _ _ _ ht0 _ _ _ _ P hx hw _ S cbs fx hr.left.left.right
  have ihe := fun fx => ihb _ _ _ _ _ _ he0 _ _ _ _ P hx hw _ S cbs fx hr.right
  have jt := hr.left.left.left.right.step (venv := venv) (S := .bool true :: S) (fx := lowerFx Lay fx0)
    exec_jumpIfFalse_true
  have jf := hr.left.left.left.right.step (venv := venv) (S := .bool false :: S) (fx := lowerFx Lay fx0)
    (exec_jumpIfFalse_false hr.top hr.fits (by simp; omega))
  have jm := fun fx => hr.left.right.step (venv := venv) (S := S) (fx := lowerFx Lay fx)
    (exec_jump hr.top hr.fits (by simp; omega))
  refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
  · cases hex with
    | ifTrue h1 h2 h3 =>
      cases h3
      obtain ⟨_, hs1⟩ := ihc.1 _ h1
      refine hs1.trans (jt.join (((iht fx0).1 _ h2).join ((jm _).join .refl ?_) ?_) ?_)
      all_goals (simp <;> omega)
    | ifFalse h1 h2 h3 =>
      cases h3
      obtain ⟨_, hs1⟩ := ihc.1 _ h1
      refine hs1.trans (jf.join (((ihe fx0).1 _ h2).join .refl ?_) ?_)
      all_goals (simp <;> omega)
  · obtain ⟨w, h1⟩ := ihc.2 hh
    obtain ⟨_, hs1⟩ := ihc.1 w h1
    have hh1 := hh.star hs1
    obtain ⟨st, hst⟩ := hr.left.left.left.right.halts hh1
    obtain ⟨bv, rfl⟩ := exec_jumpIfFalse_bool hst
    cases bv
    · obtain ⟨fx1, h2⟩ := (ihe fx0).2 ((hh1.star jf).pc (by simp; omega))
      exact ⟨fx1, .ifFalse h1 h2 .nil⟩
    · obtain ⟨fx1, h2⟩ := (iht fx0).2 ((hh1.star jt).pc (by simp <;> omega))
      exact ⟨fx1, .ifTrue h1 h2 .nil⟩

theorem stmt_match (ihb : BlockOk fuel)
    (hc : compileStmt (fuel + 1) p Lay sc n (.matchS s x sm nn) = .ok c)
    (hx : Ctx env false ls venv L p sc n) (hw : Writes env venv Lay) :
    BlockSpec env ls venv P L Lay [.matchS s x sm nn] c := by
  simp only [compileStmt, Except.bind_ok_iff] at hc
  obtain ⟨⟨cs, ts⟩, hs0, csm, hsm0, cnn, hnn0, h1⟩ := hc
  simp only [Except.ok.injEq] at h1; subst h1
  have hn := hx.len
  intro pc S cbs fx0 hr
  have hl := hr.locals
  have r0 := hr.left.left.left.right
  have r1 := hr.left.right
  have ihs := ih_e (P := P) hs0 hx pc S cbs (lowerFx Lay fx0) hr.left.left.left.left
  have ihsm := fun w (hw' : VTy env.prog.shapes w ts.inner) fx =>
    ihb _ _ _ _ _ _ hsm0 _ _ _ _ P (hx.push (x := x) hw') hw _ S cbs fx (hr.left.left.right.locals_append [w])
  have ihnn := fun fx => ihb _ _ _ _ _ _ hnn0 _ _ _ _ P hx hw _ S cbs fx hr.right
  have js := fun w => r0.step (venv := venv) (S := .some w :: S) (fx := lowerFx Lay fx0) (exec_jumpIfNone_some (w := w))
  have jn := r0.step (venv := venv) (S := .none :: S) (fx := lowerFx Lay fx0)
    (exec_jumpIfNone_none hr.top hr.fits (by simp; omega))
  have uw := fun w => r0.tail.step (venv := venv) (S := .some w :: S) (fx := lowerFx Lay fx0) (exec_unwrap (w := w))
  have bl := fun w => r0.tail.tail.step (venv := venv) (S := w :: S) (fx := lowerFx Lay fx0) exec_bindLocal
  have dl := fun w fx => (r1.locals_append [w]).step (venv := venv) (S := S) (fx := lowerFx Lay fx) (exec_drop hl)
  have jm := fun fx => r1.tail.step (venv := venv) (S := S) (fx := lowerFx Lay fx)
    (exec_jump hr.top hr.fits (by simp; omega))
  have pp := r1.tail.tail.step (venv := venv) (S := .none :: S) (fx := lowerFx Lay fx0) (exec_pop (w := .none))
  refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
  · match hex with
    | .matchSome (w := w) h1 h2 .nil =>
      obtain ⟨htw, hs1⟩ := ihs.1 _ h1
      have hs2 := (ihsm w (vty_inner htw) fx0).1 _ (by subst hn; exact h2)
      refine hs1.trans ((js w).trans ((uw w).trans ((bl w).join (hs2.join ((dl w _).join ((jm _).join .refl ?_) ?_) ?_) ?_)))
      all_goals (simp <;> omega)
    | .matchNone h1 h2 .nil =>
      obtain ⟨_, hs1⟩ := ihs.1 _ h1
      refine hs1.trans (jn.join (pp.join (((ihnn fx0).1 _ h2).join .refl ?_) ?_) ?_)
      all_goals (simp <;> omega)
  · obtain ⟨w, h1⟩ := ihs.2 hh
    obtain ⟨htw, hs1⟩ := ihs.1 w h1
    have hh1 := hh.star hs1
    obtain ⟨st, hst⟩ := r0.halts hh1
    rcases exec_jumpIfNone_opt hst with rfl | ⟨u, rfl⟩
    · have hh2 := ((hh1.star jn).pc (by simp; omega)).star pp
      obtain ⟨fx1, h2⟩ := (ihnn fx0).2 (hh2.pc (by simp; omega))
      exact ⟨fx1, .matchNone h1 h2 .nil⟩
    · have hh2 := ((hh1.star (js u)).star (uw u)).star (bl u)
      obtain ⟨fx1, h2⟩ := (ihsm u (vty_inner htw) fx0).2 (hh2.pc (by simp; omega))
      exact ⟨fx1, .matchSome h1 (by subst hn; exact h2) .nil⟩

theorem stmt_succ (ihb : BlockOk fuel) : StmtOk (fuel + 1) := by
  intro p Lay sc n s c hc env ls venv L P hx hw
  cases s with
  | letS x e => simp [compileStmt] at hc
  | assign t e => exact stmt_assign hc hx hw
  | command name args => exact stmt_command hc hx
  | send t src args => exact stmt_send hc hx hw
  | refresh t => exact stmt_refresh hc
  | ifS e thn els => exact stmt_if ihb hc hx hw
  | matchS s x sm nn => exact stmt_match ihb hc hx hw

theorem block_succ (ihb : BlockOk fuel) (ihs : StmtOk fuel) : BlockOk (fuel + 1) := by
  intro p Lay sc n ss c hc env ls venv L P hx hw
  cases ss with
  | nil =>
    simp [compileBlock] at hc; subst hc
    intro pc S cbs fx0 hr
    refine ⟨fun fx1 hex => ?_, fun _ => ⟨fx0, .nil⟩⟩
    cases hex; simpa using Star.refl
  | cons s rest =>
    by_cases hlet : ∃ x e, s = .letS x e
    · obtain ⟨x, e, rfl⟩ := hlet
      simp only [compileBlock, Except.bind_ok_iff] at hc
      obtain ⟨⟨ce, te⟩, he, cr, hr0, h1⟩ := hc
      simp only [Except.ok.injEq] at h1; subst h1
      intro pc S cbs fx0 hr
      have hl := hr.locals
      have ihe := ih_e (P := P) he hx pc S cbs (lowerFx Lay fx0) hr.left.left.left
      have bl := fun w => hr.left.left.right.step (venv := venv) (S := w :: S) (fx := lowerFx Lay fx0)
        exec_bindLocal
      have ihr := fun w (hw' : VTy env.prog.shapes w te) fx =>
        ihb _ _ _ _ _ _ hr0 _ _ _ _ P (hx.push (x := x) hw') hw _ S cbs fx (hr.left.right.locals_append [w])
      have dl := fun w fx => (hr.right.locals_append [w]).step (venv := venv) (S := S) (fx := lowerFx Lay fx)
        (exec_drop hl)
      refine ⟨fun fx1 hex => ?_, fun hh => ?_⟩
      · cases hex with
        | letS h1 h2 =>
          rename_i w
          obtain ⟨htw, hs1⟩ := ihe.1 _ h1
          have hs2 := (ihr w htw fx0).1 _ h2
          refine hs1.trans ((bl w).join (hs2.join ((dl w fx1).join .refl ?_) ?_) ?_)
          all_goals (simp <;> omega)
      · obtain ⟨w, h1⟩ := ihe.2 hh
        obtain ⟨htw, hs1⟩ := ihe.1 w h1
        obtain ⟨fx1, h2⟩ := (ihr w htw fx0).2 (((hh.star hs1).star (bl w)).pc (by simp <;> omega))
        exact ⟨fx1, .letS h1 h2⟩
    · have hs : ∀ x e, s ≠ .letS x e := fun x e h => hlet ⟨x, e, h⟩
      have hc' : (do
          let c ← compileStmt fuel p Lay sc n s
          let cr ← compileBlock fuel p Lay sc n rest
          Except.ok (c ++ cr) : Except String Code) = .ok c := by
        cases s <;> first | (exact absurd rfl (hs _ _)) | simpa [compileBlock] using hc
      simp only [Except.bind_ok_iff] at hc'
      obtain ⟨cs, hs0, cr, hr0, h1⟩ := hc'
      simp only [Except.ok.injEq] at h1; subst h1
      intro pc S cbs fx0 hr
      have ihs1 := fun fx => ihs _ _ _ _ _ _ hs0 _ _ _ _ P hx hw pc S cbs fx hr.left
      have ihr := fun fx => ihb _ _ _ _ _ _ hr0 _ _ _ _ P hx hw _ S cbs fx hr.right
      refine ⟨fun fx2 hex => ?_, fun hh => ?_⟩
      · obtain ⟨fx1, h1, h2⟩ := (execR_cons hs).mp hex
        exact ((ihs1 fx0).1 _ h1).trans (((ihr fx1).1 _ h2).pc (by simp <;> omega))
      · obtain ⟨fx1, h1⟩ := (ihs1 fx0).2 hh
        obtain ⟨fx2, h2⟩ := (ihr fx1).2 (hh.star ((ihs1 fx0).1 _ h1))
        exact ⟨fx2, (execR_cons hs).mpr ⟨fx1, h1, h2⟩⟩

theorem blockOk : ∀ fuel, BlockOk fuel ∧ StmtOk fuel
  | 0 => ⟨fun _ _ _ _ _ _ h => by simp [compileBlock] at h, fun _ _ _ _ _ _ h => by simp [compileStmt] at h⟩
  | fuel + 1 =>
    have ih := blockOk fuel
    ⟨block_succ ih.1 ih.2, stmt_succ ih.1⟩

/-! ## Whole bodies -/

theorem _root_.Contract.Vm.Runs.after {P venv m m' v fx} (h : Runs P venv m v fx) (hs : Star P venv m m') :
    Runs P venv m' v fx := by
  induction hs with
  | refl => exact h
  | cons hs _ ih =>
    cases h with
    | done hd => rw [hs] at hd; cases hd
    | next hn hr => rw [hs] at hn; cases hn; exact ih hr

/-- A body and its `Return`, at the start. -/
theorem room_init {c : Code} : Room (c ++ [.ret]) c 0 [] [] :=
  ⟨by intro i hi; simp [List.getElem?_append_left hi], fun _ h => by simp at h, by simp,
   fun _ h => by simp at h⟩

theorem ret_runs {c : Code} {venv : Vm.Env} {S fx} :
    Runs (c ++ [.ret]) venv (M c.length S [] [] fx) (S.head?.getD .unit) fx :=
  .done (by simp [Vm.step, Vm.exec])

/-- An expression's code, run from the start: it returns exactly `eval`'s
value, with no effects, and returns at all exactly when `eval` has a value. -/
theorem expr_correct {depth e c t inFn} (hc : compile fuel p depth sc 0 e = .ok (c, t))
    (hx : Ctx env inFn ls venv [] p sc 0) :
    (∀ v, EvalR env inFn ls e v → ∃ k, runFrom (c ++ [.ret]) venv k .init = .ok (v, {})) ∧
    (∀ v fx k, runFrom (c ++ [.ret]) venv k .init = .ok (v, fx) → EvalR env inFn ls e v ∧ fx = {}) := by
  have hs := (allOk fuel).1 _ _ _ _ _ _ _ hc _ _ _ _ _ (c ++ [.ret]) hx 0 [] [] {} room_init
  refine ⟨fun v hv => ?_, fun v fx k hk => ?_⟩
  · obtain ⟨_, hst⟩ := hs.1 v hv
    exact runs_runFrom (Runs.of_star hst (by simpa using ret_runs (c := c) (venv := venv) (S := [v]) (fx := {})))
  · have hr := runFrom_runs hk
    obtain ⟨w, hw⟩ := hs.2 ⟨v, fx, hr⟩
    obtain ⟨_, hst⟩ := hs.1 w hw
    have h1 := hr.after hst
    have h2 : Runs (c ++ [.ret]) venv (M (0 + c.length) [w] [] [] {}) w {} := by
      simpa using ret_runs (c := c) (venv := venv) (S := [w]) (fx := {})
    obtain ⟨rfl, rfl⟩ := Runs.det h1 h2
    exact ⟨hw, rfl⟩

theorem lowerFx_empty : lowerFx Lay {} = {} := rfl

/-- A block's code, run from the start: it returns exactly the effects
`exec` records, and returns at all exactly when `exec` has an outcome. -/
theorem block_correct {ss c} (hc : compileBlock fuel p Lay sc 0 ss = .ok c)
    (hx : Ctx env false ls venv [] p sc 0) (hw : Writes env venv Lay) :
    (∀ fx, ExecR env ls ss {} fx → ∃ k, runFrom (c ++ [.ret]) venv k .init = .ok (.unit, lowerFx Lay fx)) ∧
    (∀ v vfx k, runFrom (c ++ [.ret]) venv k .init = .ok (v, vfx) →
      ∃ fx, ExecR env ls ss {} fx ∧ vfx = lowerFx Lay fx ∧ v = .unit) := by
  have hs := (blockOk fuel).1 _ _ _ _ _ _ hc _ _ _ _ (c ++ [.ret]) hx hw 0 [] [] {} room_init
  rw [lowerFx_empty] at hs
  refine ⟨fun fx hfx => ?_, fun v vfx k hk => ?_⟩
  · have hst := hs.1 fx hfx
    exact runs_runFrom (Runs.of_star hst (by simpa using ret_runs (c := c) (venv := venv) (S := []) (fx := lowerFx Lay fx)))
  · have hr := runFrom_runs hk
    obtain ⟨fx, hfx⟩ := hs.2 ⟨v, vfx, hr⟩
    have h1 := hr.after (hs.1 fx hfx)
    have h2 : Runs (c ++ [.ret]) venv (M (0 + c.length) [] [] [] (lowerFx Lay fx)) .unit (lowerFx Lay fx) := by
      simpa using ret_runs (c := c) (venv := venv) (S := []) (fx := lowerFx Lay fx)
    obtain ⟨rfl, rfl⟩ := Runs.det h1 h2
    exact ⟨fx, hfx, rfl, rfl⟩

/-- **Compiler correctness for a body** (a derive, a slot initializer, a
resource argument): in a machine state that corresponds to the semantics'
environment, the compiled code returns `v` exactly when `eval` answers `v`. -/
theorem compileBody_correct {e code} (hc : compileBody p Lay e = .ok code)
    (hx : Ctx env false [] venv [] p (globalScope p Lay) 0) :
    (∀ v, (∃ k, eval k env false [] e = .ok v) → ∃ k, runFrom code venv k .init = .ok (v, {})) ∧
    (∀ v fx k, runFrom code venv k .init = .ok (v, fx) → (∃ k', eval k' env false [] e = .ok v) ∧ fx = {}) := by
  simp only [compileBody, Except.bind_ok_iff] at hc
  obtain ⟨⟨c, t⟩, h0, h1⟩ := hc
  simp only [Except.ok.injEq] at h1; subst h1
  have h := expr_correct h0 hx
  exact ⟨fun v hv => h.1 v (eval_iff.mpr hv), fun v fx k hk =>
    let ⟨h1, h2⟩ := h.2 v fx k hk; ⟨eval_iff.mp h1, h2⟩⟩

/-- **Compiler correctness for an action**: in a corresponding machine
state whose `writes` admit the body's writes, the compiled body returns
exactly the effects `exec` records (names resolved as the plan did), and
returns at all exactly when `exec` has an outcome. -/
theorem compileAction_correct {a code} (hc : compileAction p Lay a = .ok code)
    (hx : Ctx env false ls venv [] p (paramScope a.params ++ globalScope p Lay) 0) (hw : Writes env venv Lay) :
    (∀ fx, (∃ k, exec k env ls a.body {} = .ok fx) →
      ∃ k, runFrom code venv k .init = .ok (.unit, lowerFx Lay fx)) ∧
    (∀ v vfx k, runFrom code venv k .init = .ok (v, vfx) →
      ∃ fx, (∃ k', exec k' env ls a.body {} = .ok fx) ∧ vfx = lowerFx Lay fx ∧ v = .unit) := by
  simp only [compileAction, Except.bind_ok_iff] at hc
  obtain ⟨c, h0, h1⟩ := hc
  simp only [Except.ok.injEq] at h1; subst h1
  have h := block_correct h0 hx hw
  exact ⟨fun fx hfx => h.1 fx (exec_iff.mpr hfx), fun v vfx k hk =>
    let ⟨fx, h1, h2, h3⟩ := h.2 v vfx k hk; ⟨fx, exec_iff.mp h1, h2, h3⟩⟩

end Contract.Lower
