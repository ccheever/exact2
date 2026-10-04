/-
The compiler of `Contract.Lower` is correct for expressions: every piece
meets its specification at every fuel, and so a compiled body, run on the
VM in a corresponding state, returns exactly `eval`'s value, and returns at
all exactly when `eval` has one.
-/
import Contract.LowerCalls

namespace Contract.Lower

open Vm

variable {fuel : Nat} {p : Program} {depth : Nat} {sc : Scope} {n : Nat} {c : Code} {t : STy}
  {env : Contract.Env} {inFn : Bool} {ls : Locals} {venv : Vm.Env} {L : List Value} {P : Code}

theorem each_sem {fl : Bool} {name l ps body} (hname : name = if fl then "filter" else "map")
    (hfd : env.prog.fns.find? (·.name == name) = none) :
    ∀ v, EvalR env inFn ls (.call (if fl then "filter" else "map") [l, .arrow ps body]) v ↔
      ∃ xs ys, EvalR env inFn ls l (.list xs) ∧ v = .list ys ∧
        if fl then FilterR env inFn ls ps body xs 0 ys else MapR env inFn ls ps body xs 0 ys := by
  subst hname
  intro v
  constructor
  · intro h
    rcases evalR_call_inv h with ⟨_, _, h1, _⟩ | ⟨_, ⟨_, _, _, _, _, h1, h2, h3, h4, h5⟩ |
      ⟨_, _, _, _, _, h1, h2, h3, h4, h5⟩ | ⟨y, h2, _⟩ | ⟨_, _, h1⟩⟩
    · rw [hfd] at h1; cases h1
    · cases fl <;> simp at h1 h2
      obtain ⟨rfl, rfl, rfl⟩ := h2
      exact ⟨_, _, h3, h5, h4⟩
    · cases fl <;> simp at h1 h2
      obtain ⟨rfl, rfl, rfl⟩ := h2
      exact ⟨_, _, h3, h5, h4⟩
    · cases fl <;> simp at h2
    · cases fl
      · exact absurd h1 stdlib_map
      · exact absurd h1 stdlib_filter
  · rintro ⟨xs, ys, h1, rfl, h2⟩
    cases fl
    · exact .map hfd h1 h2
    · exact .filter hfd h1 h2

theorem case_call (ih : AllOk fuel) (hc : compile (fuel + 1) p depth sc n (.call name args) = .ok (c, t))
    (hx : Ctx env inFn ls venv L p sc n) : ExprSpec env inFn ls venv P L (.call name args) c t := by
  rw [compile.eq_def] at hc
  simp only at hc
  split at hc
  · rename_i fd hfd
    split at hc
    · cases hc
    split at hc
    · cases hc
    rename_i hlen
    exact case_fn ih hfd hc (by simpa using hlen) hx
  · rename_i hfd
    have hfd' : env.prog.fns.find? (·.name == name) = none := by rw [hx.prog]; exact hfd
    split at hc
    · split at hc
      · cases hc
      simp only [Except.bind_ok_iff] at hc
      obtain ⟨⟨cl, tl⟩, hl0, ⟨cb, tb⟩, hb0, h1⟩ := hc
      simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
      exact case_each (fl := false) ih hl0 hb0 hx (each_sem (fl := false) rfl hfd')
    · split at hc
      · cases hc
      simp only [Except.bind_ok_iff] at hc
      obtain ⟨⟨cl, tl⟩, hl0, ⟨cb, tb⟩, hb0, h1⟩ := hc
      simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
      exact case_each (fl := true) ih hl0 hb0 hx (each_sem (fl := true) rfl hfd')
    · exact case_pending hfd hc hx
    · exact case_failed hfd hc hx
    · split at hc
      · cases hc
      rename_i hname
      split at hc
      · cases hc
      simp only [Except.bind_ok_iff] at hc
      obtain ⟨⟨ca, ts⟩, ha, h1⟩ := hc
      simp only [Except.ok.injEq, Prod.mk.injEq] at h1; obtain ⟨rfl, rfl⟩ := h1
      exact case_stdlib ih hfd (fun h => hname (by rcases h with h | h | h | h <;> simp [h])) ha hx

theorem compile_succ (ih : AllOk fuel) : CompileOk (fuel + 1) := by
  intro p depth sc n e c t hc env inFn ls venv L P hx
  cases e with
  | num b => exact case_lit (s := "") (bb := false) hc hx (.inl rfl)
  | str s => exact case_lit (b := 0) (bb := false) hc hx (.inr (.inl rfl))
  | bool b => exact case_lit (b := 0) (s := "") (bb := b) hc hx (.inr (.inr (.inl rfl)))
  | none => exact case_lit (b := 0) (s := "") (bb := false) hc hx (.inr (.inr (.inr (.inl rfl))))
  | emptyList => exact case_lit (b := 0) (s := "") (bb := false) hc hx (.inr (.inr (.inr (.inr rfl))))
  | some e => exact case_some ih hc hx
  | template parts => exact case_template ih ih.2.2.2.1 hc hx
  | var x => exact case_var hc hx
  | member e f => exact case_member ih hc hx
  | call name args => exact case_call ih hc hx
  | record shape base fields => exact case_record ih hc hx
  | unary op e =>
    cases op
    · exact case_neg ih hc hx
    · exact case_not ih hc hx
  | binary op a b =>
    cases op
    case and => exact case_and ih hc hx
    case or => exact case_or ih hc hx
    all_goals exact case_binary ih ⟨by simp, by simp⟩ hc hx
  | ternary e a b => exact case_ternary ih hc hx
  | matchOpt s x a b => exact case_match ih hc hx
  | arrow ps body => simp [compile] at hc
  | letE x v body => exact case_let ih hc hx
  | named x e => simp [compile] at hc
  | typed e ty => exact case_typed ih hc hx

/-- Every piece meets its specification, at every fuel. -/
theorem allOk : ∀ fuel, AllOk fuel
  | 0 => allOk_zero
  | fuel + 1 =>
    have ih := allOk fuel
    ⟨compile_succ ih, args_succ ih, bind_succ ih, parts_succ ih, fields_succ ih⟩

end Contract.Lower
