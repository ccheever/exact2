/-
Photo Editor (apps/photo-editor/app.contract): a native editor module
reports gestures; two buttons rotate and reset; `ready` records that the
photo loaded.
-/
import Apps.PhotoEditor
open Contract

namespace PhotoEditor

/-- IEEE-754 binary64 zero and one, as the embedding writes `0` and `1`. -/
def zero : F64 := F64.ofBits 0
def one : F64 := F64.ofBits 0x3ff0000000000000

/-- Every action keeps `ready = true`: only `loaded` writes it, with `true`. -/
theorem keeps_ready {c name args rows} :
    BodyKeeps photoEditor "ready" (· = .bool true) c name args rows := by
  refine BodyKeeps.of_wp fun a ha _ => ?_
  simp only [photoEditor, List.mem_cons, List.mem_nil_iff, or_false] at ha
  rcases ha with rfl | rfl | rfl | rfl | rfl <;> simp [wp, assignPre, Keeps, Effects.write, Effects.rowWrite]

/-- **Once ready, always ready.** From any configuration where the photo
has loaded, no event — a gesture, a tap, a payload of any shape, a clock
move — makes `ready` anything but `true`. -/
theorem ready_stays {o c} (ev : Event) (h : lookup "ready" c.slots = .some (.bool true)) :
    SlotIn "ready" (· = .bool true) (ev.step photoEditor o c).1.slots :=
  ev.step_slotIn (by decide) (fun _ _ _ _ _ => keeps_ready) fun v hv => by rw [h] at hv; cases hv; rfl

/-- **Rotating touches nothing but the turns.** -/
theorem rotate_only_turns {o c args rows c' out x} (hx : x ≠ "turns")
    (h : runAction photoEditor o c "rotate" args rows = (c', out)) :
    lookup x c'.slots = lookup x c.slots :=
  runAction_untouched (fun a ha => by
    simp [photoEditor] at ha; subst ha
    simp [Stmt.noAssigns, Stmt.noAssign, Stmt.noSends, Stmt.noSend, Ne.symm hx]) h

/-- **Reset puts the turns back to zero and counts itself.** A committed
`reset` leaves `turns` at zero and `resets` at its old value plus one. -/
theorem reset_spec {o c rows c' out r} (hr : lookup "resets" c.slots = .some (.num r))
    (ht : (lookup "turns" c.slots).isSome)
    (h : runAction photoEditor o c "reset" [] rows = (c', out)) (hout : ∀ e, out ≠ .refused e) :
    lookup "turns" c'.slots = .some (.num zero) ∧ lookup "resets" c'.slots = .some (.num (r + one)) := by
  obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h hout
  simp [photoEditor] at ha
  subst ha
  have hroot : ∀ x ∈ ["turns", "resets"], isRootState photoEditor x = true := by decide
  have hq := wp_sound hx
    (Q := fun _ _ fx => fx.writes = [("turns", .num zero), ("resets", .num (r + one))] ∧ fx.sends = [])
    (by
      simp only [wp, assignPre, actionEnv, hroot "turns" (by simp), hroot "resets" (by simp)]
      intro v hv
      rw [EvalR.num_iff] at hv; subst hv
      refine ⟨fun _ w hw => ?_, fun h => nomatch h⟩
      cases hw with
      | binop h₁ h₂ h₃ =>
        rw [EvalR.var_global (by simp [actionLocals, lookup])] at h₁
        simp [Env.global, photoEditor, hr] at h₁
        subst h₁
        rw [EvalR.num_iff] at h₂; subst h₂
        simp only [binop, Except.ok.injEq] at h₃; subst h₃
        exact ⟨fun _ => ⟨rfl, rfl⟩, fun h => nomatch h⟩)
  obtain ⟨hw, hs⟩ := hq
  rw [hs] at hA; cases hA
  rw [hsl]
  exact ⟨applyWrites_last (by simp [hw, lookup]) ht, applyWrites_last (by simp [hw, lookup]) (by simp [hr])⟩

end PhotoEditor
