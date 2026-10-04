/-
Correctness of the compiler for the list-shaped pieces: arguments, a
`fn`'s bound arguments, a template's parts, a record's fields, and the
constructs made of them (templates, records).
-/
import Contract.LowerProof

namespace Contract.Lower

open Vm

variable {fuel : Nat} {p : Program} {depth : Nat} {sc : Scope} {n : Nat} {c : Code} {t : STy}
  {env : Contract.Env} {inFn : Bool} {ls : Locals} {venv : Vm.Env} {L : List Value} {P : Code}

theorem args_succ (ih : AllOk fuel) : ArgsOk (fuel + 1) := by
  intro p depth sc n es c ts hc env inFn ls venv L P hx
  cases es with
  | nil =>
    simp [compileArgs] at hc; obtain ⟨rfl, rfl⟩ := hc
    intro pc S cbs fx hr
    refine ⟨fun vs hv => ?_, fun _ => ⟨[], .nil⟩⟩
    cases hv; exact ⟨by simp [VTys], by simpa using Star.refl⟩
  | cons e es =>
    simp only [compileArgs, Except.bind_ok_iff] at hc
    obtain ⟨⟨ce, te⟩, he, ⟨cs, tss⟩, hs, h1⟩ := hc
    simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
    intro pc S cbs fx hr
    have ihe := ih_expr (P := P) ih he hx pc S cbs fx hr.left
    have ihs := fun w => ih.2.1 _ _ _ _ _ _ _ hs _ _ _ _ _ P hx _ (w :: S) cbs fx hr.right
    refine ⟨fun vs hv => ?_, fun hh => ?_⟩
    · cases hv with
      | cons h1 h2 =>
        rename_i v vs
        obtain ⟨htv, hs1⟩ := ihe.1 v h1
        obtain ⟨htvs, hs2⟩ := (ihs v).1 vs h2
        refine ⟨⟨htv, htvs⟩, hs1.trans (Star.pc (by simpa using hs2) (by simp; omega))⟩
    · obtain ⟨v, h1⟩ := ihe.2 hh
      obtain ⟨_, hs1⟩ := ihe.1 v h1
      obtain ⟨vs, h2⟩ := (ihs v).2 (hh.star hs1)
      exact ⟨v :: vs, .cons h1 h2⟩

theorem bind_succ (ih : AllOk fuel) : BindOk (fuel + 1) := by
  intro p depth sc n es c ts hc env inFn ls venv L P hx
  cases es with
  | nil =>
    simp [compileBind] at hc; obtain ⟨rfl, rfl⟩ := hc
    intro pc S cbs fx hr
    refine ⟨fun vs hv => ?_, fun _ => ⟨[], .nil⟩⟩
    cases hv; exact ⟨by simp [VTys], by simpa using Star.refl⟩
  | cons e es =>
    simp only [compileBind, Except.bind_ok_iff] at hc
    obtain ⟨⟨ce, te⟩, he, ⟨cs, tss⟩, hs, h1⟩ := hc
    simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
    intro pc S cbs fx hr
    have ihe := ih_expr (P := P) ih he hx pc S cbs fx hr.left.left
    have rb := hr.left.right
    have ihs := fun w => ih.2.2.1 _ _ _ _ _ _ _ hs _ _ _ _ _ P (by simpa using hx.append [w]) _ S cbs fx
      (hr.right.locals_append [w])
    have bl := fun w => rb.step (venv := venv) (S := w :: S) (fx := fx) exec_bindLocal
    refine ⟨fun vs hv => ?_, fun hh => ?_⟩
    · cases hv with
      | cons h1 h2 =>
        rename_i v vs
        obtain ⟨htv, hs1⟩ := ihe.1 v h1
        obtain ⟨htvs, hs2⟩ := (ihs v).1 vs h2
        have e1 : L ++ [v] ++ vs = L ++ v :: vs := by simp
        rw [e1] at hs2
        refine ⟨⟨htv, htvs⟩, hs1.trans ((bl v).join (hs2.pc ?_) ?_)⟩
        all_goals (simp <;> omega)
    · obtain ⟨v, h1⟩ := ihe.2 hh
      obtain ⟨_, hs1⟩ := ihe.1 v h1
      obtain ⟨vs, h2⟩ := (ihs v).2 (((hh.star hs1).star (bl v)).pc (by simp <;> omega))
      exact ⟨v :: vs, .cons h1 h2⟩

/-! ## Templates -/

theorem literal_displays : ∀ {parts : List Expr} {s : String}, literalParts parts = some s →
    (∀ ss, DisplaysR env inFn ls parts ss → String.join ss = s) ∧ ∃ ss, DisplaysR env inFn ls parts ss
  | [], s, h => by
    simp [literalParts] at h; subst h
    exact ⟨fun ss hd => by cases hd; rfl, ⟨[], .nil⟩⟩
  | .str s0 :: rest, s, h => by
    simp only [literalParts, Option.map_eq_some_iff] at h
    obtain ⟨s', hs', rfl⟩ := h
    have ⟨ih1, ss', ih2⟩ := literal_displays hs'
    refine ⟨fun ss hd => ?_, ⟨s0 :: ss', .cons .str (by simp [Value.display]) ih2⟩⟩
    cases hd with
    | cons h1 h2 h3 =>
      cases h1; simp [Value.display] at h2; subst h2
      rw [join_cons, ih1 _ h3]
  | .num _ :: _, _, h | .bool _ :: _, _, h | .none :: _, _, h | .emptyList :: _, _, h
  | .some _ :: _, _, h | .template _ :: _, _, h | .var _ :: _, _, h | .member _ _ :: _, _, h
  | .call _ _ :: _, _, h | .record _ _ _ :: _, _, h | .unary _ _ :: _, _, h | .binary _ _ _ :: _, _, h
  | .ternary _ _ _ :: _, _, h | .matchOpt _ _ _ _ :: _, _, h | .arrow _ _ :: _, _, h
  | .letE _ _ _ :: _, _, h | .named _ _ :: _, _, h | .typed _ _ :: _, _, h => by simp [literalParts] at h

/-- A template part's code: its value, printed. -/
theorem part_spec {e ce te} (ihe : ExprSpec env inFn ls venv P L e ce te) :
    ∀ pc S cbs fx, Room P (if te = .string then ce else ce ++ [.call "toString" 1]) pc cbs L →
      (∀ v s, EvalR env inFn ls e v → v.display = .ok s →
        Star P venv (M pc S L cbs fx)
          (M (pc + (if te = .string then ce else ce ++ [.call "toString" 1]).length) (.str s :: S) L cbs fx)) ∧
      (Halts P venv (M pc S L cbs fx) → ∃ v s, EvalR env inFn ls e v ∧ v.display = .ok s) := by
  intro pc S cbs fx hr
  by_cases hts : te = .string
  · simp only [hts, ite_true] at hr ⊢
    have ⟨fwd, bwd⟩ := ihe pc S cbs fx hr
    refine ⟨fun v s hv hd => ?_, fun hh => ?_⟩
    · obtain ⟨htv, hs⟩ := fwd v hv
      rw [hts] at htv
      cases v <;> simp [VTy] at htv
      simp [Value.display] at hd; subst hd
      exact hs
    · obtain ⟨v, hv⟩ := bwd hh
      obtain ⟨htv, _⟩ := fwd v hv
      rw [hts] at htv
      cases v <;> simp [VTy] at htv
      exact ⟨_, _, hv, rfl⟩
  · simp only [hts, ite_false] at hr ⊢
    have ⟨fwd, bwd⟩ := ihe pc S cbs fx hr.left
    have hcall : ∀ v, Vm.exec P.length venv (.call "toString" 1) (M (pc + ce.length) (v :: S) L cbs fx) =
        match v.display with
        | .ok s => .ok (.run (M (pc + ce.length + 1) (.str s :: S) L cbs fx))
        | .error e => .error (.call e) := by
      intro v
      cases hd : v.display <;> simp [Vm.exec, popN, stdlib, hd, Functor.map, Except.map]
    refine ⟨fun v s hv hd => ?_, fun hh => ?_⟩
    · obtain ⟨_, hs⟩ := fwd v hv
      have := hr.right.step (venv := venv) (S := v :: S) (fx := fx) (by rw [hcall v, hd])
      exact hs.trans (Star.pc this (by simp; omega))
    · obtain ⟨v, hv⟩ := bwd hh
      obtain ⟨_, hs⟩ := fwd v hv
      obtain ⟨st, hst⟩ := hr.right.halts (hh.star hs)
      rw [hcall v] at hst
      cases hd : v.display with
      | error e => rw [hd] at hst; cases hst
      | ok s => exact ⟨v, s, hv, hd⟩

theorem parts_succ (ih : AllOk fuel) : PartsOk (fuel + 1) := by
  intro p depth sc n first es c hc env inFn ls venv L P hx
  cases es with
  | nil =>
    simp [compileParts] at hc; subst hc
    intro pc S cbs fx hr
    cases first
    · simp only [Bool.false_eq_true, ite_false]
      intro acc
      refine ⟨fun ss hd => ?_, fun _ => ⟨[], .nil⟩⟩
      cases hd; simpa [join_nil] using Star.refl
    · simp
  | cons e es =>
    simp only [compileParts, Except.bind_ok_iff] at hc
    obtain ⟨⟨ce, te⟩, he, cs, hs, h1⟩ := hc
    simp only [Except.ok.injEq] at h1; subst h1
    intro pc S cbs fx hr
    have ihe := ih_expr (P := P) ih he hx
    have ihs := ih.2.2.2.1 _ _ _ _ _ _ _ hs _ _ _ _ _ P hx _ S cbs fx hr.right
    cases first
    · -- A later part: printed, then concatenated.
      simp only [Bool.false_eq_true, ite_false] at hr ihs ⊢
      intro acc
      have hpart := part_spec ihe pc (.str acc :: S) cbs fx hr.left.left
      have hcat := fun s => hr.left.right.step (venv := venv) (S := .str s :: .str acc :: S) (fx := fx)
        (m' := M _ (.str (acc ++ s) :: S) L cbs fx) (by simp [Vm.exec, pop2, binary]; rfl)
      refine ⟨fun ss hd => ?_, fun hh => ?_⟩
      · cases hd with
        | cons h1 h2 h3 =>
          rename_i v s ss
          have h4 := (ihs (acc ++ s)).1 ss h3
          rw [join_cons, ← String.append_assoc]
          refine ((hpart.1 v s h1 h2).trans (hcat s)).join (h4.pc ?_) ?_
          all_goals (simp <;> omega)
      · obtain ⟨v, s, h1, h2⟩ := hpart.2 hh
        have hh1 := (hh.star (hpart.1 v s h1 h2)).star (hcat s)
        obtain ⟨ss, h3⟩ := (ihs (acc ++ s)).2 (hh1.pc (by simp <;> omega))
        exact ⟨s :: ss, .cons h1 h2 h3⟩
    · -- The first part: printed.
      simp only [ite_true, Bool.false_eq_true, ite_false] at hr ihs ⊢
      intro _
      have hpart := part_spec ihe pc S cbs fx hr.left
      refine ⟨fun ss hd => ?_, fun hh => ?_⟩
      · cases hd with
        | cons h1 h2 h3 =>
          rename_i v s ss
          have h4 := (ihs s).1 ss h3
          rw [join_cons]
          exact (hpart.1 v s h1 h2).join (h4.pc (by simp; omega)) (by simp)
      · obtain ⟨v, s, h1, h2⟩ := hpart.2 hh
        obtain ⟨ss, h3⟩ := (ihs s).2 ((hh.star (hpart.1 v s h1 h2)).pc (by simp))
        exact ⟨s :: ss, .cons h1 h2 h3⟩

theorem case_template (ih : AllOk fuel) (ihp : PartsOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.template parts) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.template parts) c t := by
  simp only [compile] at hc
  split at hc
  · rename_i s hlit
    simp at hc; obtain ⟨rfl, rfl⟩ := hc
    have ⟨h1, ss, h2⟩ := literal_displays (env := env) (inFn := inFn) (ls := ls) hlit
    refine spec_push (w := .str s) (fun v => ⟨fun hv => ?_, fun hv => ?_⟩) (by simp [VTy])
      (fun _ _ _ _ => by simp [Vm.exec])
    · cases hv with
      | template hd => rw [h1 _ hd]
    · subst hv; rw [← h1 _ h2]; exact .template h2
  · rename_i hlit
    simp only [Except.bind_ok_iff] at hc
    obtain ⟨cp, hp, h1⟩ := hc
    simp at h1; obtain ⟨rfl, rfl⟩ := h1
    have hne : parts ≠ [] := by intro h; subst h; simp [literalParts] at hlit
    intro pc S cbs fx hr
    have hs := ihp _ _ _ _ _ _ _ hp _ _ _ _ _ P hx pc S cbs fx hr
    simp only [ite_true] at hs
    have ⟨fwd, bwd⟩ := hs hne
    refine ⟨fun v hv => ?_, fun hh => ?_⟩
    · cases hv with
      | template hd => exact ⟨by simp [VTy], fwd _ hd⟩
    · obtain ⟨ss, hd⟩ := bwd hh
      exact ⟨_, .template hd⟩

/-! ## Records -/

theorem _root_.Contract.FieldsR.length {env inFn ls written bv} : ∀ {fs i vs},
    FieldsR env inFn ls written bv fs i vs → vs.length = fs.length
  | _, _, _, .nil => rfl
  | _, _, _, .written _ _ h => by simp [FieldsR.length h]
  | _, _, _, .base _ _ h => by simp [FieldsR.length h]

theorem fields_succ (ih : AllOk fuel) : FieldsOk (fuel + 1) := by
  intro p depth sc n written base shape fs i c ok hc env inFn ls venv L P bv hx hbase hdecl
  cases fs with
  | nil =>
    simp [compileFields] at hc; obtain ⟨rfl, rfl⟩ := hc
    intro pc S cbs fx hr
    refine ⟨fun vs hv => ?_, fun _ => ⟨[], .nil⟩⟩
    cases hv; exact ⟨fun _ => by simp [VTys], by simpa using Star.refl⟩
  | cons f fs =>
    rw [compileFields.eq_def] at hc
    simp only at hc
    have hdecl' : ∀ b s', base = .some (b, s') → s' = shape → ∃ d, p.shapes.find? (·.name == shape) = .some d ∧
        d.fields.drop (i + 1) = fs := by
      intro b s' hb hs'
      obtain ⟨d, hd, hdr⟩ := hdecl b s' hb hs'
      refine ⟨d, hd, ?_⟩
      rw [← List.drop_drop, hdr]; rfl
    intro pc S cbs fx hr
    split at hc
    · -- The field is written.
      rename_i e hlook
      simp only [Except.bind_ok_iff, pure, Except.pure, Except.ok.injEq] at hc
      obtain ⟨⟨ce, te⟩, he, ⟨c1, ok1⟩, h3, ⟨cs, oks⟩, hs, h2⟩ := hc
      simp only [Prod.mk.injEq] at h2 h3
      obtain ⟨rfl, rfl⟩ := h3; obtain ⟨rfl, rfl⟩ := h2
      have ihs := fun S' => ih.2.2.2.2 _ _ _ _ _ _ _ _ _ _ _ hs _ _ _ _ _ P bv hx hbase hdecl' _ S' cbs fx hr.right
      have ihe := ih_expr (P := P) ih he hx pc S cbs fx hr.left
      refine ⟨fun vs hv => ?_, fun hh => ?_⟩
      · cases hv with
        | written h4 h5 h6 =>
          rename_i e' v vs
          rw [hlook] at h4; cases h4
          obtain ⟨htv, hs1⟩ := ihe.1 v h5
          obtain ⟨htvs, hs2⟩ := (ihs (v :: S)).1 vs h6
          refine ⟨fun hok => ?_, hs1.trans (Star.pc (by simpa using hs2) (by simp; omega))⟩
          simp at hok
          exact ⟨vty_sub hok.1 htv, htvs hok.2⟩
        | base h4 _ _ => rw [hlook] at h4; cases h4
      · obtain ⟨v, h4⟩ := ihe.2 hh
        obtain ⟨_, hs1⟩ := ihe.1 v h4
        obtain ⟨vs, h5⟩ := (ihs (v :: S)).2 (hh.star hs1)
        exact ⟨v :: vs, .written hlook h4 h5⟩
    · -- From the base.
      rename_i hlook
      split at hc
      · rename_i b s'
        simp only [Except.bind_ok_iff, pure, Except.pure, Except.ok.injEq] at hc
        obtain ⟨⟨c1, ok1⟩, h3, ⟨cs, oks⟩, hs, h2⟩ := hc
        simp only [Prod.mk.injEq] at h2 h3
        obtain ⟨rfl, rfl⟩ := h3; obtain ⟨rfl, rfl⟩ := h2
        have ihs := fun S' => ih.2.2.2.2 _ _ _ _ _ _ _ _ _ _ _ hs _ _ _ _ _ P bv hx hbase hdecl' _ S' cbs fx hr.right
        cases bv with
        | none => simp [BaseOk] at hbase
        | some bs =>
          simp only [BaseOk] at hbase
          obtain ⟨hLb, hty⟩ := hbase
          have ld := hr.left.step (venv := venv) (S := S) (fx := fx) (exec_loadLocal hLb)
          have hfield : ∀ v, bs[i]? = some v →
              Star P venv (M pc S L cbs fx) (M (pc + 1 + 1) (v :: S) L cbs fx) := fun v hv =>
            ld.trans (hr.left.tail.step (venv := venv) (S := .record s' bs :: S) (fx := fx)
              (by simp [Vm.exec, pop1, hv]))
          refine ⟨fun vs hv => ?_, fun hh => ?_⟩
          · cases hv with
            | written h4 _ _ => rw [hlook] at h4; cases h4
            | base _ h5 h6 =>
              revert h5 h6
              rename_i v vs _
              intro h5 h6
              obtain ⟨htvs, hs2⟩ := (ihs (v :: S)).1 vs h6
              refine ⟨fun hok => ?_, (hfield v h5).trans (Star.pc (by simpa using hs2) (by simp <;> omega))⟩
              simp at hok
              obtain ⟨rfl, hok2⟩ := hok
              obtain ⟨d, hd, hdr⟩ := hdecl b s' rfl rfl
              simp only [VTy] at hty
              obtain ⟨-, d', hd', hts⟩ := hty
              rw [hx.prog, hd] at hd'; cases hd'
              refine ⟨VTys.get hts h5 ?_, htvs hok2⟩
              have : d.fields[i]? = some f := by
                have := congrArg (·[0]?) hdr
                simpa [List.getElem?_drop] using this
              simp [this]
          · obtain ⟨st, hst⟩ := hr.left.tail.halts (hh.star ld)
            cases hv : bs[i]? with
            | none => simp [Vm.exec, pop1, hv] at hst
            | some v =>
              obtain ⟨vs, h6⟩ := (ihs (v :: S)).2 ((hh.star (hfield v hv)).pc (by simp))
              exact ⟨v :: vs, .base hlook hv h6⟩
      · simp [Except.bind_ok_iff] at hc

theorem exec_record {P : Code} {venv q vs S L cbs fx shape} :
    Vm.exec P.length venv (.record shape vs.length) (M q (vs.reverse ++ S) L cbs fx) =
      .ok (.run (M (q + 1) (.record shape vs :: S) L cbs fx)) := by
  have : ¬ (vs.length + S.length < vs.length) := by omega
  simp [Vm.exec, popN, this]

theorem case_record (ih : AllOk fuel)
    (hc : compile (fuel + 1) p depth sc n (.record shape base fields) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.record shape base fields) c t := by
  have hp := hx.prog
  simp only [compile] at hc
  split at hc
  · cases hc
  rename_i decl hdecl
  have hshape : env.shape shape = some decl := by simp [Env.shape, hp, hdecl]
  have vty_rec : ∀ (ok : Bool) vs, (ok = true → VTys env.prog.shapes vs (decl.fields.map fun f => STy.ofTy f.ty)) →
      VTy env.prog.shapes (.record shape vs) (if ok then .record shape else .top) := by
    intro ok vs h
    cases ok
    · exact VTy.top'
    · simp only [ite_true, VTy, true_and]
      exact ⟨decl, by rw [hp]; exact hdecl, h rfl⟩
  split at hc
  · -- No base.
    simp only [Except.bind_ok_iff] at hc
    obtain ⟨⟨cf, ok⟩, hf, h1⟩ := hc
    simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
    intro pc S cbs fx hr
    have ihf := ih.2.2.2.2 _ _ _ _ _ _ _ _ _ _ _ hf _ _ _ _ _ P .none hx (by simp [BaseOk])
      (fun _ _ h => by cases h) pc S cbs fx hr.left
    have hrec := fun (vs : List Value) (hl : vs.length = decl.fields.length) =>
      hr.right.step (venv := venv) (S := vs.reverse ++ S) (fx := fx) (by rw [← hl]; exact exec_record)
    refine ⟨fun v hv => ?_, fun hh => ?_⟩
    · cases hv with
      | record hd hfs =>
        rw [hshape] at hd; cases hd
        obtain ⟨hty, hs⟩ := ihf.1 _ hfs
        exact ⟨vty_rec ok _ hty, hs.trans (Star.pc (hrec _ hfs.length) (by simp <;> omega))⟩
    · obtain ⟨vs, hfs⟩ := ihf.2 hh
      exact ⟨_, .record hshape hfs⟩
  · -- A base, bound as local `n`.
    rename_i b
    simp only [Except.bind_ok_iff] at hc
    obtain ⟨⟨cb, tb⟩, hb, h1⟩ := hc
    cases tb
    case record s' =>
      simp only [Except.bind_ok_iff] at h1
      obtain ⟨⟨cf, ok⟩, hf, h2⟩ := h1
      simp only [Except.ok.injEq, Prod.mk.injEq] at h2; obtain ⟨rfl, rfl⟩ := h2
      intro pc S cbs fx hr
      have hl := hr.locals
      have ihb := ih_expr (P := P) ih hb hx pc S cbs fx hr.left.left.left
      have bl := fun w => hr.left.left.right.step (venv := venv) (S := w :: S) (fx := fx) exec_bindLocal
      have ihf := fun bs (hty : VTy env.prog.shapes (.record s' bs) (.record s')) =>
        ih.2.2.2.2 _ _ _ _ _ _ _ _ _ _ _ hf _ _ _ _ _ P (.some bs) (by simpa using hx.append [.record s' bs])
          (by simp only [BaseOk]; exact ⟨by rw [← hx.len]; simp, hty⟩)
          (fun _ _ h hs => by cases h; subst hs; exact ⟨decl, hdecl, rfl⟩) _ S cbs fx
          (hr.left.right.locals_append [.record s' bs])
      have rd := hr.right
      refine ⟨fun v hv => ?_, fun hh => ?_⟩
      · obtain ⟨s'', bs, decl', vs, h1, hd, hfs, rfl⟩ : ∃ s'' bs decl' vs,
            EvalR env inFn ls b (.record s'' bs) ∧ env.shape shape = some decl' ∧
            FieldsR env inFn ls fields (.some bs) decl'.fields 0 vs ∧ v = .record shape vs := by
          cases hv with | recordBase h1 h2 h3 => exact ⟨_, _, _, _, h1, h2, h3, rfl⟩
        · rw [hshape] at hd; cases hd
          obtain ⟨htw, hs1⟩ := ihb.1 _ h1
          have : s'' = s' := by simp [VTy] at htw; exact htw.1
          subst this
          obtain ⟨hty, hs2⟩ := (ihf bs htw).1 _ hfs
          have r1 := (rd.locals_append [.record s'' bs]).step (venv := venv) (S := vs.reverse ++ S) (fx := fx)
            (by rw [← hfs.length]; exact exec_record)
          have r2 := (rd.tail.locals_append [.record s'' bs]).step (venv := venv) (S := .record shape vs :: S)
            (fx := fx) (exec_drop hl)
          refine ⟨vty_rec ok _ hty, hs1.trans ((bl _).join (hs2.join (r1.join (r2.join .refl ?_) ?_) ?_) ?_)⟩
          all_goals (simp <;> omega)
      · obtain ⟨w, h1⟩ := ihb.2 hh
        obtain ⟨htw, hs1⟩ := ihb.1 w h1
        cases w <;> simp [VTy] at htw
        rename_i s'' bs
        obtain ⟨rfl, _⟩ := htw
        obtain ⟨htw, _⟩ := ihb.1 _ h1
        obtain ⟨vs, hfs⟩ := (ihf bs htw).2 (((hh.star hs1).star (bl _)).pc (by simp <;> omega))
        exact ⟨_, .recordBase h1 hshape hfs⟩
    all_goals cases h1

end Contract.Lower
