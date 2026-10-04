/-
Video Player (apps/video-player/app.contract): a video leaf with a
play/pause button, a note field, and an error line.
-/
import Apps.VideoPlayer
open Contract

namespace VideoPlayer

def IsBool (v : Value) : Prop := ∃ b, v = .bool b

/-- **`paused` is always a bool**, in every reachable configuration —
whatever the video element reports (`play`, `pause`, `error`) and
whatever payloads come with it. -/
theorem paused_bool : ∀ c, Reachable videoPlayer c → SlotIn "paused" IsBool c.slots := by
  have keeps : ∀ c name args rows, BodyKeeps videoPlayer "paused" IsBool c name args rows :=
    fun _ _ _ _ => BodyKeeps.of_wp fun a ha _ => by
      simp only [videoPlayer, List.mem_cons, List.mem_nil_iff, or_false] at ha
      rcases ha with rfl | rfl | rfl | rfl | rfl | rfl <;>
        simp [wp, assignPre, Keeps, Effects.write, Effects.rowWrite, Effects.command, IsBool]
      -- `toggle` writes `not paused`, a bool whatever it read.
      intro v hv _
      rcases hv with ⟨-, rfl⟩ | ⟨-, rfl⟩ <;> simp
  refine Reachable.slotIn ?_ (fun c _ a _ _ _ _ _ _ _ _ _ => keeps c a _ _) (fun c a _ _ => keeps c a _ _)
  rintro v (⟨st, hst, hn, -, hv⟩ | ⟨m, hm, -⟩ | ⟨hr, -⟩)
  · simp only [videoPlayer, List.mem_cons, List.mem_nil_iff, or_false] at hst
    rcases hst with rfl | rfl | rfl <;> simp at hn
    rcases hv with ⟨h, -⟩ | ⟨_, hv⟩
    · cases h
    · rw [EvalR.bool_iff] at hv; exact ⟨_, hv⟩
  · simp [videoPlayer] at hm
  · simp [videoPlayer] at hr

/-- **The button flips the state.** A committed `toggle` from `paused = b`
leaves `paused = !b`. -/
theorem toggle_flips {o c rows c' out b} (hp : lookup "paused" c.slots = .some (.bool b))
    (h : runAction videoPlayer o c "toggle" [] rows = (c', out)) (hout : ∀ e, out ≠ .refused e) :
    lookup "paused" c'.slots = .some (.bool !b) := by
  obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h hout
  simp [videoPlayer] at ha
  subst ha
  have hroot : isRootState videoPlayer "paused" = true := by decide
  have hq := wp_sound hx (Q := fun _ _ fx => fx.writes = [("paused", .bool !b)] ∧ fx.sends = [])
    (by
      simp only [wp, assignPre, actionEnv, hroot]
      intro v hv
      rw [EvalR.not_iff] at hv
      obtain ⟨b', h₁, rfl⟩ := hv
      rw [EvalR.var_global (by simp [actionLocals, lookup])] at h₁
      simp [Env.global, videoPlayer, hp] at h₁
      subst h₁
      exact ⟨fun _ => ⟨rfl, rfl⟩, fun h => nomatch h⟩)
  obtain ⟨hw, hs⟩ := hq
  rw [hs] at hA; cases hA
  rw [hsl]
  exact applyWrites_last (by simp [hw, lookup]) (by simp [hp])

/-- **Done only moves focus.** A committed `done` changes no slot and
issues exactly one command, `focus("done")`. -/
theorem done_focuses {o c args rows c' out}
    (h : runAction videoPlayer o c "done" args rows = (c', out)) (hout : ∀ e, out ≠ .refused e) :
    c'.slots = c.slots ∧ c'.commands = c.commands ++ [("focus", [.str "done"])] := by
  obtain ⟨a, fx, ans, ha, hx, hA, hsl, hcmd⟩ := runAction_commit h hout
  simp [videoPlayer] at ha
  subst ha
  have hq := wp_sound hx
    (Q := fun _ _ fx => fx.writes = [] ∧ fx.sends = [] ∧ fx.commands = [("focus", [.str "done"])])
    (by
      simp only [wp]
      intro vs hvs
      cases hvs with
      | cons h₁ h₂ =>
        cases h₂; rw [EvalR.str_iff] at h₁; subst h₁
        exact ⟨rfl, rfl, rfl⟩)
  obtain ⟨hw, hs, hc⟩ := hq
  rw [hs] at hA; cases hA
  exact ⟨by rw [hsl, hw]; rfl, by rw [hcmd, hc]⟩

end VideoPlayer
