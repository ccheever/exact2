/-
Correctness of the compiler for calls: a `fn` expanded inline with its
arguments as locals, `map` and `filter` with their callback body run by the
VM once per item, `pending`/`failed`, and any other roster entry.
-/
import Contract.LowerLists

namespace Contract.Lower

open Vm

variable {fuel : Nat} {p : Program} {depth : Nat} {sc : Scope} {n : Nat} {c : Code} {t : STy}
  {env : Contract.Env} {inFn : Bool} {ls : Locals} {venv : Vm.Env} {L : List Value} {P : Code}

/-! ## `fn` -/

theorem _root_.Contract.ListR.length {env inFn ls} : ∀ {es vs}, ListR env inFn ls es vs → vs.length = es.length
  | _, _, .nil => rfl
  | _, _, .cons _ h => by simp [ListR.length h]

/-- The `DropLocal`s after a `fn`'s body. -/
theorem drops_star {P : Code} {venv S cbs fx} : ∀ (k q : Nat) (L ws : List Value), ws.length = k →
    Room P (List.replicate k .dropLocal) q cbs L → LocalsOk cbs L →
    Star P venv (M q S (L ++ ws) cbs fx) (M (q + k) S L cbs fx)
  | 0, q, L, ws, h, _, _ => by rw [List.length_eq_zero_iff.mp h]; simpa using Star.refl
  | k + 1, q, L, ws, h, hr, hl => by
    have hne : ws ≠ [] := by intro h'; subst h'; simp at h
    rw [← List.dropLast_concat_getLast hne]
    have hr' : Room P (List.replicate (k + 1) .dropLocal) q cbs (L ++ ws.dropLast ++ [ws.getLast hne]) := by
      rw [List.append_assoc]; exact hr.locals_append _
    have h1 := hr'.step (venv := venv) (S := S) (fx := fx)
      (exec_drop (L := L ++ ws.dropLast) (w := ws.getLast hne) (hl.append _))
    have h2 := drops_star (venv := venv) (S := S) (fx := fx) k (q + 1) L ws.dropLast (by simp [h]) hr.tail hl
    rw [List.append_assoc] at h1
    exact (h1.trans h2).pc (by omega)

theorem vtys_refine {sh} : ∀ {vs : List Value} {ts : List STy} {ds : List (String × Ty)},
    VTys sh vs ts → ts.length = ds.length →
    VTys sh vs ((ts.zip ds).map fun (t, (_, d)) => t.refine (STy.ofTy d))
  | [], [], [], _, _ => by simp [VTys]
  | v :: vs, t :: ts, d :: ds, h, hl => by
    simp only [VTys] at h
    simp only [List.zip_cons_cons, List.map_cons, VTys]
    exact ⟨vty_refine h.1, vtys_refine h.2 (by simpa using hl)⟩
  | [], _ :: _, _, h, _ | _ :: _, [], _, h, _ => by simp [VTys] at h
  | [], [], _ :: _, _, hl | _ :: _, _ :: _, [], _, hl => by simp at hl

theorem case_fn (ih : AllOk fuel) {name args fd} (hfd : p.fns.find? (·.name == name) = some fd)
    (hc : (do
      let (ca, ts) ← compileBind fuel p depth sc n args
      let ts := (ts.zip fd.params).map fun (t, (_, d)) => t.refine (STy.ofTy d)
      let (cb, tb) ← compile fuel p (depth + 1) (fnScope (fd.params.map (·.1)) n ts) (n + args.length) fd.body
      Except.ok (ca ++ cb ++ List.replicate args.length Instr.dropLocal, tb.refine (STy.ofTy fd.ret)) :
        Except String (Code × STy)) = .ok (c, t))
    (hlen : args.length = fd.params.length)
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.call name args) c t := by
  have hp := hx.prog
  simp only [Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ts0⟩, hca, ⟨cb, tb⟩, hcb, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  intro pc S cbs fx hr
  have hl := hr.locals
  have ihb := ih.2.2.1 _ _ _ _ _ _ _ hca _ _ _ _ _ P hx pc S cbs fx hr.left.left
  have hfd' : env.prog.fns.find? (·.name == name) = some fd := by rw [hp]; exact hfd
  have body_ok : ∀ vs, ListR env inFn ls args vs → VTys env.prog.shapes vs ts0 →
      ExprSpec env true ((fd.params.map (·.1)).zip vs).reverse venv P (L ++ vs) fd.body cb tb := by
    intro vs hvs hts
    have hvl : vs.length = args.length := ListR.length hvs
    refine ih_expr ih hcb ⟨hp, ?_, by simp [hx.len, hvl], hx.quiet⟩
    have := fnScope_agree (env := env) (venv := venv) (ps := fd.params.map (·.1)) (L := L)
      (vtys_refine (ds := fd.params) hts (by rw [← hts.length, hvl, hlen]))
      (by simp [hvl, hlen])
    rwa [hx.len] at this
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · cases hv
    case fn hfd2 hargs hbody =>
      rw [hfd'] at hfd2; cases hfd2
      rename_i vs
      obtain ⟨hts, hs1⟩ := ihb.1 vs hargs
      have hvl := ListR.length hargs
      obtain ⟨htv, hs2⟩ := (body_ok vs hargs hts _ S cbs fx (hr.left.right.locals_append vs)).1 v hbody
      have hd := drops_star (venv := venv) (S := v :: S) (fx := fx) args.length _ L vs hvl hr.right hl
      exact ⟨vty_refine htv, hs1.trans (hs2.join (hd.pc (by simp; omega)) (by simp <;> omega))⟩
    all_goals simp_all
  · obtain ⟨vs, hargs⟩ := ihb.2 hh
    obtain ⟨hts, hs1⟩ := ihb.1 vs hargs
    obtain ⟨v, hbody⟩ := (body_ok vs hargs hts _ S cbs fx (hr.left.right.locals_append vs)).2
      ((hh.star hs1).pc (by simp))
    exact ⟨v, .fn hfd' hargs hbody⟩

/-! ## `map` and `filter` -/

/-- A callback in progress. -/
def mkCb (filter : Bool) (start stop : Nat) (x : Value) (i : Nat) (rest out : List Value) (base : Nat)
    (caller : List Value) : Callback :=
  { filter, start, stop, cur := x, idx := i, rest, out, base, caller }

/-- A callback body's end, for `map`. -/
theorem step_end_map {P : Code} {venv start stop x i rest out L S cbs fx y} :
    Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)]) (mkCb false start stop x i rest out L.length S :: cbs) fx) =
      match rest with
      | x' :: rest' => .ok (.run (M start [] (L ++ [x', .num (F64.ofNat (i + 1))])
          (mkCb false start stop x' (i + 1) rest' (out ++ [y]) L.length S :: cbs) fx))
      | [] => .ok (.run (M stop (.list (out ++ [y]) :: S) L cbs fx)) := by
  cases rest <;> simp [Vm.step, bodyEnd, mkCb]

/-- A callback body's end, for `filter`. -/
theorem step_end_filter {P : Code} {venv start stop x i rest out L S cbs fx b} :
    Vm.step P venv (M stop [.bool b] (L ++ [x, .num (F64.ofNat i)])
      (mkCb true start stop x i rest out L.length S :: cbs) fx) =
      match rest with
      | x' :: rest' => .ok (.run (M start [] (L ++ [x', .num (F64.ofNat (i + 1))])
          (mkCb true start stop x' (i + 1) rest' (out ++ if b then [x] else []) L.length S :: cbs) fx))
      | [] => .ok (.run (M stop (.list (out ++ if b then [x] else []) :: S) L cbs fx)) := by
  cases rest <;> cases b <;> simp [Vm.step, bodyEnd, mkCb, pure, Except.pure, bind, Except.bind]

theorem step_end_filter_bool {P : Code} {venv start stop x i rest out L S cbs fx y s}
    (h : Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)])
      (mkCb true start stop x i rest out L.length S :: cbs) fx) = .ok s) : ∃ b, y = .bool b := by
  cases y <;> simp [Vm.step, bodyEnd, mkCb, bind, Except.bind] at h
  exact ⟨_, rfl⟩

theorem vty_item {sh xs t} (h : VTy sh (.list xs) t) : VTyAll sh xs t.item := by
  cases t <;> simp [VTy, STy.item] at h ⊢ <;> try exact h
  induction xs with
  | nil => simp [VTyAll]
  | cons x xs ih => simp [VTyAll]; exact ⟨VTy.top', ih⟩

theorem map_loop {ps body cb tb item q stop S cbs fx}
    (hbody : ∀ x i, VTy env.prog.shapes x item →
      ExprSpec env inFn (bindParams ps x i ls) venv P (L ++ [x, .num (F64.ofNat i)]) body cb tb)
    (hroom : ∀ x i rest out, Room P cb (q + 1) (mkCb false (q + 1) stop x i rest out L.length S :: cbs)
      (L ++ [x, .num (F64.ofNat i)]))
    (hstop : stop = q + 1 + cb.length) :
    ∀ rest x i out, VTyAll env.prog.shapes (x :: rest) item →
      (∀ ys, MapR env inFn ls ps body (x :: rest) i ys → VTyAll env.prog.shapes ys tb ∧
        Star P venv (M (q + 1) [] (L ++ [x, .num (F64.ofNat i)]) (mkCb false (q + 1) stop x i rest out L.length S :: cbs) fx)
          (M stop (.list (out ++ ys) :: S) L cbs fx)) ∧
      (Halts P venv (M (q + 1) [] (L ++ [x, .num (F64.ofNat i)]) (mkCb false (q + 1) stop x i rest out L.length S :: cbs) fx) →
        ∃ ys, MapR env inFn ls ps body (x :: rest) i ys) := by
  intro rest
  induction rest with
  | nil =>
    intro x i out hx
    have hb := hbody x i hx.1 _ [] _ fx (hroom x i [] out)
    refine ⟨fun ys hm => ?_, fun hh => ?_⟩
    · cases hm with
      | cons hy hn =>
        rename_i y ys'
        cases hn
        obtain ⟨hty, hs⟩ := hb.1 y hy
        refine ⟨⟨hty, trivial⟩, (hs.pc (by omega : q + 1 + cb.length = stop)).trans (Star.one ?_)⟩
        rw [step_end_map]
    · obtain ⟨y, hy⟩ := hb.2 hh
      exact ⟨[y], .cons hy .nil⟩
  | cons x' rest ih =>
    intro x i out hx
    have hb := hbody x i hx.1 _ [] _ fx (hroom x i (x' :: rest) out)
    refine ⟨fun ys hm => ?_, fun hh => ?_⟩
    · cases hm with
      | cons hy hn =>
        rename_i y ys'
        obtain ⟨hty, hs⟩ := hb.1 y hy
        obtain ⟨htys, hs2⟩ := (ih x' (i + 1) (out ++ [y]) hx.2).1 ys' hn
        refine ⟨⟨hty, htys⟩, (hs.pc (by omega : q + 1 + cb.length = stop)).trans
          (.cons (m' := M (q + 1) [] (L ++ [x', .num (F64.ofNat (i + 1))])
            (mkCb false (q + 1) stop x' (i + 1) rest (out ++ [y]) L.length S :: cbs) fx) ?_ ?_)⟩
        · rw [step_end_map]
        · simpa using hs2
    · obtain ⟨y, hy⟩ := hb.2 hh
      obtain ⟨_, hs⟩ := hb.1 y hy
      have hh1 := (hh.star (hs.pc (by omega : q + 1 + cb.length = stop))).next step_end_map
      obtain ⟨ys, hn⟩ := (ih x' (i + 1) (out ++ [y]) hx.2).2 hh1
      exact ⟨y :: ys, .cons hy hn⟩

theorem filter_loop {ps body cb tb item q stop S cbs fx}
    (hbody : ∀ x i, VTy env.prog.shapes x item →
      ExprSpec env inFn (bindParams ps x i ls) venv P (L ++ [x, .num (F64.ofNat i)]) body cb tb)
    (hroom : ∀ x i rest out, Room P cb (q + 1) (mkCb true (q + 1) stop x i rest out L.length S :: cbs)
      (L ++ [x, .num (F64.ofNat i)]))
    (hstop : stop = q + 1 + cb.length) :
    ∀ rest x i out, VTyAll env.prog.shapes (x :: rest) item →
      (∀ ys, FilterR env inFn ls ps body (x :: rest) i ys → VTyAll env.prog.shapes ys item ∧
        Star P venv (M (q + 1) [] (L ++ [x, .num (F64.ofNat i)]) (mkCb true (q + 1) stop x i rest out L.length S :: cbs) fx)
          (M stop (.list (out ++ ys) :: S) L cbs fx)) ∧
      (Halts P venv (M (q + 1) [] (L ++ [x, .num (F64.ofNat i)]) (mkCb true (q + 1) stop x i rest out L.length S :: cbs) fx) →
        ∃ ys, FilterR env inFn ls ps body (x :: rest) i ys) := by
  intro rest
  induction rest with
  | nil =>
    intro x i out hx
    have hb := hbody x i hx.1 _ [] _ fx (hroom x i [] out)
    refine ⟨fun ys hm => ?_, fun hh => ?_⟩
    · cases hm with
      | keep hy hn =>
        cases hn
        obtain ⟨_, hs⟩ := hb.1 _ hy
        refine ⟨⟨hx.1, trivial⟩, (hs.pc (by omega : q + 1 + cb.length = stop)).trans (Star.one ?_)⟩
        rw [step_end_filter]; simp
      | drop hy hn =>
        cases hn
        obtain ⟨_, hs⟩ := hb.1 _ hy
        refine ⟨trivial, (hs.pc (by omega : q + 1 + cb.length = stop)).trans (Star.one ?_)⟩
        rw [step_end_filter]; simp
    · obtain ⟨y, hy⟩ := hb.2 hh
      obtain ⟨_, hs⟩ := hb.1 y hy
      have hh1 := hh.star (hs.pc (by omega : q + 1 + cb.length = stop))
      obtain ⟨st, hst⟩ : ∃ st, Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)])
          (mkCb true (q + 1) stop x i [] out L.length S :: cbs) fx) = .ok st := by
        cases h : Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)])
          (mkCb true (q + 1) stop x i [] out L.length S :: cbs) fx) with
        | error e => exact absurd h hh1.not_error
        | ok st => exact ⟨st, rfl⟩
      obtain ⟨b, rfl⟩ := step_end_filter_bool hst
      cases b
      · exact ⟨[], .drop hy .nil⟩
      · exact ⟨[x], .keep hy .nil⟩
  | cons x' rest ih =>
    intro x i out hx
    have hb := hbody x i hx.1 _ [] _ fx (hroom x i (x' :: rest) out)
    refine ⟨fun ys hm => ?_, fun hh => ?_⟩
    · cases hm with
      | keep hy hn =>
        rename_i ys'
        obtain ⟨_, hs⟩ := hb.1 _ hy
        obtain ⟨htys, hs2⟩ := (ih x' (i + 1) (out ++ [x]) hx.2).1 ys' hn
        refine ⟨⟨hx.1, htys⟩, (hs.pc (by omega : q + 1 + cb.length = stop)).trans
          (.cons (m' := M (q + 1) [] (L ++ [x', .num (F64.ofNat (i + 1))])
            (mkCb true (q + 1) stop x' (i + 1) rest (out ++ [x]) L.length S :: cbs) fx) ?_ ?_)⟩
        · rw [step_end_filter]; simp
        · simpa using hs2
      | drop hy hn =>
        obtain ⟨_, hs⟩ := hb.1 _ hy
        obtain ⟨htys, hs2⟩ := (ih x' (i + 1) out hx.2).1 _ hn
        refine ⟨htys, (hs.pc (by omega : q + 1 + cb.length = stop)).trans
          (.cons (m' := M (q + 1) [] (L ++ [x', .num (F64.ofNat (i + 1))])
            (mkCb true (q + 1) stop x' (i + 1) rest out L.length S :: cbs) fx) ?_ ?_)⟩
        · rw [step_end_filter]; simp
        · simpa using hs2
    · obtain ⟨y, hy⟩ := hb.2 hh
      obtain ⟨_, hs⟩ := hb.1 y hy
      have hh1 := hh.star (hs.pc (by omega : q + 1 + cb.length = stop))
      obtain ⟨st, hst⟩ : ∃ st, Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)])
          (mkCb true (q + 1) stop x i (x' :: rest) out L.length S :: cbs) fx) = .ok st := by
        cases h : Vm.step P venv (M stop [y] (L ++ [x, .num (F64.ofNat i)])
          (mkCb true (q + 1) stop x i (x' :: rest) out L.length S :: cbs) fx) with
        | error e => exact absurd h hh1.not_error
        | ok st => exact ⟨st, rfl⟩
      obtain ⟨b, rfl⟩ := step_end_filter_bool hst
      have hh2 := hh1.next step_end_filter
      cases b
      · obtain ⟨ys, hn⟩ := (ih x' (i + 1) _ hx.2).2 hh2
        exact ⟨ys, .drop hy hn⟩
      · obtain ⟨ys, hn⟩ := (ih x' (i + 1) _ hx.2).2 hh2
        exact ⟨x :: ys, .keep hy hn⟩

theorem exec_each_nil {P : Code} {venv q S L cbs fx off} {fl : Bool} (hq : TopOk cbs (q + 1 + off))
    (hs : q + 1 + off ≤ P.length) :
    Vm.exec P.length venv (if fl then .filter off else .map off) (M q (.list [] :: S) L cbs fx) =
      .ok (.run (M (q + 1 + off) (.list [] :: S) L cbs fx)) := by
  cases fl <;> simp [Vm.exec, pop1, jumpTo_ok hq hs, show ¬ (q + 1 + off > P.length) by omega]

theorem exec_each_cons {P : Code} {venv q x rest S L cbs fx off} {fl : Bool} (hs : q + 1 + off ≤ P.length) :
    Vm.exec P.length venv (if fl then .filter off else .map off) (M q (.list (x :: rest) :: S) L cbs fx) =
      .ok (.run (M (q + 1) [] (L ++ [x, .num (F64.ofNat 0)])
        (mkCb fl (q + 1) (q + 1 + off) x 0 rest [] L.length S :: cbs) fx)) := by
  cases fl <;> simp [Vm.exec, pop1, mkCb, show ¬ (q + 1 + off > P.length) by omega]

theorem exec_each_list {P : Code} {venv q w S L cbs fx off s} {fl : Bool}
    (h : Vm.exec P.length venv (if fl then .filter off else .map off) (M q (w :: S) L cbs fx) = .ok s) :
    ∃ xs, w = .list xs := by
  cases w
  case list xs => exact ⟨xs, rfl⟩
  all_goals (exfalso; cases fl <;> simp [Vm.exec, pop1] at h <;> split at h <;> simp at h)

/-- The room of a callback body. -/
theorem body_room {P cl cb : Code} {i0 pc cbs L fl x i rest out S}
    (hr : Room P (cl ++ [i0] ++ cb) pc cbs L) :
    Room P cb (pc + cl.length + 1)
      (mkCb fl (pc + cl.length + 1) (pc + cl.length + 1 + cb.length) x i rest out L.length S :: cbs)
      (L ++ [x, .num (F64.ofNat i)]) := by
  have h := hr.right
  refine ⟨by simpa [Nat.add_assoc] using h.at_, fun c hc => ?_, by simpa [Nat.add_assoc] using h.fits,
    fun c hc => ?_⟩
  · simp at hc; subst hc; simp [mkCb]
  · simp at hc; subst hc; simp [mkCb]

/-- `map` and `filter`, together: the list, then the body after the opcode. -/
theorem case_each (ih : AllOk fuel) {fl : Bool} {l ps body cl tl cb tb}
    (hl0 : compile fuel p depth sc n l = .ok (cl, tl))
    (hb0 : compile fuel p depth (bindScope ps n tl.item sc) (n + 2) body = .ok (cb, tb))
    (hx : Ctx env inFn ls venv L p sc n)
    (hsem : ∀ v, EvalR env inFn ls (.call (if fl then "filter" else "map") [l, .arrow ps body]) v ↔
      ∃ xs ys, EvalR env inFn ls l (.list xs) ∧ v = .list ys ∧
        if fl then FilterR env inFn ls ps body xs 0 ys else MapR env inFn ls ps body xs 0 ys) :
    ExprSpec env inFn ls venv P L (.call (if fl then "filter" else "map") [l, .arrow ps body])
      (cl ++ [if fl then .filter cb.length else .map cb.length] ++ cb) (if fl then .list tl.item else .list tb) := by
  obtain rfl := hx.len
  intro pc S cbs fx hr
  have ihl := ih_expr (P := P) ih hl0 hx pc S cbs fx hr.left.left
  have hbody : ∀ x i, VTy env.prog.shapes x tl.item →
      ExprSpec env inFn (bindParams ps x i ls) venv P (L ++ [x, .num (F64.ofNat i)]) body cb tb :=
    fun x i hx' => ih_expr ih hb0 ⟨hx.prog, hx.agree.bind hx', by simp, hx.quiet⟩
  have hroom := fun x i rest out => body_room (fl := fl) (x := x) (i := i) (rest := rest) (out := out) (S := S) hr
  have hnil := hr.left.right.step (venv := venv) (S := .list [] :: S) (fx := fx)
    (exec_each_nil (fl := fl) (off := cb.length) (hr.top.mono (by simp; omega)) (by have := hr.fits; simp at this; omega))
  have hcons := fun x rest => hr.left.right.step (venv := venv) (S := .list (x :: rest) :: S) (fx := fx)
    (exec_each_cons (fl := fl) (x := x) (rest := rest) (off := cb.length) (by have := hr.fits; simp at this; omega))
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨xs, ys, h1, rfl, h2⟩ := (hsem v).mp hv
    obtain ⟨htl, hs1⟩ := ihl.1 _ h1
    have hitems := vty_item htl
    cases xs with
    | nil =>
      cases fl <;> simp at h2 <;> cases h2 <;>
        exact ⟨by simp [VTy, VTyAll], hs1.trans (hnil.pc (by simp; omega))⟩
    | cons x rest =>
      cases fl
      · simp only [Bool.false_eq_true, ite_false] at h2 ⊢
        obtain ⟨hty, hs2⟩ := (map_loop hbody hroom rfl rest x 0 [] hitems).1 ys h2
        exact ⟨hty, hs1.trans ((hcons x rest).trans (hs2.pc (by simp; omega)))⟩
      · simp only [ite_true] at h2 ⊢
        obtain ⟨hty, hs2⟩ := (filter_loop hbody hroom rfl rest x 0 [] hitems).1 ys h2
        exact ⟨hty, hs1.trans ((hcons x rest).trans (hs2.pc (by simp; omega)))⟩
  · obtain ⟨w, h1⟩ := ihl.2 hh
    obtain ⟨htl, hs1⟩ := ihl.1 w h1
    have hh1 := hh.star hs1
    obtain ⟨st, hst⟩ := hr.left.right.halts hh1
    obtain ⟨xs, rfl⟩ := exec_each_list hst
    have hitems := vty_item htl
    cases xs with
    | nil =>
      refine ⟨.list [], (hsem _).mpr ⟨[], [], h1, rfl, ?_⟩⟩
      cases fl
      · exact .nil
      · exact .nil
    | cons x rest =>
      have hh2 := hh1.star (hcons x rest)
      cases fl
      · obtain ⟨ys, hm⟩ := (map_loop hbody hroom rfl rest x 0 [] hitems).2 hh2
        exact ⟨.list ys, (hsem _).mpr ⟨_, ys, h1, rfl, hm⟩⟩
      · obtain ⟨ys, hm⟩ := (filter_loop hbody hroom rfl rest x 0 [] hitems).2 hh2
        exact ⟨.list ys, (hsem _).mpr ⟨_, ys, h1, rfl, hm⟩⟩

/-! ## `pending`, `failed` and the roster -/

theorem stdlib_now {e₁ e₂ : Contract.Env} {f vs} (h : e₁.now = e₂.now)
    (hr : e₁.prog.routes = e₂.prog.routes) : stdlib e₁ f vs = stdlib e₂ f vs := by
  unfold stdlib; rw [h, hr]

/-- What a call evaluates by, with its name a variable (so that a literal
name never meets the unifier). -/
theorem evalR_call_inv {name args v} (h : EvalR env inFn ls (.call name args) v) :
    (∃ fd vs, env.prog.fns.find? (·.name == name) = some fd ∧ ListR env inFn ls args vs ∧
      EvalR env true ((fd.params.map (·.1)).zip vs).reverse fd.body v) ∨
    (env.prog.fns.find? (·.name == name) = none ∧
      ((∃ l ps body xs ys, name = "map" ∧ args = [l, .arrow ps body] ∧ EvalR env inFn ls l (.list xs) ∧
          MapR env inFn ls ps body xs 0 ys ∧ v = .list ys) ∨
       (∃ l ps body xs ys, name = "filter" ∧ args = [l, .arrow ps body] ∧ EvalR env inFn ls l (.list xs) ∧
          FilterR env inFn ls ps body xs 0 ys ∧ v = .list ys) ∨
       (∃ x, (name = "pending" ∨ name = "failed") ∧ args = [.var x] ∧ v = .bool false ∧
          ((env.prog.resources.any (·.name == x) = true ∧ (lookup x env.resources).isSome = true) ∨
            env.prog.resources.any (·.name == x) = false)) ∨
       (∃ vs, ListR env inFn ls args vs ∧ stdlib env name vs = .ok v))) := by
  cases h
  case fn hfd hargs hbody => exact .inl ⟨_, _, hfd, hargs, hbody⟩
  case map hfd hl hm => exact .inr ⟨hfd, .inl ⟨_, _, _, _, _, rfl, rfl, hl, hm, rfl⟩⟩
  case filter hfd hl hm => exact .inr ⟨hfd, .inr (.inl ⟨_, _, _, _, _, rfl, rfl, hl, hm, rfl⟩)⟩
  case pendingSettled hfd h1 h2 => exact .inr ⟨hfd, .inr (.inr (.inl ⟨_, .inl rfl, rfl, rfl, .inl ⟨h1, h2⟩⟩))⟩
  case pendingOther hfd h1 => exact .inr ⟨hfd, .inr (.inr (.inl ⟨_, .inl rfl, rfl, rfl, .inr h1⟩))⟩
  case failedSettled hfd h1 h2 => exact .inr ⟨hfd, .inr (.inr (.inl ⟨_, .inr rfl, rfl, rfl, .inl ⟨h1, h2⟩⟩))⟩
  case failedOther hfd h1 => exact .inr ⟨hfd, .inr (.inr (.inl ⟨_, .inr rfl, rfl, rfl, .inr h1⟩))⟩
  case stdlib hfd hargs hs => exact .inr ⟨hfd, .inr (.inr (.inr ⟨_, hargs, hs⟩))⟩

/-- `pending(r)`/`failed(r)` of a resource: the flag, once it settled. -/
theorem spec_flag {e : Expr} {x : String} {j : Nat} {i : Instr} {flags : List Bool}
    (hflags : ∀ k : Nat, flags[k]?.getD false = false)
    (hev : ∀ v, EvalR env inFn ls e v ↔ v = .bool false ∧ (lookup x env.resources).isSome = true)
    (hiff : (lookup x env.resources).isSome = true ↔ ∃ w, venv.resources[j]? = some (some w))
    (hex : ∀ pc S cbs fx, (∀ w, venv.resources[j]? = some (some w) →
        Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (.bool (flags[j]?.getD false) :: S) L cbs fx))) ∧
      ((∀ w, venv.resources[j]? ≠ some (some w)) → ∀ s, Vm.exec P.length venv i (M pc S L cbs fx) ≠ .ok s)) :
    ExprSpec env inFn ls venv P L e [i] .bool := by
  intro pc S cbs fx hr
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨rfl, hs⟩ := (hev v).mp hv
    obtain ⟨w, hw⟩ := hiff.mp hs
    refine ⟨by simp [VTy], hr.step ?_⟩
    rw [(hex pc S cbs fx).1 w hw, hflags]
    simp
  · obtain ⟨st, hst⟩ := hr.halts hh
    by_cases hw : ∃ w, venv.resources[j]? = some (some w)
    · exact ⟨_, (hev _).mpr ⟨rfl, hiff.mpr hw⟩⟩
    · exact absurd hst ((hex pc S cbs fx).2 (fun w h => hw ⟨w, h⟩) st)

theorem exec_flag {P : Code} {venv j pc S L cbs fx} {flags : List Bool} {i : Instr}
    (hi : i = .pendingResource j ∧ flags = venv.pendingResources ∨ i = .failedResource j ∧ flags = venv.failedResources) :
    (∀ w, venv.resources[j]? = some (some w) →
        Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run (M (pc + 1) (.bool (flags[j]?.getD false) :: S) L cbs fx))) ∧
      ((∀ w, venv.resources[j]? ≠ some (some w)) → ∀ s, Vm.exec P.length venv i (M pc S L cbs fx) ≠ .ok s) := by
  rcases hi with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩ <;>
  refine ⟨fun w hw => by simp [Vm.exec, hw], fun hn s => ?_⟩ <;>
  simp only [Vm.exec] <;> (try split) <;> simp_all

theorem case_pending (hfd : p.fns.find? (·.name == "pending") = none)
    (hc : (match scopeLookup x sc with
      | some (.resource i, _) => Except.ok ([Instr.pendingResource i], STy.bool)
      | some (.mutation _ k, _) => Except.ok ([Instr.pendingMutation k], STy.bool)
      | _ => Except.error s!"`{x}` is not a resource or a mutation" : Except String (Code × STy)) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.call "pending" [.var x]) c t := by
  have hfd' : env.prog.fns.find? (·.name == "pending") = none := by rw [hx.prog]; exact hfd
  have hinv : ∀ v, EvalR env inFn ls (.call "pending" [.var x]) v ↔ v = .bool false ∧
      ((env.prog.resources.any (·.name == x) = true ∧ (lookup x env.resources).isSome = true) ∨
        env.prog.resources.any (·.name == x) = false) := by
    intro v
    constructor
    · intro h
      rcases evalR_call_inv h with ⟨_, _, h1, _⟩ | ⟨_, ⟨_, _, _, _, _, h1, _⟩ | ⟨_, _, _, _, _, h1, _⟩ |
        ⟨y, _, h2, h3, h4⟩ | ⟨_, _, h1⟩⟩
      · rw [hfd'] at h1; cases h1
      · simp at h1
      · simp at h1
      · simp at h2; subst h2; exact ⟨h3, h4⟩
      · exact absurd h1 stdlib_pending
    · rintro ⟨rfl, ⟨h1, h2⟩ | h1⟩
      · exact .pendingSettled hfd' h1 h2
      · exact .pendingOther hfd' h1
  split at hc
  · rename_i i t0 hl
    simp at hc; obtain ⟨rfl, rfl⟩ := hc
    obtain ⟨-, -, -, hany, hiff⟩ := hx.agree x _ _ hl
    refine spec_flag hx.quiet.pendingResources (fun v => ?_) hiff (fun _ _ _ _ => exec_flag (.inl ⟨rfl, rfl⟩))
    rw [hinv]; simp [hany]
  · rename_i s k t0 hl
    simp at hc; obtain ⟨rfl, rfl⟩ := hc
    obtain ⟨-, -, -, hany⟩ := hx.agree x _ _ hl
    refine spec_push (w := .bool false) (fun v => ?_) (by simp [VTy])
      (fun _ _ _ _ => by simp [Vm.exec, hx.quiet.pendingMutations])
    rw [hinv]; simp [hany]
  · cases hc

theorem case_failed (hfd : p.fns.find? (·.name == "failed") = none)
    (hc : (match scopeLookup x sc with
      | some (.resource i, _) => Except.ok ([Instr.failedResource i], STy.bool)
      | _ => Except.error s!"`{x}` is not a resource" : Except String (Code × STy)) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.call "failed" [.var x]) c t := by
  have hfd' : env.prog.fns.find? (·.name == "failed") = none := by rw [hx.prog]; exact hfd
  have hinv : ∀ v, EvalR env inFn ls (.call "failed" [.var x]) v ↔ v = .bool false ∧
      ((env.prog.resources.any (·.name == x) = true ∧ (lookup x env.resources).isSome = true) ∨
        env.prog.resources.any (·.name == x) = false) := by
    intro v
    constructor
    · intro h
      rcases evalR_call_inv h with ⟨_, _, h1, _⟩ | ⟨_, ⟨_, _, _, _, _, h1, _⟩ | ⟨_, _, _, _, _, h1, _⟩ |
        ⟨y, _, h2, h3, h4⟩ | ⟨_, _, h1⟩⟩
      · rw [hfd'] at h1; cases h1
      · simp at h1
      · simp at h1
      · simp at h2; subst h2; exact ⟨h3, h4⟩
      · exact absurd h1 stdlib_failed
    · rintro ⟨rfl, ⟨h1, h2⟩ | h1⟩
      · exact .failedSettled hfd' h1 h2
      · exact .failedOther hfd' h1
  split at hc
  · rename_i i t0 hl
    simp at hc; obtain ⟨rfl, rfl⟩ := hc
    obtain ⟨-, -, -, hany, hiff⟩ := hx.agree x _ _ hl
    refine spec_flag hx.quiet.failedResources (fun v => ?_) hiff (fun _ _ _ _ => exec_flag (.inr ⟨rfl, rfl⟩))
    rw [hinv]; simp [hany]
  · cases hc

/-- Any other roster entry, on its arguments. -/
theorem case_stdlib (ih : AllOk fuel) {name args ca ts} (hfd : p.fns.find? (·.name == name) = none)
    (hname : ¬ (name = "map" ∨ name = "filter" ∨ name = "pending" ∨ name = "failed"))
    (ha : compileArgs fuel p depth sc n args = .ok (ca, ts))
    (hx : Ctx env inFn ls venv L p sc n) :
    ExprSpec env inFn ls venv P L (.call name args) (ca ++ [.call name args.length]) (rosterTy name ts) := by
  have hfd' : env.prog.fns.find? (·.name == name) = none := by rw [hx.prog]; exact hfd
  have hinv : ∀ v, EvalR env inFn ls (.call name args) v ↔
      ∃ vs, ListR env inFn ls args vs ∧ stdlib env name vs = .ok v := by
    intro v
    constructor
    · intro h
      rcases evalR_call_inv h with ⟨_, _, h1, _⟩ | ⟨_, ⟨_, _, _, _, _, h1, _⟩ | ⟨_, _, _, _, _, h1, _⟩ |
        ⟨y, h2, _⟩ | h1⟩
      · rw [hfd'] at h1; cases h1
      · exact absurd (.inl h1) hname
      · exact absurd (.inr (.inl h1)) hname
      · rcases h2 with h2 | h2
        · exact absurd (.inr (.inr (.inl h2))) hname
        · exact absurd (.inr (.inr (.inr h2))) hname
      · exact h1
    · rintro ⟨vs, h1, h2⟩; exact .stdlib hfd' h1 h2
  intro pc S cbs fx hr
  have iha := ih.2.1 _ _ _ _ _ _ _ ha _ _ _ _ _ P hx pc S cbs fx hr.left
  have hcall : ∀ vs, vs.length = args.length →
      Vm.exec P.length venv (.call name args.length) (M (pc + ca.length) (vs.reverse ++ S) L cbs fx) =
        match stdlib env name vs with
        | .ok v => .ok (.run (M (pc + ca.length + 1) (v :: S) L cbs fx))
        | .error e => .error (.call e) := by
    intro vs hl
    rw [← hl, ← stdlib_now (e₁ := callEnv venv) (by simp [callEnv, hx.quiet.now])
      (by simp [callEnv, hx.quiet.routes])]
    simp only [Vm.exec, popN_ok]
    cases stdlib (callEnv venv) name vs <;> rfl
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨vs, h1, h2⟩ := (hinv v).mp hv
    obtain ⟨hts, hs1⟩ := iha.1 vs h1
    have := hr.right.step (venv := venv) (S := vs.reverse ++ S) (fx := fx)
      (by rw [hcall vs (ListR.length h1), h2])
    exact ⟨stdlib_ty h2 hts, hs1.trans (this.pc (by simp <;> omega))⟩
  · obtain ⟨vs, h1⟩ := iha.2 hh
    obtain ⟨_, hs1⟩ := iha.1 vs h1
    obtain ⟨st, hst⟩ := hr.right.halts (hh.star hs1)
    rw [hcall vs (ListR.length h1)] at hst
    cases h2 : stdlib env name vs with
    | error e => rw [h2] at hst; cases hst
    | ok v => exact ⟨v, (hinv v).mpr ⟨vs, h1, h2⟩⟩

/-- `[a, b]` (LLP 1088 §9.1): the items, left to right, then `List n`. -/
theorem case_list (ih : AllOk fuel) {items} (hc : compile (fuel + 1) p depth sc n (.list items) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.list items) c t := by
  rw [compile.eq_def] at hc
  simp only [Except.bind_ok_iff] at hc
  obtain ⟨⟨ca, ts⟩, ha, h1⟩ := hc
  simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
  have hinv : ∀ v, EvalR env inFn ls (.list items) v ↔ ∃ vs, ListR env inFn ls items vs ∧ v = .list vs := by
    intro v
    constructor
    · intro h; cases h with | list h => exact ⟨_, h, rfl⟩
    · rintro ⟨vs, h, rfl⟩; exact .list h
  intro pc S cbs fx hr
  have iha := ih.2.1 _ _ _ _ _ _ _ ha _ _ _ _ _ P hx pc S cbs fx hr.left
  have hlist : ∀ vs : List Value, vs.length = items.length →
      Vm.exec P.length venv (.list items.length) (M (pc + ca.length) (vs.reverse ++ S) L cbs fx) =
        .ok (.run (M (pc + ca.length + 1) (.list vs :: S) L cbs fx)) := by
    intro vs hl
    rw [← hl]
    simp [Vm.exec, popN_ok]
  refine ⟨fun v hv => ?_, fun hh => ?_⟩
  · obtain ⟨vs, h1, rfl⟩ := (hinv v).mp hv
    obtain ⟨hts, hs1⟩ := iha.1 vs h1
    have := hr.right.step (venv := venv) (S := vs.reverse ++ S) (fx := fx) (hlist vs (ListR.length h1))
    exact ⟨VTys.joinAll hts, hs1.trans (this.pc (by simp <;> omega))⟩
  · obtain ⟨vs, h1⟩ := iha.2 hh
    exact ⟨.list vs, (hinv _).mpr ⟨vs, h1, rfl⟩⟩

end Contract.Lower
