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
def one : Float := Float.ofBits 0x3ff0000000000000

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

end UpdateLab
