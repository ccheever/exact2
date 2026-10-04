/-
Facts about the VM model: runs as a relation, determinism, code placed in
a program, and the single steps the compiled code takes.
-/
import Contract.Vm
import Contract.Big

namespace Contract.Vm

/-- Zero or more steps that continue. -/
inductive Star (P : Code) (env : Env) : Machine → Machine → Prop
  | refl : Star P env m m
  | cons : Vm.step P env m = .ok (.run m') → Star P env m' m'' → Star P env m m''

/-- A run from `m` that returns `v` with effects `fx`. -/
inductive Runs (P : Code) (env : Env) : Machine → Value → Effects → Prop
  | done : Vm.step P env m = .ok (.done v fx) → Runs P env m v fx
  | next : Vm.step P env m = .ok (.run m') → Runs P env m' v fx → Runs P env m v fx

/-- The run from `m` returns. -/
def Halts (P : Code) (env : Env) (m : Machine) : Prop := ∃ v fx, Runs P env m v fx

theorem Star.trans {P env m₁ m₂ m₃} (h₁ : Star P env m₁ m₂) (h₂ : Star P env m₂ m₃) :
    Star P env m₁ m₃ := by
  induction h₁ with
  | refl => exact h₂
  | cons h _ ih => exact .cons h (ih h₂)

theorem Star.one {P env m m'} (h : step P env m = .ok (.run m')) : Star P env m m' :=
  .cons h .refl

theorem Halts.next {P env m m'} (h : Halts P env m) (hs : step P env m = .ok (.run m')) :
    Halts P env m' := by
  obtain ⟨v, fx, hr⟩ := h
  cases hr with
  | done hd => rw [hs] at hd; cases hd
  | next hn hr => rw [hs] at hn; cases hn; exact ⟨v, fx, hr⟩

theorem Halts.star {P env m m'} (h : Halts P env m) (hs : Star P env m m') : Halts P env m' := by
  induction hs with
  | refl => exact h
  | cons hs _ ih => exact ih (h.next hs)

/-- A run that returns never traps on the way. -/
theorem Halts.not_error {P env m t} (h : Halts P env m) : step P env m ≠ .error t := by
  obtain ⟨v, fx, hr⟩ := h
  cases hr with
  | done hd => rw [hd]; intro h; cases h
  | next hn _ => rw [hn]; intro h; cases h

/-- Where a returning run goes next when its step is computed. -/
theorem Halts.inv {P env m} (h : Halts P env m) :
    (∃ m', step P env m = .ok (.run m')) ∨ (∃ v fx, step P env m = .ok (.done v fx)) := by
  obtain ⟨v, fx, hr⟩ := h
  cases hr with
  | done hd => exact .inr ⟨_, _, hd⟩
  | next hn _ => exact .inl ⟨_, hn⟩

theorem Runs.of_star {P env m m' v fx} (hs : Star P env m m') (hr : Runs P env m' v fx) :
    Runs P env m v fx := by
  induction hs with
  | refl => exact hr
  | cons h _ ih => exact .next h (ih hr)

theorem Runs.det {P env m v fx w gx} (h₁ : Runs P env m v fx) (h₂ : Runs P env m w gx) :
    v = w ∧ fx = gx := by
  induction h₁ with
  | done hd =>
    cases h₂ with
    | done hd' => rw [hd] at hd'; cases hd'; exact ⟨rfl, rfl⟩
    | next hn _ => rw [hd] at hn; cases hn
  | next hn _ ih =>
    cases h₂ with
    | done hd' => rw [hn] at hd'; cases hd'
    | next hn' hr' => rw [hn] at hn'; cases hn'; exact ih hr'

theorem runFrom_runs {P env n m v fx} (h : runFrom P env n m = .ok (v, fx)) : Runs P env m v fx := by
  induction n generalizing m with
  | zero => simp [runFrom] at h
  | succ n ih =>
    simp only [runFrom] at h
    split at h
    · cases h
    · rename_i hs; cases h; exact .done hs
    · rename_i hs; exact .next hs (ih h)

theorem runs_runFrom {P env m v fx} (h : Runs P env m v fx) : ∃ n, runFrom P env n m = .ok (v, fx) := by
  induction h with
  | done hd => exact ⟨1, by simp [runFrom, hd]⟩
  | next hn _ ih => obtain ⟨n, hn'⟩ := ih; exact ⟨n + 1, by simp [runFrom, hn, hn']⟩

/-! ## Code in a program -/

/-- `c` sits at `pc` in `P`. -/
def At (P c : Code) (pc : Nat) : Prop := ∀ i, i < c.length → P[pc + i]? = c[i]?

theorem At.append {P c₁ c₂ pc} : At P (c₁ ++ c₂) pc ↔ At P c₁ pc ∧ At P c₂ (pc + c₁.length) := by
  constructor
  · intro h
    refine ⟨fun i hi => ?_, fun i hi => ?_⟩
    · have := h i (by simp; omega)
      rw [this, List.getElem?_append_left hi]
    · have := h (c₁.length + i) (by simp; omega)
      rw [List.getElem?_append_right (by omega)] at this
      simpa [Nat.add_assoc] using this
  · intro ⟨h₁, h₂⟩ i hi
    by_cases hlt : i < c₁.length
    · rw [h₁ i hlt, List.getElem?_append_left hlt]
    · have := h₂ (i - c₁.length) (by simp at hi; omega)
      rw [List.getElem?_append_right (by omega)]
      rw [← this]; congr 1; omega

theorem At.cons {P i c pc} : At P (i :: c) pc ↔ P[pc]? = some i ∧ At P c (pc + 1) := by
  have := @At.append P [i] c pc
  simp only [List.singleton_append] at this
  rw [this]
  simp [At]

theorem At.left {P c₁ c₂ pc} (h : At P (c₁ ++ c₂) pc) : At P c₁ pc := (At.append.mp h).1
theorem At.right {P c₁ c₂ pc} (h : At P (c₁ ++ c₂) pc) : At P c₂ (pc + c₁.length) := (At.append.mp h).2
theorem At.head {P i c pc} (h : At P (i :: c) pc) : P[pc]? = some i := (At.cons.mp h).1
theorem At.tail {P i c pc} (h : At P (i :: c) pc) : At P c (pc + 1) := (At.cons.mp h).2

theorem At.single {P i pc} (h : At P [i] pc) : P[pc]? = some i := h.head

theorem At.fits {P c pc} (h : At P c pc) (hne : c ≠ []) : pc + c.length ≤ P.length := by
  have := h (c.length - 1) (by cases c <;> simp_all)
  have hlt : c.length - 1 < c.length := by cases c <;> simp_all
  rw [List.getElem?_eq_getElem hlt] at this
  have := (List.getElem?_eq_some_iff.mp this).1
  omega

/-! ## Single steps -/

/-- No callback body ends before `q`. -/
def TopOk (cbs : List Callback) (q : Nat) : Prop := ∀ c, cbs.head? = some c → q ≤ c.stop

theorem TopOk.mono {cbs q q'} (h : TopOk cbs q) (hle : q' ≤ q) : TopOk cbs q' :=
  fun c hc => Nat.le_trans hle (h c hc)

/-- At a `pc` before the innermost body's end, a step runs the instruction. -/
theorem step_at {P env pc S L cbs fx i q} (hat : P[pc]? = some i) (hq : TopOk cbs q) (hlt : pc < q) :
    step P env ⟨pc, S, L, cbs, fx⟩ = exec P.length env i ⟨pc, S, L, cbs, fx⟩ := by
  unfold step
  cases cbs with
  | nil => simp [hat]
  | cons c cbs =>
    have := hq c rfl
    have hne : c.stop ≠ pc := by omega
    simp [hne, hat]

theorem jumpTo_ok {size cbs t} (hq : TopOk cbs t) (hs : t ≤ size) : jumpTo size cbs t = .ok t := by
  unfold jumpTo
  cases cbs with
  | nil => simp [show ¬ t > size by omega]
  | cons c cbs =>
    have := hq c rfl
    simp [show ¬ t > c.stop by omega, show ¬ t > size by omega]

theorem popN_ok (vs S : List Value) : popN vs.length (vs.reverse ++ S) = some (vs, S) := by
  simp [popN]

/-- Locals at least as deep as the innermost callback's own two. -/
def LocalsOk (cbs : List Callback) (L : List Value) : Prop := ∀ c, cbs.head? = some c → c.base + 2 ≤ L.length

theorem LocalsOk.append {cbs L} (h : LocalsOk cbs L) (ws : List Value) : LocalsOk cbs (L ++ ws) :=
  fun c hc => by have := h c hc; simp; omega

/-- A `DropLocal` after a `BindLocal`. -/
theorem exec_dropLocal {size env pc S L cbs fx w} (hl : LocalsOk cbs L) :
    exec size env .dropLocal ⟨pc, S, L ++ [w], cbs, fx⟩ = .ok (.run ⟨pc + 1, S, L, cbs, fx⟩) := by
  unfold exec
  cases cbs with
  | nil => simp
  | cons c cbs =>
    have := hl c rfl
    have : ¬ (L.length + 1 ≤ c.base + 2) := by omega
    simp [this]

end Contract.Vm
