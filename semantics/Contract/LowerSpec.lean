/-
What each compiled piece must do, and the stepping lemmas the proofs of
`Contract.LowerProof` are made of.
-/
import Contract.LowerSim

namespace Contract.Lower

open Vm

/-- The machine at `pc`. -/
abbrev M (pc : Nat) (S L : List Value) (cbs : List Callback) (fx : Vm.Effects) : Machine := ⟨pc, S, L, cbs, fx⟩

/-- Where code `c` runs: at `pc` in the program, before the innermost
callback body ends, with that body's locals in force. -/
structure Room (P c : Code) (pc : Nat) (cbs : List Callback) (L : List Value) : Prop where
  at_ : At P c pc
  top : TopOk cbs (pc + c.length)
  fits : pc + c.length ≤ P.length
  locals : LocalsOk cbs L

theorem Room.left {P c₁ c₂ pc cbs L} (h : Room P (c₁ ++ c₂) pc cbs L) : Room P c₁ pc cbs L :=
  ⟨h.at_.left, h.top.mono (by simp), by have := h.fits; simp at this; omega, h.locals⟩

theorem Room.right {P c₁ c₂ pc cbs L} (h : Room P (c₁ ++ c₂) pc cbs L) :
    Room P c₂ (pc + c₁.length) cbs L :=
  ⟨h.at_.right, h.top.mono (by simp; omega), by have := h.fits; simp at this; omega, h.locals⟩

theorem Room.locals_append {P c pc cbs L} (h : Room P c pc cbs L) (ws : List Value) :
    Room P c pc cbs (L ++ ws) := ⟨h.at_, h.top, h.fits, h.locals.append ws⟩

theorem Room.tail {P i c pc cbs L} (h : Room P (i :: c) pc cbs L) : Room P c (pc + 1) cbs L := by
  have := Room.right (c₁ := [i]) (c₂ := c) h
  simpa using this

theorem Room.first {P i c pc cbs L} (h : Room P (i :: c) pc cbs L) : Room P [i] pc cbs L :=
  Room.left (c₁ := [i]) (c₂ := c) h

/-- The instruction at the start of the room. -/
theorem Room.head {P i c pc cbs L} (h : Room P (i :: c) pc cbs L) : P[pc]? = some i := h.at_.head

theorem Room.lt {P i c pc cbs L} (h : Room P (i :: c) pc cbs L) : TopOk cbs (pc + 1) :=
  h.top.mono (by simp)

/-- One instruction in the room, run. -/
theorem Room.step {P venv i c pc S L cbs fx m'} (h : Room P (i :: c) pc cbs L)
    (he : Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run m')) : Star P venv (M pc S L cbs fx) m' :=
  Star.one (by rw [step_at h.head h.lt (by omega)]; exact he)

/-- One instruction in the room, on a run that returns. -/
theorem Room.halts {P venv i c pc S L cbs fx} (h : Room P (i :: c) pc cbs L)
    (hh : Halts P venv (M pc S L cbs fx)) :
    ∃ s, Vm.exec P.length venv i (M pc S L cbs fx) = .ok s := by
  have hs := step_at (env := venv) (S := S) (L := L) (fx := fx) h.head h.lt (by omega)
  cases he : Vm.exec P.length venv i (M pc S L cbs fx) with
  | error t => exact absurd (hs.trans he) hh.not_error
  | ok s => exact ⟨s, rfl⟩

theorem Room.next {P venv i c pc S L cbs fx m'} (h : Room P (i :: c) pc cbs L)
    (hh : Halts P venv (M pc S L cbs fx)) (he : Vm.exec P.length venv i (M pc S L cbs fx) = .ok (.run m')) :
    Halts P venv m' := hh.star (h.step he)

/-! ## Jumps -/

theorem exec_jump {P : Code} {venv q S L cbs fx off e} (hq : TopOk cbs e) (hfit : e ≤ P.length) (ht : q + 1 + off ≤ e) :
    Vm.exec P.length venv (.jump off) (M q S L cbs fx) = .ok (.run (M (q + 1 + off) S L cbs fx)) := by
  simp [Vm.exec, jumpTo_ok (hq.mono ht) (by omega : q + 1 + off ≤ P.length)]

theorem exec_jumpIfFalse_false {P : Code} {venv q S L cbs fx off e} (hq : TopOk cbs e) (hfit : e ≤ P.length)
    (ht : q + 1 + off ≤ e) :
    Vm.exec P.length venv (.jumpIfFalse off) (M q (.bool false :: S) L cbs fx) =
      .ok (.run (M (q + 1 + off) S L cbs fx)) := by
  simp [Vm.exec, pop1, jumpTo_ok (hq.mono ht) (by omega : q + 1 + off ≤ P.length)]

theorem exec_jumpIfFalse_true {P : Code} {venv q S L cbs fx off} :
    Vm.exec P.length venv (.jumpIfFalse off) (M q (.bool true :: S) L cbs fx) =
      .ok (.run (M (q + 1) S L cbs fx)) := by
  simp [Vm.exec, pop1]

/-- `JumpIfFalse` on anything but a bool traps. -/
theorem exec_jumpIfFalse_bool {P : Code} {venv q w S L cbs fx off s}
    (h : Vm.exec P.length venv (.jumpIfFalse off) (M q (w :: S) L cbs fx) = .ok s) : ∃ b, w = .bool b := by
  cases w <;> simp [Vm.exec, pop1] at h
  exact ⟨_, rfl⟩

theorem exec_jumpIfNone_none {P : Code} {venv q S L cbs fx off e} (hq : TopOk cbs e) (hfit : e ≤ P.length)
    (ht : q + 1 + off ≤ e) :
    Vm.exec P.length venv (.jumpIfNone off) (M q (.none :: S) L cbs fx) =
      .ok (.run (M (q + 1 + off) (.none :: S) L cbs fx)) := by
  simp [Vm.exec, jumpTo_ok (hq.mono ht) (by omega : q + 1 + off ≤ P.length)]

theorem exec_jumpIfNone_some {P : Code} {venv q w S L cbs fx off} :
    Vm.exec P.length venv (.jumpIfNone off) (M q (.some w :: S) L cbs fx) =
      .ok (.run (M (q + 1) (.some w :: S) L cbs fx)) := by
  simp [Vm.exec]

/-- `JumpIfNone` on anything but an option traps. -/
theorem exec_jumpIfNone_opt {P : Code} {venv q w S L cbs fx off s}
    (h : Vm.exec P.length venv (.jumpIfNone off) (M q (w :: S) L cbs fx) = .ok s) :
    w = .none ∨ ∃ u, w = .some u := by
  cases w <;> simp [Vm.exec] at h
  · exact .inl rfl
  · exact .inr ⟨_, rfl⟩

theorem exec_bindLocal {P : Code} {venv q w S L cbs fx} :
    Vm.exec P.length venv .bindLocal (M q (w :: S) L cbs fx) = .ok (.run (M (q + 1) S (L ++ [w]) cbs fx)) := by
  simp [Vm.exec, pop1]

theorem exec_loadLocal {P : Code} {venv q S L cbs fx j w} (h : L[j]? = some w) :
    Vm.exec P.length venv (.loadLocal j) (M q S L cbs fx) = .ok (.run (M (q + 1) (w :: S) L cbs fx)) := by
  simp [Vm.exec, h]

theorem exec_unwrap {P : Code} {venv q w S L cbs fx} :
    Vm.exec P.length venv .unwrap (M q (.some w :: S) L cbs fx) = .ok (.run (M (q + 1) (w :: S) L cbs fx)) := by
  simp [Vm.exec, pop1]

theorem exec_pop {P : Code} {venv q w S L cbs fx} :
    Vm.exec P.length venv .pop (M q (w :: S) L cbs fx) = .ok (.run (M (q + 1) S L cbs fx)) := by
  simp [Vm.exec, pop1]

theorem exec_not {P : Code} {venv q b S L cbs fx} :
    Vm.exec P.length venv .not (M q (.bool b :: S) L cbs fx) = .ok (.run (M (q + 1) (.bool (!b) :: S) L cbs fx)) := by
  simp [Vm.exec, pop1]

theorem exec_not_bool {P : Code} {venv q w S L cbs fx s}
    (h : Vm.exec P.length venv .not (M q (w :: S) L cbs fx) = .ok s) : ∃ b, w = .bool b := by
  cases w <;> simp [Vm.exec, pop1] at h
  exact ⟨_, rfl⟩

theorem exec_drop {P : Code} {venv q S L cbs fx w} (hl : LocalsOk cbs L) :
    Vm.exec P.length venv .dropLocal (M q S (L ++ [w]) cbs fx) = .ok (.run (M (q + 1) S L cbs fx)) :=
  exec_dropLocal hl

/-! ## Specifications -/

/-- The code of an expression: from `S`, it leaves exactly `eval`'s value
on top, of the static type; and a run through it that returns had a value. -/
def ExprSpec (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (P : Code)
    (L : List Value) (e : Expr) (c : Code) (t : STy) : Prop :=
  ∀ pc S cbs fx, Room P c pc cbs L →
    (∀ v, EvalR env inFn ls e v → VTy env.prog.shapes v t ∧
      Star P venv (M pc S L cbs fx) (M (pc + c.length) (v :: S) L cbs fx)) ∧
    (Halts P venv (M pc S L cbs fx) → ∃ v, EvalR env inFn ls e v)

/-- Arguments: each left on the stack. -/
def ArgsSpec (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (P : Code)
    (L : List Value) (es : List Expr) (c : Code) (ts : List STy) : Prop :=
  ∀ pc S cbs fx, Room P c pc cbs L →
    (∀ vs, ListR env inFn ls es vs → VTys env.prog.shapes vs ts ∧
      Star P venv (M pc S L cbs fx) (M (pc + c.length) (vs.reverse ++ S) L cbs fx)) ∧
    (Halts P venv (M pc S L cbs fx) → ∃ vs, ListR env inFn ls es vs)

/-- A `fn`'s arguments: each bound as the next local. -/
def BindSpec (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (P : Code)
    (L : List Value) (es : List Expr) (c : Code) (ts : List STy) : Prop :=
  ∀ pc S cbs fx, Room P c pc cbs L →
    (∀ vs, ListR env inFn ls es vs → VTys env.prog.shapes vs ts ∧
      Star P venv (M pc S L cbs fx) (M (pc + c.length) S (L ++ vs) cbs fx)) ∧
    (Halts P venv (M pc S L cbs fx) → ∃ vs, ListR env inFn ls es vs)

/-- A template's parts: the first leaves its text, each later one is
concatenated to the text so far. -/
def PartsSpec (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (P : Code)
    (L : List Value) (first : Bool) (es : List Expr) (c : Code) : Prop :=
  ∀ pc S cbs fx, Room P c pc cbs L →
    if first then es ≠ [] →
      (∀ ss, DisplaysR env inFn ls es ss →
        Star P venv (M pc S L cbs fx) (M (pc + c.length) (.str (String.join ss) :: S) L cbs fx)) ∧
      (Halts P venv (M pc S L cbs fx) → ∃ ss, DisplaysR env inFn ls es ss)
    else ∀ acc,
      (∀ ss, DisplaysR env inFn ls es ss →
        Star P venv (M pc (.str acc :: S) L cbs fx)
          (M (pc + c.length) (.str (acc ++ String.join ss) :: S) L cbs fx)) ∧
      (Halts P venv (M pc (.str acc :: S) L cbs fx) → ∃ ss, DisplaysR env inFn ls es ss)

/-- The base of a record as the code and the semantics hold it. -/
def BaseOk (sh : List Shape) (L : List Value) : Option (Nat × String) → Option (List Value) → Prop
  | .none, .none => True
  | .some (b, s'), .some bs => L[b]? = some (.record s' bs) ∧ VTy sh (.record s' bs) (.record s')
  | _, _ => False

/-- A record's fields from the `i`th declared. -/
def FieldsSpec (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (P : Code)
    (L : List Value) (written : List (String × Expr)) (bv : Option (List Value)) (fs : List Field)
    (i : Nat) (c : Code) (ok : Bool) : Prop :=
  ∀ pc S cbs fx, Room P c pc cbs L →
    (∀ vs, FieldsR env inFn ls written bv fs i vs →
      (ok = true → VTys env.prog.shapes vs (fs.map fun f => STy.ofTy f.ty)) ∧
      Star P venv (M pc S L cbs fx) (M (pc + c.length) (vs.reverse ++ S) L cbs fx)) ∧
    (Halts P venv (M pc S L cbs fx) → ∃ vs, FieldsR env inFn ls written bv fs i vs)

/-- The context a compiled piece runs in: the scope corresponds, `n`
locals are in force, nothing is in flight. -/
structure Ctx (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (L : List Value)
    (p : Program) (sc : Scope) (n : Nat) : Prop where
  prog : env.prog = p
  agree : Agree env inFn ls venv L sc
  len : L.length = n
  quiet : Quiet env venv

def CompileOk (fuel : Nat) : Prop :=
  ∀ p depth sc n e c t, compile fuel p depth sc n e = .ok (c, t) →
  ∀ env inFn ls venv L P, Ctx env inFn ls venv L p sc n → ExprSpec env inFn ls venv P L e c t

def ArgsOk (fuel : Nat) : Prop :=
  ∀ p depth sc n es c ts, compileArgs fuel p depth sc n es = .ok (c, ts) →
  ∀ env inFn ls venv L P, Ctx env inFn ls venv L p sc n → ArgsSpec env inFn ls venv P L es c ts

def BindOk (fuel : Nat) : Prop :=
  ∀ p depth sc n es c ts, compileBind fuel p depth sc n es = .ok (c, ts) →
  ∀ env inFn ls venv L P, Ctx env inFn ls venv L p sc n → BindSpec env inFn ls venv P L es c ts

def PartsOk (fuel : Nat) : Prop :=
  ∀ p depth sc n first es c, compileParts fuel p depth sc n first es = .ok c →
  ∀ env inFn ls venv L P, Ctx env inFn ls venv L p sc n → PartsSpec env inFn ls venv P L first es c

def FieldsOk (fuel : Nat) : Prop :=
  ∀ p depth sc n written base shape fs i c ok,
  compileFields fuel p depth sc n written base shape fs i = .ok (c, ok) →
  ∀ env inFn ls venv L P bv, Ctx env inFn ls venv L p sc n →
  BaseOk env.prog.shapes L base bv →
  (∀ b s', base = .some (b, s') → s' = shape → ∃ d, p.shapes.find? (·.name == shape) = .some d ∧
    d.fields.drop i = fs) →
  FieldsSpec env inFn ls venv P L written bv fs i c ok

/-- Everything at one fuel. -/
def AllOk (fuel : Nat) : Prop :=
  CompileOk fuel ∧ ArgsOk fuel ∧ BindOk fuel ∧ PartsOk fuel ∧ FieldsOk fuel

theorem Ctx.push {env inFn ls venv L p sc n x w t} (hx : Ctx env inFn ls venv L p sc n)
    (hw : VTy env.prog.shapes w t) :
    Ctx env inFn ((x, w) :: ls) venv (L ++ [w]) p ((x, .local n, t) :: sc) (n + 1) :=
  ⟨hx.prog, by have := hx.agree.push (x := x) hw; rwa [hx.len] at this, by simp [hx.len], hx.quiet⟩

theorem Ctx.append {env inFn ls venv L p sc n} (hx : Ctx env inFn ls venv L p sc n) (ws : List Value) :
    Ctx env inFn ls venv (L ++ ws) p sc (n + ws.length) :=
  ⟨hx.prog, hx.agree.append ws, by simp [hx.len], hx.quiet⟩


theorem _root_.Contract.Vm.Star.pc {P venv m a b S L cbs fx} (h : Star P venv m (M a S L cbs fx))
    (hab : a = b) : Star P venv m (M b S L cbs fx) := hab ▸ h

theorem _root_.Contract.Vm.Star.join {P venv m₁ m₃ a b S L cbs fx} (h₁ : Star P venv m₁ (M a S L cbs fx))
    (h₂ : Star P venv (M b S L cbs fx) m₃) (hab : a = b) : Star P venv m₁ m₃ := by
  subst hab; exact h₁.trans h₂

theorem _root_.Contract.Vm.Halts.pc {P venv a b S L cbs fx} (h : Halts P venv (M a S L cbs fx))
    (hab : a = b) : Halts P venv (M b S L cbs fx) := hab ▸ h

theorem allOk_zero : AllOk 0 := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · intro _ _ _ _ _ _ _ h; simp [compile] at h
  · intro _ _ _ _ _ _ _ h; simp [compileArgs] at h
  · intro _ _ _ _ _ _ _ h; simp [compileBind] at h
  · intro _ _ _ _ _ _ _ h; simp [compileParts] at h
  · intro _ _ _ _ _ _ _ _ _ _ _ h; simp [compileFields] at h

end Contract.Lower
