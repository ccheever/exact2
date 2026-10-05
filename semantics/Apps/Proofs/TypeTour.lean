/-
Type Tour (apps/typetour/app.contract): the phone is always on one of its
five screens.

`screen` is a string, and `go(target: string)` writes whatever it is
given, so nothing about the action alone bounds it. The bound comes from
the view: every handler that names `go` passes a literal screen. A host
can only run an action through a handler the view rendered (or a task;
Type Tour has none), so `Reachable.slotIn` reduces the claim to: the
initial value is a screen, every handler's `go` names one, and each
action body writes `screen` only with a screen.
-/
import Apps.TypeTour
open Contract

namespace TypeTour

/-- The screens Type Tour has. -/
def screens : List String := ["lock", "home", "settings", "display", "messages"]

def ScreenOK (v : Value) : Prop := ∃ s ∈ screens, v = .str s

/-- A handler that names `go` passes one literal screen. -/
def goCheck (h : String × String × List Expr) : Bool :=
  h.2.1 != "go" || match h.2.2 with
    | [.str s] => screens.contains s
    | _ => false

/-- Checked over the whole view by evaluation in the kernel. -/
theorem go_handlers_checked : (Node.handlerLists typeTour.view).all goCheck = true := by decide

theorem go_handler {ev args} (h : (ev, "go", args) ∈ Node.handlerLists typeTour.view) :
    ∃ s ∈ screens, args = [.str s] := by
  have := List.all_eq_true.mp go_handlers_checked _ h
  simp only [goCheck, bne_self_eq_false, Bool.false_or] at this
  split at this
  · exact ⟨_, List.contains_iff_mem.mp this, rfl⟩
  · cases this

/-- `screen` starts as `"lock"`. -/
theorem screen_boot : ∀ v, SlotOrigin typeTour "screen" v → ScreenOK v := by
  rintro v (⟨st, hst, hn, -, hv⟩ | ⟨m, hm, -⟩ | ⟨hr, -⟩)
  · simp only [typeTour, List.mem_cons, List.mem_nil_iff, or_false] at hst
    rcases hst with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      simp at hn
    rcases hv with ⟨h, -⟩ | ⟨_, hv⟩
    · cases h
    · cases hv; exact ⟨"lock", by simp [screens], rfl⟩
  · simp [typeTour] at hm
  · simp [typeTour] at hr

/-- **The phone is always on one of its five screens**, after any sequence
of taps, inputs and clock moves, whatever its data sources answer. -/
theorem screen_always : ∀ c, Reachable typeTour c → SlotIn "screen" ScreenOK c.slots := by
  refine Reachable.slotIn (by decide) screen_boot ?_ (fun _ a ha => by simp [typeTour, clockActions, thenActions] at ha)
  intro c ev a args env ls vs payload rows _ hh hvs
  refine BodyKeeps.of_wp fun ad had hname => ?_
  subst hname
  simp only [typeTour, List.mem_cons, List.mem_nil_iff, or_false] at had
  rcases had with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
    rfl | rfl | rfl <;> simp only [wp, assignPre] <;>
    simp [Keeps, Effects.write, Effects.rowWrite, Effects.command, ScreenOK, screens]
  -- What is left is `go`: it writes its argument, which the handler gave
  -- as a literal screen.
  obtain ⟨s, hs, rfl⟩ := go_handler hh
  cases hvs with
  | cons h₁ h₂ =>
    cases h₂; rw [EvalR.str_iff] at h₁; subst h₁
    intro v hv _
    rw [EvalR.var_local (w := .str s) (by simp [actionLocals, lookup])] at hv
    subst hv
    simp only [screens, List.mem_cons, List.mem_nil_iff, or_false] at hs
    rcases hs with rfl | rfl | rfl | rfl | rfl <;> simp

end TypeTour
