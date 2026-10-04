/-
Update Lab (apps/update-lab/app.contract): a counter, a note, a pause
switch, and a timer that samples three data sources every 250 ms.
-/
import Apps.UpdateLab
open Contract

namespace UpdateLab

/-- **The live probe never touches what the user set.** However the
clock moves, in any reachable configuration, the 250 ms `sample` timer
leaves the counter, the note and the pause switch as they were (it only
sends). -/
theorem advance_keeps_user_state {c o t c' out} (hc : Reachable updateLab c)
    (h : advance updateLab o c t = (c', out)) :
    ∀ x ∈ ["counter", "note", "live"], lookup x c'.slots = lookup x c.slots := by
  intro x hx
  refine hc.advance_untouched (fun a ha hname => ?_) h
  simp only [updateLab, List.mem_cons, List.mem_nil_iff, or_false] at ha hname hx
  rcases hx with rfl | rfl | rfl <;>
    rcases ha with rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> simp_all <;> decide

/-- A result slot holds no answer yet, or one. -/
def IsResult (v : Value) : Prop := v = .none ∨ ∃ w, v = .some w

/-- **A probe's result is always absent or an answer.** No action assigns
the three mutation slots; they start `none` and only a send's answer,
wrapped in `some`, lands in them. -/
theorem results_shape : ∀ m ∈ ["tsResult", "rustResult", "rustExecutorResult"],
    ∀ c, Reachable updateLab c → SlotIn m IsResult c.slots := by
  intro m hm
  have hn : ∀ a ∈ updateLab.actions, Stmt.noAssigns m a.body = true := by
    simp only [List.mem_cons, List.mem_nil_iff, or_false] at hm
    rcases hm with rfl | rfl | rfl <;> decide
  refine Reachable.slotIn ?_ (fun _ _ _ _ _ _ _ _ _ _ _ _ => .of_noAssign hn fun v => .inr ⟨v, rfl⟩)
    (fun _ _ _ _ => .of_noAssign hn fun v => .inr ⟨v, rfl⟩)
  rintro v (⟨st, hst, hname, -⟩ | ⟨_, -, -, rfl⟩ | ⟨hr, -⟩)
  · simp only [updateLab, List.mem_cons, List.mem_nil_iff, or_false] at hst
    simp only [List.mem_cons, List.mem_nil_iff, or_false] at hm
    rcases hst with rfl | rfl | rfl <;> rcases hm with rfl | rfl | rfl <;> simp at hname
  · exact .inl rfl
  · simp [updateLab] at hr

/-- IEEE-754 binary64 one, as the embedding writes the literal `1`. -/
def one : F64 := F64.ofBits 0x3ff0000000000000

/-- **`increment` adds one.** A committed `increment` leaves the counter
at its old value plus one (in binary64; no other slot is written). Proved
with the weakest precondition of its body and lifted with
`runAction_commit`. -/
theorem increment_adds_one {o c rows c' out r} (hc : lookup "counter" c.slots = .some (.num r))
    (h : runAction updateLab o c "increment" [] rows = (c', out)) (hout : ∀ e, out ≠ .refused e) :
    lookup "counter" c'.slots = .some (.num (r + one)) := by
  obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h hout
  simp [updateLab] at ha
  subst ha
  -- The body's one outcome: one write, no sends.
  have hq := wp_sound hx (Q := fun _ _ fx => fx.writes = [("counter", .num (r + one))] ∧ fx.sends = [])
    (by
      simp only [wp, assignPre]
      intro v hv
      cases hv with
      | binop h₁ h₂ h₃ =>
        rw [EvalR.var_global (by simp [actionLocals, lookup])] at h₁
        simp [Env.global, actionEnv, updateLab, hc] at h₁
        subst h₁
        rw [EvalR.num_iff] at h₂; subst h₂
        simp only [binop, Except.ok.injEq] at h₃; subst h₃
        have hroot : isRootState updateLab "counter" = true := by decide
        exact ⟨fun _ => ⟨rfl, rfl⟩, fun h => by simp only [actionEnv, hroot] at h; cases h⟩)
  obtain ⟨hw, hs⟩ := hq
  rw [hs] at hA; cases hA
  rw [hsl]
  exact applyWrites_last (by simp [hw, lookup]) (by simp [hc])

/-! ## The counter is a positive integer

Numbers are `F64`s with proved arithmetic (`Contract.Binary64Facts`), so a
numeric invariant is proved like any other. -/

/-- The counter's values: the doubles of `1, 2, …, 2^53`. -/
def CounterOK (v : Value) : Prop := ∃ n, 1 ≤ n ∧ n ≤ 2 ^ 53 ∧ v = .num (F64.ofNat n)

/-- The literal `1` is the double of the integer 1. -/
theorem lit_one : F64.ofBits 0x3ff0000000000000 = F64.ofNat 1 := by decide +kernel

/-- Adding one to `1 … 2^53` stays there: exact below `2^53`
(`F64.add_ofNat`), and `2^53 + 1` rounds back to `2^53`
(`F64.add_one_saturates`, ties to even). -/
theorem succ_ok {n : Nat} (h1 : 1 ≤ n) (h2 : n ≤ 2 ^ 53) :
    CounterOK (.num (F64.ofNat n + F64.ofNat 1)) := by
  by_cases h : n < 2 ^ 53
  · exact ⟨n + 1, by omega, by omega, by rw [F64.add_ofNat (by omega)]⟩
  · have : n = 2 ^ 53 := by omega
    subst this
    exact ⟨2 ^ 53, by decide, Nat.le_refl _, by rw [F64.add_one_saturates]⟩

theorem counter_boot : ∀ v, SlotOrigin updateLab "counter" v → CounterOK v := by
  rintro v (⟨st, hst, hn, -, hv⟩ | m | ⟨hr, -⟩)
  · simp only [updateLab, List.mem_cons, List.mem_nil_iff, or_false] at hst
    rcases hst with rfl | rfl | rfl <;> simp at hn
    rcases hv with ⟨h, -⟩ | ⟨_, hv⟩
    · cases h
    · rw [EvalR.num_iff] at hv; subst hv
      exact ⟨1, Nat.le_refl _, by decide, by rw [lit_one]⟩
  · obtain ⟨m, hm, hmn, -⟩ := m
    simp only [updateLab, List.mem_cons, List.mem_nil_iff, or_false] at hm
    rcases hm with rfl | rfl | rfl <;> simp at hmn
  · simp [updateLab] at hr

/-- Every action keeps the counter in `1 … 2^53`: `increment` adds one,
`reset` writes `1`, the rest leave it. -/
theorem counter_keeps {c name args rows} (hc : SlotIn "counter" CounterOK c.slots) :
    BodyKeeps updateLab "counter" CounterOK c name args rows := by
  refine BodyKeeps.of_wp fun ad had _ => ?_
  simp only [updateLab, List.mem_cons, List.mem_nil_iff, or_false] at had
  rcases had with rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> simp only [wp, assignPre] <;>
    simp [Keeps, Effects.write, Effects.rowWrite, Effects.send]
  · -- increment
    intro v hv _
    cases hv with
    | binop h₁ h₂ h₃ =>
      rw [EvalR.var_global (by simp [actionLocals, lookup])] at h₁
      simp only [Env.global, actionEnv, updateLab, List.find?, beq_self_eq_true] at h₁
      cases hl : lookup "counter" c.slots with
      | none => rw [hl] at h₁; cases h₁
      | some w =>
        rw [hl] at h₁; cases h₁
        obtain ⟨n, h1, h2, rfl⟩ := hc _ hl
        rw [EvalR.num_iff] at h₂; subst h₂
        simp only [binop, Except.ok.injEq] at h₃; subst h₃
        rw [lit_one]; exact succ_ok h1 h2
  · -- reset
    have h1 : CounterOK (.num (F64.ofBits 0x3ff0000000000000)) :=
      ⟨1, Nat.le_refl _, by decide, by rw [lit_one]⟩
    exact fun _ => ⟨fun _ => h1, fun _ _ _ => h1⟩

/-- **The counter is always one of `1, 2, …, 2^53`**, exactly, after any
sequence of taps, inputs and clock moves. It never reaches a non-integer,
zero, a negative number or infinity, however many times `increment` runs:
past `2^53` adding one rounds back (ties to even). -/
theorem counter_always : ∀ c, Reachable updateLab c → SlotIn "counter" CounterOK c.slots :=
  Reachable.slotIn counter_boot (fun _ _ _ _ _ _ _ _ _ hc _ _ => counter_keeps hc)
    (fun _ _ _ hc => counter_keeps hc)

/-- **The counter is at least one**, in the IEEE order the program's `<`
and `>=` compare with. -/
theorem counter_pos {c x} (hc : Reachable updateLab c) (h : lookup "counter" c.slots = .some (.num x)) :
    F64.ofNat 1 ≤ x := by
  obtain ⟨n, h1, h2, hv⟩ := counter_always c hc _ h
  cases hv
  exact (F64.ofNat_le_ofNat h2 (by decide)).mpr h1

end UpdateLab
