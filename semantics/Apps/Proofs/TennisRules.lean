/-
Tennis's rules (game/games/tennis/rules/rules.contract; LLP 1046.009 §3.3):
the program a game world runs inside its tick. Its view is the event table,
one handler per event the world's kernels raise (`exact-game-rules` runs
only those), so what holds here holds of every sequence of events a match
can raise.
-/
import Apps.TennisRules
open Contract

namespace TennisRules

/-! ## A slot written only with literals, through calls

`Stmt.noAssigns` counts a call as writing anything. These look the callee
up instead: a check of every body is a check of every call. -/

/-- `e` is a string literal in `S`, or a choice between such. -/
def Expr.litIn (S : List String) : Expr → Bool
  | .str s => S.contains s
  | .ternary _ a b => Expr.litIn S a && Expr.litIn S b
  | _ => false

theorem Expr.litIn_value {S env inFn ls} {e : Expr} {v : Value} (h : Expr.litIn S e = true)
    (he : EvalR env inFn ls e v) : ∃ s ∈ S, v = .str s := by
  cases e <;> simp only [Expr.litIn, Bool.false_eq_true, Bool.and_eq_true] at h
  case str s =>
    rw [EvalR.str_iff] at he
    exact ⟨_, List.contains_iff_mem.mp h, he⟩
  case ternary c a b =>
    cases he with
    | ternaryTrue _ h₂ => exact Expr.litIn_value h.1 h₂
    | ternaryFalse _ h₂ => exact Expr.litIn_value h.2 h₂
termination_by sizeOf e

mutual
/-- `s` assigns `x` only literals in `S`, and never sends into it. -/
def Stmt.litOnly (x : String) (S : List String) : Stmt → Bool
  | .assign t e => t != x || Expr.litIn S e
  | .send t _ _ => t != x
  | .ifS _ a b => Stmt.litsOnly x S a && Stmt.litsOnly x S b
  | .matchS _ _ a b => Stmt.litsOnly x S a && Stmt.litsOnly x S b
  | _ => true
def Stmt.litsOnly (x : String) (S : List String) : List Stmt → Bool
  | [] => true
  | s :: ss => Stmt.litOnly x S s && Stmt.litsOnly x S ss
end

/-- A block whose statements — and the bodies of every action it may call —
write `x` only with literals in `S` adds only such writes, and no send. -/
theorem ExecR.litsOnly {x S env ls ss fx fx'} (h : ExecR env ls ss fx fx')
    (hall : ∀ a ∈ env.prog.actions, Stmt.litsOnly x S a.body = true)
    (hn : Stmt.litsOnly x S ss = true) :
    (∀ v, (x, v) ∈ fx'.writes → (x, v) ∈ fx.writes ∨ ∃ s ∈ S, v = .str s) ∧
    (∀ s ∈ fx'.sends, s.1 = x → s ∈ fx.sends) := by
  induction h with
  | nil => exact ⟨fun _ h => .inl h, fun _ h _ => h⟩
  | letS _ _ ih => exact ih (by simpa [Stmt.litsOnly, Stmt.litOnly] using hn)
  | assignRoot he _ _ ih =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true, Bool.or_eq_true, bne_iff_ne,
      ne_eq] at hn
    obtain ⟨hw, hs⟩ := ih hn.2
    refine ⟨fun v hv => ?_, fun s hs' hx => by simpa [Effects.write] using hs s hs' hx⟩
    rcases hw v hv with h | h
    · simp only [Effects.write, List.mem_append, List.mem_singleton, Prod.mk.injEq] at h
      rcases h with h | ⟨rfl, rfl⟩
      · exact .inl h
      · rcases hn.1 with h | h
        · exact absurd rfl h
        · exact .inr (Expr.litIn_value h he)
    · exact .inr h
  | assignRow _ _ _ _ _ ih =>
    simp only [Stmt.litsOnly, Bool.and_eq_true] at hn
    obtain ⟨hw, hs⟩ := ih hn.2
    exact ⟨fun v hv => by simpa [Effects.rowWrite] using hw v hv,
      fun s hs' hx => by simpa [Effects.rowWrite] using hs s hs' hx⟩
  | command _ _ ih =>
    simp only [Stmt.litsOnly, Bool.and_eq_true] at hn
    obtain ⟨hw, hs⟩ := ih hn.2
    exact ⟨fun v hv => by simpa [Effects.command] using hw v hv,
      fun s hs' hx => by simpa [Effects.command] using hs s hs' hx⟩
  | refresh _ ih =>
    simp only [Stmt.litsOnly, Bool.and_eq_true] at hn
    obtain ⟨hw, hs⟩ := ih hn.2
    exact ⟨fun v hv => by simpa [Effects.refresh] using hw v hv,
      fun s hs' hx => by simpa [Effects.refresh] using hs s hs' hx⟩
  | send _ _ ih =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true, bne_iff_ne, ne_eq] at hn
    obtain ⟨hw, hs⟩ := ih hn.2
    refine ⟨fun v hv => by simpa [Effects.send] using hw v hv, fun s hs' hx => ?_⟩
    have := hs s hs' hx
    simp only [Effects.send, List.mem_append, List.mem_singleton] at this
    rcases this with h | rfl
    · exact h
    · exact absurd hx hn.1
  | ifTrue _ _ _ ih₁ ih₂ =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true] at hn
    obtain ⟨hw₁, hs₁⟩ := ih₁ hn.1.1
    obtain ⟨hw₂, hs₂⟩ := ih₂ hn.2
    exact ⟨fun v hv => (hw₂ v hv).elim (hw₁ v) .inr, fun s h hx => hs₁ s (hs₂ s h hx) hx⟩
  | ifFalse _ _ _ ih₁ ih₂ =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true] at hn
    obtain ⟨hw₁, hs₁⟩ := ih₁ hn.1.2
    obtain ⟨hw₂, hs₂⟩ := ih₂ hn.2
    exact ⟨fun v hv => (hw₂ v hv).elim (hw₁ v) .inr, fun s h hx => hs₁ s (hs₂ s h hx) hx⟩
  | matchSome _ _ _ ih₁ ih₂ =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true] at hn
    obtain ⟨hw₁, hs₁⟩ := ih₁ hn.1.1
    obtain ⟨hw₂, hs₂⟩ := ih₂ hn.2
    exact ⟨fun v hv => (hw₂ v hv).elim (hw₁ v) .inr, fun s h hx => hs₁ s (hs₂ s h hx) hx⟩
  | matchNone _ _ _ ih₁ ih₂ =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.and_eq_true] at hn
    obtain ⟨hw₁, hs₁⟩ := ih₁ hn.1.2
    obtain ⟨hw₂, hs₂⟩ := ih₂ hn.2
    exact ⟨fun v hv => (hw₂ v hv).elim (hw₁ v) .inr, fun s h hx => hs₁ s (hs₂ s h hx) hx⟩
  | call _ hd _ _ _ ih₁ ih₂ =>
    simp only [Stmt.litsOnly, Stmt.litOnly, Bool.true_and] at hn
    obtain ⟨hw₁, hs₁⟩ := ih₁ (hall _ (List.mem_of_find?_eq_some hd))
    obtain ⟨hw₂, hs₂⟩ := ih₂ hn
    exact ⟨fun v hv => (hw₂ v hv).elim (hw₁ v) .inr, fun s h hx => hs₁ s (hs₂ s h hx) hx⟩

/-- Every body of `p` writes `x` only with literals in `S`: so does every
action a handler or the clock runs. -/
theorem BodyKeeps.of_lits {p x S c name args rows}
    (hall : ∀ a ∈ p.actions, Stmt.litsOnly x S a.body = true) :
    BodyKeeps p x (fun v => ∃ s ∈ S, v = .str s) c name args rows := fun a ha _ hx => by
  obtain ⟨hw, hs⟩ := ExecR.litsOnly hx hall (hall a (List.mem_of_find?_eq_some ha))
  exact ⟨fun v hv => (hw v hv).elim (fun h => by simp at h) id,
    fun s h hx' => by have := hs s h hx'; simp at this⟩

/-- A slot every body writes only with literals in `S`, which starts as one,
holds one in every reachable configuration. -/
theorem Reachable.lits {p x S} (hq : isQueue p x = false)
    (hall : ∀ a ∈ p.actions, Stmt.litsOnly x S a.body = true)
    (hst : (p.states.all fun st => st.name != x || (!st.late && st.owner == .none && Expr.litIn S st.init)) = true)
    (hm : ∀ m ∈ p.mutations, m.name ≠ x) (hr : p.router ≠ .some x) :
    ∀ c, Reachable p c → SlotIn x (fun v => ∃ s ∈ S, v = .str s) c.slots :=
  Reachable.slotIn hq
    (by
      rintro v (⟨st, hmem, hname, -, hv⟩ | ⟨m, hmem, hname, -⟩ | ⟨hrx, -⟩)
      · have := List.all_eq_true.mp hst st hmem
        simp only [hname, bne_self_eq_false, Bool.false_or, Bool.and_eq_true, Bool.not_eq_true',
          beq_iff_eq] at this
        rcases hv with ⟨hl, -⟩ | ⟨_, hv⟩
        · rw [this.1.1] at hl; cases hl
        · exact Expr.litIn_value this.2 hv
      · exact absurd hname (hm m hmem)
      · exact absurd hrx hr)
    (fun _ _ _ _ _ _ _ _ _ _ _ _ => BodyKeeps.of_lits hall)
    (fun _ _ _ _ => BodyKeeps.of_lits hall)

/-! ## The phase -/

/-- The point's phases. -/
def phases : List String := ["ready", "toss", "rally", "dead", "over"]

/-- **A match is always in one of its five phases**, after any sequence of
events: every action writes `phase` only with one of them, through every
call. -/
theorem phase_always : ∀ c, Reachable tennisRules c →
    SlotIn "phase" (fun v => ∃ s ∈ phases, v = .str s) c.slots :=
  Reachable.lits (by decide) (by decide) (by decide) (by simp [tennisRules]) (by simp [tennisRules])

/-- `winner` is always no one, you or Jev. -/
theorem winner_always : ∀ c, Reachable tennisRules c →
    SlotIn "winner" (fun v => ∃ s ∈ ["", "near", "far"], v = .str s) c.slots :=
  Reachable.lits (by decide) (by decide) (by decide) (by simp [tennisRules]) (by simp [tennisRules])

/-! ## Once over, always over

Every action is guarded by the phase it belongs to: its body is one `if`
on `phase == "…"` (alone, or `and` more) with no `else`, or one call of an
action whose body is. None names "over", so from a match that is over no
event writes anything. -/

/-- `p`, when `e` is `phase == "p"`. -/
def Expr.phaseIs : Expr → Option String
  | .binary .eq (.var "phase") (.str p) => .some p
  | _ => .none

theorem Expr.phaseIs_eq {e : Expr} {p : String} (h : Expr.phaseIs e = .some p) :
    e = .binary .eq (.var "phase") (.str p) := by
  unfold Expr.phaseIs at h
  split at h
  · cases h; rfl
  · cases h

/-- `a`, when `e` is `a and b`. -/
def Expr.andLeft : Expr → Option Expr
  | .binary .and a _ => .some a
  | _ => .none

theorem Expr.andLeft_eq {e a : Expr} (h : Expr.andLeft e = .some a) : ∃ b, e = .binary .and a b := by
  unfold Expr.andLeft at h
  split at h
  · cases h; exact ⟨_, rfl⟩
  · cases h

/-- `phase == "p"` with `p` not "over", alone or `and` more. -/
def Expr.phaseGuard (g : Expr) : Bool :=
  (Expr.phaseIs g).any (· != "over") ||
    (Expr.andLeft g).any fun a => (Expr.phaseIs a).any (· != "over")

theorem Expr.phaseIs_false {env ls} {g : Expr} {p : String} (hg : Expr.phaseIs g = .some p)
    (hp : p ≠ "over") (hph : ∀ v, EvalR env false ls (.var "phase") v → v = .str "over") :
    ¬ EvalR env false ls g (.bool true) := by
  rw [Expr.phaseIs_eq hg]
  intro he
  cases he with
  | binop h₁ h₂ hop =>
    rw [hph _ h₁, EvalR.str_iff.mp h₂] at hop
    simp only [binop, Value.equal, Option.elim, Function.comp, Except.ok.injEq, Value.bool.injEq,
      beq_iff_eq] at hop
    exact hp hop.symm

theorem Expr.phaseGuard_false {env ls} {g : Expr} (hg : Expr.phaseGuard g = true)
    (hph : ∀ v, EvalR env false ls (.var "phase") v → v = .str "over") :
    ¬ EvalR env false ls g (.bool true) := by
  simp only [Expr.phaseGuard, Bool.or_eq_true, Option.any_eq_true, bne_iff_ne, ne_eq] at hg
  rcases hg with ⟨p, hp, hne⟩ | ⟨a, ha, p, hp, hne⟩
  · exact Expr.phaseIs_false hp hne hph
  · obtain ⟨b, rfl⟩ := Expr.andLeft_eq ha
    intro he
    cases he with
    | andTrue h₁ _ => exact Expr.phaseIs_false hp hne hph h₁
    | binop _ _ hop => exact binop_and hop

/-- `g`, when a body is one `if g` with no `else`. -/
def Stmt.ifGuard : List Stmt → Option Expr
  | [.ifS g _ []] => .some g
  | _ => .none

theorem Stmt.ifGuard_eq {body : List Stmt} {g : Expr} (h : Stmt.ifGuard body = .some g) :
    ∃ thn, body = [.ifS g thn []] := by
  unfold Stmt.ifGuard at h
  split at h
  · cases h; exact ⟨_, rfl⟩
  · cases h

/-- `a`, when a body is one call of `a`. -/
def Stmt.callOf : List Stmt → Option String
  | [.call a _] => .some a
  | _ => .none

theorem Stmt.callOf_eq {body : List Stmt} {a : String} (h : Stmt.callOf body = .some a) :
    ∃ args, body = [.call a args] := by
  unfold Stmt.callOf at h
  split at h
  · cases h; exact ⟨_, rfl⟩
  · cases h

/-- A phase-guarded body, or one call of an action whose body is. -/
def Stmt.guarded (p : Program) (body : List Stmt) : Bool :=
  (Stmt.ifGuard body).any Expr.phaseGuard ||
    (Stmt.callOf body).any fun a =>
      (p.actions.find? (·.name == a)).any fun ad => (Stmt.ifGuard ad.body).any Expr.phaseGuard

theorem ExecR.guardedIf {env ls body g fx fx'} (hb : Stmt.ifGuard body = .some g)
    (hg : Expr.phaseGuard g = true)
    (hph : ∀ v, EvalR env false ls (.var "phase") v → v = .str "over")
    (h : ExecR env ls body fx fx') : fx' = fx := by
  obtain ⟨thn, rfl⟩ := Stmt.ifGuard_eq hb
  cases h with
  | ifTrue hc _ _ => exact absurd hc (Expr.phaseGuard_false hg hph)
  | ifFalse _ he hr => cases he; cases hr; rfl

/-- No local is named `phase`, so it reads the slot. -/
theorem phase_global {env : Env} {ls : Locals} (hl : lookup "phase" ls = .none)
    (hg : ∀ v, env.global "phase" = .ok v → v = .str "over") :
    ∀ v, EvalR env false ls (.var "phase") v → v = .str "over" :=
  fun v h => hg v ((EvalR.var_global hl).mp h)

theorem lookup_zip_none {x : String} {names : List String} {vs : List Value} (h : x ∉ names) :
    lookup x ((names.zip vs).reverse) = .none :=
  lookup_none fun _ hv => h (List.of_mem_zip (List.mem_reverse.mp hv)).1

/-- A guarded body, run while the phase is over, does nothing. -/
theorem ExecR.guarded {env : Env} {ls body fx fx'} (hb : Stmt.guarded env.prog body = true)
    (hparams : ∀ a ∈ env.prog.actions, "phase" ∉ a.params.map (·.1)) (hl : lookup "phase" ls = .none)
    (hg : ∀ v, env.global "phase" = .ok v → v = .str "over")
    (h : ExecR env ls body fx fx') : fx' = fx := by
  simp only [Stmt.guarded, Bool.or_eq_true, Option.any_eq_true] at hb
  rcases hb with ⟨g, hig, hpg⟩ | ⟨a, hca, ad, hd, g, hig, hpg⟩
  · exact ExecR.guardedIf hig hpg (phase_global hl hg) h
  · obtain ⟨args, rfl⟩ := Stmt.callOf_eq hca
    cases h with
    | call _ hd' _ hbody hr =>
      cases hr
      rw [hd] at hd'
      cases hd'
      exact ExecR.guardedIf hig hpg
        (phase_global (lookup_zip_none (hparams _ (List.mem_of_find?_eq_some hd))) hg) hbody

/-- From a configuration whose phase is over, every action keeps it over:
it writes nothing at all. -/
theorem keeps_over {c name args rows}
    (hs : SlotIn "phase" (· = .str "over") c.slots)
    (hb : ∀ a ∈ tennisRules.actions, Stmt.guarded tennisRules a.body = true)
    (hparams : ∀ a ∈ tennisRules.actions, "phase" ∉ a.params.map (·.1)) :
    BodyKeeps tennisRules "phase" (· = .str "over") c name args rows := by
  intro a ha fx hx
  have hmem := List.mem_of_find?_eq_some ha
  have hg : ∀ v, (actionEnv tennisRules c rows).global "phase" = .ok v → v = .str "over" := by
    intro v hv
    rw [actionEnv_global (st := ⟨"phase", .string, .str "ready", .none, false⟩) rfl rfl] at hv
    cases hl : lookup "phase" c.slots with
    | none => rw [hl] at hv; cases hv
    | some w => rw [hl] at hv; cases hv; exact hs _ hl
  have := ExecR.guarded (env := actionEnv tennisRules c rows) (hb a hmem) hparams
    (lookup_zip_none (hparams a hmem)) hg hx
  subst this
  exact ⟨fun v hv => by simp at hv, fun s hs' => by simp at hs'⟩

/-- **Once the match is over, it stays over**, whatever event comes next —
from any configuration, reachable or not. -/
theorem over_stays_over {o c} (ev : Event)
    (h : SlotIn "phase" (· = .str "over") c.slots) :
    SlotIn "phase" (· = .str "over") (ev.step tennisRules o c).1.slots :=
  Event.step_slotIn ev (by decide) (fun _ _ _ _ hs => keeps_over hs (by decide) (by decide)) h

end TennisRules
