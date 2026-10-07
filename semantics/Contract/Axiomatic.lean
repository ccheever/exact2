/-
An axiomatic semantics for action bodies: a Hoare logic over `ExecR`, its
weakest preconditions, and the transaction laws of `runAction`.

An assertion is about the state the action reads (`env`, which no statement
changes), the locals in scope, and the effects asked for so far. That is
the whole of Contract's discipline in one sentence: a statement adds to the
effects and never to what a later statement reads. So the assignment rule
substitutes into the effects, not into the state, and the expressions after
it are read against the same `env` as the ones before.

A block's triple relates the locals at its start to those at its end, which
are the same: a `let` scopes over the rest of its block and ends with it.
The `let` rule says so by reading the postcondition one local further out
(`Q` at `ls.tail`).
-/
import Contract.Big

namespace Contract

/-- Pre-state, locals in scope, effects so far. -/
abbrev Assn := Env → Locals → Effects → Prop

/-- `Q` holds of every outcome of `ss` run where `P` holds. Partial
correctness: a block with no outcome satisfies every triple. -/
def Triple (P : Assn) (ss : List Stmt) (Q : Assn) : Prop :=
  ∀ env ls fx fx', P env ls fx → ExecR env ls ss fx fx' → Q env ls fx'

/-- No `let` at the top of a block: what follows it sees the same locals. -/
def NoLet : List Stmt → Prop
  | [] => True
  | .letS _ _ :: _ => False
  | _ :: rest => NoLet rest

/-- The assertion one local further out. -/
def Assn.outer (Q : Assn) : Assn := fun env ls fx => Q env ls.tail fx

/-- What an assignment needs of its continuation: for the value the
right-hand side has in the pre-state, the write is appended to the
effects of its kind. Nothing about `env` changes. -/
def assignPre (t : String) (e : Expr) (Q : Assn) : Assn := fun env ls fx =>
  ∀ v, EvalR env false ls e v →
    (isRootState env.prog t = true → Q env ls (fx.write t v)) ∧
    (isRootState env.prog t = false → isRowState env.prog t = true → (lookup t env.rows).isSome →
      Q env ls (fx.rowWrite t v))

/-- What a call needs of its continuation (LLP 1089 D9): for the
arguments' values in the pre-state and every outcome of the callee's body
run from the effects so far, in a scope of its parameters alone. The
callee reads the same `env`: a call sees the state the action started
with, never the caller's pending writes. -/
def callPre (a : String) (args : List Expr) (Q : Assn) : Assn := fun env ls fx =>
  ∀ vs ad fx₁, ListR env false ls args vs → env.prog.actions.find? (·.name == a) = .some ad →
    ad.params.length = vs.length →
    ExecR env ((ad.params.map (·.1)).zip vs).reverse ad.body fx fx₁ → Q env ls fx₁

/-- The proof system. -/
inductive Derives : Assn → List Stmt → Assn → Prop
  | nil : Derives P [] P
  /-- `let x = e; rest`: `rest` runs with `x` bound to `e`'s value, and its
  postcondition is read with `x` dropped. -/
  | letS : (∀ env ls fx v, P env ls fx → EvalR env false ls e v → P' env ((x, v) :: ls) fx) →
      Derives P' rest Q.outer → Derives P (.letS x e :: rest) Q
  /-- `t = e`: the write is recorded, the state is not changed. -/
  | assign : Derives (assignPre t e Q) [.assign t e] Q
  | command : Derives (fun env ls fx => ∀ vs, ListR env false ls args vs → Q env ls (fx.command n vs))
      [.command n args] Q
  | send : Derives (fun env ls fx => ∀ vs, ListR env false ls args vs → Q env ls (fx.send t src vs))
      [.send t src args] Q
  | refresh : Derives (fun env ls fx => Q env ls (fx.refresh t)) [.refresh t] Q
  /-- A call: its continuation holds after every outcome of the callee. -/
  | call : Derives (callPre a args Q) [.call a args] Q
  | ifS : Derives (fun env ls fx => P env ls fx ∧ EvalR env false ls c (.bool true)) thn Q →
      Derives (fun env ls fx => P env ls fx ∧ EvalR env false ls c (.bool false)) els Q →
      Derives P [.ifS c thn els] Q
  /-- The `some` arm binds `x` for its block alone. -/
  | matchS : (∀ env ls fx v, P env ls fx → EvalR env false ls s (.some v) → P' env ((x, v) :: ls) fx) →
      Derives P' sm Q.outer →
      Derives (fun env ls fx => P env ls fx ∧ EvalR env false ls s .none) nn Q →
      Derives P [.matchS s x sm nn] Q
  /-- Sequencing, for a first block with no `let` of its own (a `let`
  would scope over the second block; the `let` rule covers that). -/
  | seq : NoLet ss₁ → Derives P ss₁ R → Derives R ss₂ Q → Derives P (ss₁ ++ ss₂) Q
  | conseq : (∀ env ls fx, P' env ls fx → P env ls fx) → Derives P ss Q →
      (∀ env ls fx, Q env ls fx → Q' env ls fx) → Derives P' ss Q'

/-! ## Soundness -/

theorem NoLet.tail {s : Stmt} {ss} (h : NoLet (s :: ss)) : NoLet ss := by
  cases s <;> simp_all [NoLet]

/-- A block with no `let` of its own splits at any point. -/
theorem ExecR.append {env ls} :
    ∀ {ss₁ ss₂ fx fx'}, NoLet ss₁ → ExecR env ls (ss₁ ++ ss₂) fx fx' →
      ∃ fx₁, ExecR env ls ss₁ fx fx₁ ∧ ExecR env ls ss₂ fx₁ fx'
  | [], _, fx, _, _, h => ⟨fx, .nil, h⟩
  | _ :: ss₁, ss₂, _, _, hn, h => by
    have ih := fun {fx fx'} => @ExecR.append env ls ss₁ ss₂ fx fx' hn.tail
    cases h with
    | letS => simp [NoLet] at hn
    | assignRoot he hr hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .assignRoot he hr h₁, h₂⟩
    | assignRow he hr hw hs hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .assignRow he hr hw hs h₁, h₂⟩
    | command he hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .command he h₁, h₂⟩
    | send he hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .send he h₁, h₂⟩
    | refresh hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .refresh h₁, h₂⟩
    | ifTrue hc hb hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .ifTrue hc hb h₁, h₂⟩
    | ifFalse hc hb hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .ifFalse hc hb h₁, h₂⟩
    | matchSome hc hb hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .matchSome hc hb h₁, h₂⟩
    | matchNone hc hb hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .matchNone hc hb h₁, h₂⟩
    | call ha hd hl hb hx => obtain ⟨f, h₁, h₂⟩ := ih hx; exact ⟨f, .call ha hd hl hb h₁, h₂⟩

theorem ExecR.append_intro {env ls} :
    ∀ {ss₁ ss₂ fx fx₁ fx'}, NoLet ss₁ → ExecR env ls ss₁ fx fx₁ → ExecR env ls ss₂ fx₁ fx' →
      ExecR env ls (ss₁ ++ ss₂) fx fx'
  | [], _, _, _, _, _, h₁, h₂ => by cases h₁; exact h₂
  | _ :: ss₁, ss₂, _, fx₁, fx', hn, h₁, h₂ => by
    have ih := fun {fx} (h : ExecR env ls ss₁ fx fx₁) => ExecR.append_intro (ss₂ := ss₂) (fx' := fx') hn.tail h h₂
    cases h₁ with
    | letS => simp [NoLet] at hn
    | assignRoot he hr hx => exact .assignRoot he hr (ih hx)
    | assignRow he hr hw hs hx => exact .assignRow he hr hw hs (ih hx)
    | command he hx => exact .command he (ih hx)
    | send he hx => exact .send he (ih hx)
    | refresh hx => exact .refresh (ih hx)
    | ifTrue hc hb hx => exact .ifTrue hc hb (ih hx)
    | ifFalse hc hb hx => exact .ifFalse hc hb (ih hx)
    | matchSome hc hb hx => exact .matchSome hc hb (ih hx)
    | matchNone hc hb hx => exact .matchNone hc hb (ih hx)
    | call ha hd hl hb hx => exact .call ha hd hl hb (ih hx)

theorem Derives.sound {P ss Q} (h : Derives P ss Q) : Triple P ss Q := by
  induction h with
  | nil => intro env ls fx fx' hp hx; cases hx; exact hp
  | letS hside _ ih =>
    intro env ls fx fx' hp hx
    cases hx with
    | letS he hr => exact ih _ _ _ _ (hside _ _ _ _ hp he) hr
  | assign =>
    intro env ls fx fx' hp hx
    cases hx with
    | assignRoot he hr hn => cases hn; exact ((hp _ he).1 hr)
    | assignRow he hr hw hs hn => cases hn; exact ((hp _ he).2 hr hw hs)
  | command =>
    intro env ls fx fx' hp hx
    cases hx with
    | command he hn => cases hn; exact hp _ he
  | send =>
    intro env ls fx fx' hp hx
    cases hx with
    | send he hn => cases hn; exact hp _ he
  | refresh =>
    intro env ls fx fx' hp hx
    cases hx with
    | refresh hn => cases hn; exact hp
  | call =>
    intro env ls fx fx' hp hx
    cases hx with
    | call ha hd hl hb hn => cases hn; exact hp _ _ _ ha hd hl hb
  | ifS _ _ iht ihe =>
    intro env ls fx fx' hp hx
    cases hx with
    | ifTrue hc hb hn => cases hn; exact iht _ _ _ _ ⟨hp, hc⟩ hb
    | ifFalse hc hb hn => cases hn; exact ihe _ _ _ _ ⟨hp, hc⟩ hb
  | matchS hside _ _ ihs ihn =>
    intro env ls fx fx' hp hx
    cases hx with
    | matchSome hc hb hn => cases hn; exact ihs _ _ _ _ (hside _ _ _ _ hp hc) hb
    | matchNone hc hb hn => cases hn; exact ihn _ _ _ _ ⟨hp, hc⟩ hb
  | seq hn _ _ ih₁ ih₂ =>
    intro env ls fx fx' hp hx
    obtain ⟨fx₁, h₁, h₂⟩ := ExecR.append hn hx
    exact ih₂ _ _ _ _ (ih₁ _ _ _ _ hp h₁) h₂
  | conseq hpre _ hpost ih =>
    intro env ls fx fx' hp hx
    exact hpost _ _ _ (ih _ _ _ _ (hpre _ _ _ hp) hx)

/-- And so for the interpreter, at any fuel. -/
theorem Derives.sound_exec {P ss Q} (h : Derives P ss Q) {n env ls fx fx'}
    (hp : P env ls fx) (hx : exec n env ls ss fx = .ok fx') : Q env ls fx' :=
  h.sound env ls fx fx' hp (exec_sound hx)

/-! ## Weakest preconditions -/

/-- The weakest (liberal) precondition of a block, by its syntax. Every
expression is read in the pre-state; a branch's postcondition is the rest
of the block's precondition, at the same locals. -/
def wp : List Stmt → Assn → Assn
  | [], Q => Q
  | .letS x e :: rest, Q => fun env ls fx =>
    ∀ v, EvalR env false ls e v → wp rest Q.outer env ((x, v) :: ls) fx
  | .assign t e :: rest, Q => assignPre t e (wp rest Q)
  | .command n args :: rest, Q => fun env ls fx =>
    ∀ vs, ListR env false ls args vs → wp rest Q env ls (fx.command n vs)
  | .send t src args :: rest, Q => fun env ls fx =>
    ∀ vs, ListR env false ls args vs → wp rest Q env ls (fx.send t src vs)
  | .refresh t :: rest, Q => fun env ls fx => wp rest Q env ls (fx.refresh t)
  | .ifS c thn els :: rest, Q => fun env ls fx =>
    (EvalR env false ls c (.bool true) → wp thn (wp rest Q) env ls fx) ∧
    (EvalR env false ls c (.bool false) → wp els (wp rest Q) env ls fx)
  | .matchS s x sm nn :: rest, Q => fun env ls fx =>
    (∀ v, EvalR env false ls s (.some v) → wp sm (wp rest Q).outer env ((x, v) :: ls) fx) ∧
    (EvalR env false ls s .none → wp nn (wp rest Q) env ls fx)
  | .call a args :: rest, Q => callPre a args (wp rest Q)

theorem wp_nil {Q} : wp [] Q = Q := by rw [wp]
theorem wp_letS {x e rest Q} : wp (.letS x e :: rest) Q = fun env ls fx =>
    ∀ v, EvalR env false ls e v → wp rest Q.outer env ((x, v) :: ls) fx := by rw [wp]
theorem wp_assign {t e rest Q} : wp (.assign t e :: rest) Q = assignPre t e (wp rest Q) := by rw [wp]
theorem wp_command {n args rest Q} : wp (.command n args :: rest) Q = fun env ls fx =>
    ∀ vs, ListR env false ls args vs → wp rest Q env ls (fx.command n vs) := by rw [wp]
theorem wp_send {t src args rest Q} : wp (.send t src args :: rest) Q = fun env ls fx =>
    ∀ vs, ListR env false ls args vs → wp rest Q env ls (fx.send t src vs) := by rw [wp]
theorem wp_refresh {t rest Q} : wp (.refresh t :: rest) Q = fun env ls fx =>
    wp rest Q env ls (fx.refresh t) := by rw [wp]
theorem wp_ifS {c thn els rest Q} : wp (.ifS c thn els :: rest) Q = fun env ls fx =>
    (EvalR env false ls c (.bool true) → wp thn (wp rest Q) env ls fx) ∧
    (EvalR env false ls c (.bool false) → wp els (wp rest Q) env ls fx) := by rw [wp]
theorem wp_matchS {s x sm nn rest Q} : wp (.matchS s x sm nn :: rest) Q = fun env ls fx =>
    (∀ v, EvalR env false ls s (.some v) → wp sm (wp rest Q).outer env ((x, v) :: ls) fx) ∧
    (EvalR env false ls s .none → wp nn (wp rest Q) env ls fx) := by rw [wp]
theorem wp_call {a args rest Q} : wp (.call a args :: rest) Q = callPre a args (wp rest Q) := by rw [wp]

/-- `wp` is a precondition. -/
theorem wp_sound {env ls ss fx fx'} (h : ExecR env ls ss fx fx') :
    ∀ {Q}, wp ss Q env ls fx → Q env ls fx' := by
  induction h with
  | nil => intro Q hw; rwa [wp_nil] at hw
  | letS he _ ih => intro Q hw; rw [wp_letS] at hw; exact ih (hw _ he)
  | assignRoot he hr _ ih => intro Q hw; rw [wp_assign] at hw; exact ih ((hw _ he).1 hr)
  | assignRow he hr hrow hs _ ih => intro Q hw; rw [wp_assign] at hw; exact ih ((hw _ he).2 hr hrow hs)
  | command he _ ih => intro Q hw; rw [wp_command] at hw; exact ih (hw _ he)
  | send he _ ih => intro Q hw; rw [wp_send] at hw; exact ih (hw _ he)
  | refresh _ ih => intro Q hw; rw [wp_refresh] at hw; exact ih hw
  | ifTrue hc _ _ ih₁ ih₂ => intro Q hw; rw [wp_ifS] at hw; exact ih₂ (ih₁ (hw.1 hc))
  | ifFalse hc _ _ ih₁ ih₂ => intro Q hw; rw [wp_ifS] at hw; exact ih₂ (ih₁ (hw.2 hc))
  | matchSome hc _ _ ih₁ ih₂ => intro Q hw; rw [wp_matchS] at hw; exact ih₂ (ih₁ (hw.1 _ hc))
  | matchNone hc _ _ ih₁ ih₂ => intro Q hw; rw [wp_matchS] at hw; exact ih₂ (ih₁ (hw.2 hc))
  | call ha hd hl hb _ _ ih₂ => intro Q hw; rw [wp_call] at hw; exact ih₂ (hw _ _ _ ha hd hl hb)

/-- A call's precondition from the callee's own `wp`: what every callee
body guarantees, from the caller's arguments, of the rest of the caller's
block. -/
theorem callPre_of_wp {a args Q env ls fx}
    (h : ∀ vs ad, ListR env false ls args vs → env.prog.actions.find? (·.name == a) = .some ad →
      ad.params.length = vs.length →
      wp ad.body (fun _ _ fx₁ => Q env ls fx₁) env ((ad.params.map (·.1)).zip vs).reverse fx) :
    callPre a args Q env ls fx :=
  fun _ _ _ ha hd hl hb => wp_sound hb (h _ _ ha hd hl)

theorem wp_triple (ss : List Stmt) (Q : Assn) : Triple (wp ss Q) ss Q :=
  fun _ _ _ _ hw hx => wp_sound hx hw

/-- `wp` is the weakest: whatever guarantees `Q` of every outcome implies it. -/
theorem wp_weakest : ∀ (ss : List Stmt) {Q : Assn} {env ls fx},
    (∀ fx', ExecR env ls ss fx fx' → Q env ls fx') → wp ss Q env ls fx
  | [], _, _, _, _, h => by rw [wp_nil]; exact h _ .nil
  | .letS x e :: rest, _, _, _, _, h => by
    rw [wp_letS]; exact fun _ he => wp_weakest rest fun _ hx => h _ (.letS he hx)
  | .assign t e :: rest, _, _, _, _, h => by
    rw [wp_assign]
    exact fun _ he =>
      ⟨fun hr => wp_weakest rest fun _ hx => h _ (.assignRoot he hr hx),
       fun hr hw hs => wp_weakest rest fun _ hx => h _ (.assignRow he hr hw hs hx)⟩
  | .command n args :: rest, _, _, _, _, h => by
    rw [wp_command]; exact fun _ he => wp_weakest rest fun _ hx => h _ (.command he hx)
  | .send t src args :: rest, _, _, _, _, h => by
    rw [wp_send]; exact fun _ he => wp_weakest rest fun _ hx => h _ (.send he hx)
  | .refresh t :: rest, _, _, _, _, h => by
    rw [wp_refresh]; exact wp_weakest rest fun _ hx => h _ (.refresh hx)
  | .ifS c thn els :: rest, _, _, _, _, h => by
    rw [wp_ifS]
    exact ⟨fun hc => wp_weakest thn fun _ hb => wp_weakest rest fun _ hx => h _ (.ifTrue hc hb hx),
      fun hc => wp_weakest els fun _ hb => wp_weakest rest fun _ hx => h _ (.ifFalse hc hb hx)⟩
  | .matchS s x sm nn :: rest, _, _, _, _, h => by
    rw [wp_matchS]
    exact ⟨fun _ hc => wp_weakest sm fun _ hb => wp_weakest rest fun _ hx => h _ (.matchSome hc hb hx),
      fun hc => wp_weakest nn fun _ hb => wp_weakest rest fun _ hx => h _ (.matchNone hc hb hx)⟩
  | .call a args :: rest, _, _, _, _, h => by
    rw [wp_call]
    exact fun _ _ _ ha hd hl hb => wp_weakest rest fun _ hx => h _ (.call ha hd hl hb hx)

/-- A triple holds exactly when its precondition implies `wp`. -/
theorem triple_iff_wp {P ss Q} : Triple P ss Q ↔ ∀ env ls fx, P env ls fx → wp ss Q env ls fx :=
  ⟨fun h _ _ _ hp => wp_weakest ss fun _ hx => h _ _ _ _ hp hx,
   fun h _ _ _ _ hp hx => wp_sound hx (h _ _ _ hp)⟩

/-- A one-statement block followed by more, when it is not a `let`. -/
theorem Derives.cons {P R Q s rest} (hs : NoLet [s]) (h₁ : Derives P [s] R) (h₂ : Derives R rest Q) :
    Derives P (s :: rest) Q :=
  Derives.seq (ss₁ := [s]) hs h₁ h₂

/-- `wp` is derivable: the proof system proves every block's weakest
precondition. -/
theorem Derives.wp : ∀ (ss : List Stmt) (Q : Assn), Derives (wp ss Q) ss Q
  | [], _ => by rw [wp_nil]; exact .nil
  | .letS x e :: rest, Q => by
    rw [wp_letS]; exact .letS (fun _ _ _ _ hw he => hw _ he) (Derives.wp rest Q.outer)
  | .assign t e :: rest, Q => by rw [wp_assign]; exact .cons trivial .assign (Derives.wp rest Q)
  | .command n args :: rest, Q => by rw [wp_command]; exact .cons trivial .command (Derives.wp rest Q)
  | .send t src args :: rest, Q => by rw [wp_send]; exact .cons trivial .send (Derives.wp rest Q)
  | .refresh t :: rest, Q => by rw [wp_refresh]; exact .cons trivial .refresh (Derives.wp rest Q)
  | .ifS c thn els :: rest, Q => by
    rw [wp_ifS]
    exact .cons trivial
      (.ifS (.conseq (fun _ _ _ h => h.1.1 h.2) (Derives.wp thn _) (fun _ _ _ h => h))
            (.conseq (fun _ _ _ h => h.1.2 h.2) (Derives.wp els _) (fun _ _ _ h => h)))
      (Derives.wp rest Q)
  | .matchS s x sm nn :: rest, Q => by
    rw [wp_matchS]
    exact .cons trivial
      (.matchS (fun _ _ _ _ hw hc => hw.1 _ hc) (Derives.wp sm _)
        (.conseq (fun _ _ _ h => h.1.2 h.2) (Derives.wp nn _) (fun _ _ _ h => h)))
      (Derives.wp rest Q)
  | .call a args :: rest, Q => by rw [wp_call]; exact .cons trivial .call (Derives.wp rest Q)

/-- The proof system is complete relative to the assertion language:
every true triple is derivable. -/
theorem Derives.complete {P ss Q} (h : Triple P ss Q) : Derives P ss Q :=
  .conseq (triple_iff_wp.mp h) (Derives.wp ss Q) (fun _ _ _ h => h)

theorem derives_iff_triple {P ss Q} : Derives P ss Q ↔ Triple P ss Q :=
  ⟨Derives.sound, Derives.complete⟩

/-! ## Laws of statements -/

/-- Effects only accumulate: a block appends to each list and removes
nothing. -/
def Effects.Extends (fx fx' : Effects) : Prop :=
  (∃ ws, fx'.writes = fx.writes ++ ws) ∧ (∃ ws, fx'.rowWrites = fx.rowWrites ++ ws) ∧
  (∃ cs, fx'.commands = fx.commands ++ cs) ∧ (∃ ss, fx'.sends = fx.sends ++ ss) ∧
  (∃ rs, fx'.refreshes = fx.refreshes ++ rs)

theorem Effects.Extends.refl (fx : Effects) : fx.Extends fx :=
  ⟨⟨[], by simp⟩, ⟨[], by simp⟩, ⟨[], by simp⟩, ⟨[], by simp⟩, ⟨[], by simp⟩⟩

theorem Effects.Extends.trans {a b c : Effects} (h₁ : a.Extends b) (h₂ : b.Extends c) : a.Extends c := by
  obtain ⟨⟨w, hw⟩, ⟨r, hr⟩, ⟨m, hm⟩, ⟨s, hs⟩, ⟨f, hf⟩⟩ := h₁
  obtain ⟨⟨w', hw'⟩, ⟨r', hr'⟩, ⟨m', hm'⟩, ⟨s', hs'⟩, ⟨f', hf'⟩⟩ := h₂
  exact ⟨⟨w ++ w', by simp [hw', hw]⟩, ⟨r ++ r', by simp [hr', hr]⟩, ⟨m ++ m', by simp [hm', hm]⟩,
    ⟨s ++ s', by simp [hs', hs]⟩, ⟨f ++ f', by simp [hf', hf]⟩⟩

theorem ExecR.extends {env ls ss fx fx'} (h : ExecR env ls ss fx fx') : fx.Extends fx' := by
  induction h with
  | nil => exact .refl _
  | letS _ _ ih => exact ih
  | assignRoot _ _ _ ih | assignRow _ _ _ _ _ ih | command _ _ ih | send _ _ ih | refresh _ ih =>
    refine Effects.Extends.trans ?_ ih
    refine ⟨?_, ?_, ?_, ?_, ?_⟩ <;>
      first | exact ⟨_, rfl⟩
            | exact ⟨[], by simp [Effects.write, Effects.rowWrite, Effects.command, Effects.send,
                Effects.refresh]⟩
  | ifTrue _ _ _ ih₁ ih₂ | ifFalse _ _ _ ih₁ ih₂ | matchSome _ _ _ ih₁ ih₂ | matchNone _ _ _ ih₁ ih₂
  | call _ _ _ _ _ ih₁ ih₂ =>
    exact ih₁.trans ih₂

/-- Reads see the pre-state: after `x = e₁`, `y = x` writes the value `x`
had when the action began, not `e₁`'s. -/
theorem triple_read_prestate {x y e₁ v₀ fx₀} :
    Triple (fun env ls fx => fx = fx₀ ∧ lookup x ls = .none ∧ env.global x = .ok v₀ ∧
              isRootState env.prog x = true ∧ isRootState env.prog y = true)
      [.assign x e₁, .assign y (.var x)]
      (fun _ _ fx => ∃ v₁, fx = (fx₀.write x v₁).write y v₀) := by
  intro env ls fx fx' ⟨hfx, hl, hg, hx, hy⟩ h
  subst hfx
  cases h with
  | assignRoot _ _ h =>
    cases h with
    | assignRoot hv _ h =>
      cases h
      cases hv with
      | «local» hl' => rw [hl] at hl'; cases hl'
      | global _ _ hg' => rw [hg] at hg'; cases hg'; exact ⟨_, rfl⟩
    | assignRow _ hr => rw [hy] at hr; cases hr
  | assignRow _ hr => rw [hx] at hr; cases hr

/-! ## Actions as transactions -/

/-- The state an action reads: the configuration as the event found it. -/
def actionEnv (p : Program) (c : Config) (rows : List RowId) : Env :=
  { prog := p, slots := c.slots, derives := c.settled.derives, resources := c.settled.resources,
    rows := rowSlots c.store rows, now := c.now }

/-- An action's parameters bound to its arguments. -/
def actionLocals (a : ActionDecl) (args : List Value) : Locals := ((a.params.map (·.1)).zip args).reverse

/-- Writes applied in order to the slots. -/
def applyWrites (slots ws : List (String × Value)) : List (String × Value) :=
  ws.foldl (fun s (x, v) => setSlot s x v) slots

/-- Each send answered by the oracle, the answer for its mutation's slot. -/
inductive Answered (o : Oracle) : List (String × String × List Value) → List (String × Value) → Prop
  | nil : Answered o [] []
  | cons : o.ask src vs = .ok v → Answered o sends ans → Answered o ((m, src, vs) :: sends) ((m, .some v) :: ans)

theorem Answered.of_mapM {o : Oracle} {f : String × String × List Value → Result (String × Value)}
    (hf : ∀ m src vs r, f (m, src, vs) = .ok r → ∃ v, o.ask src vs = .ok v ∧ r = (m, .some v)) :
    ∀ {sends ans}, sends.mapM f = .ok ans → Answered o sends ans
  | [], ans, h => by simp at h; subst h; exact .nil
  | (m, src, vs) :: sends, ans, h => by
    simp only [List.mapM_cons, Except.bind_ok_iff, Except.pure_ok_iff] at h
    obtain ⟨r, hr, rs, hrs, rfl⟩ := h
    obtain ⟨v, hv, rfl⟩ := hf _ _ _ _ hr
    exact .cons hv (Answered.of_mapM hf hrs)

/-- A refused action leaves the configuration as it was. -/
theorem runAction_refused {p o c name args rows c' e}
    (h : runAction p o c name args rows = (c', .refused e)) : c' = c := by
  simp only [runAction] at h
  repeat' split at h
  all_goals first | (simp at h; done) | (simp at h; exact h.1.symm) | (cases h; rfl)

/-- An action on a poisoned configuration is refused. -/
theorem runAction_poisoned {p o c name args rows} (h : c.poisoned = true) :
    runAction p o c name args rows = (c, .refused (.refused "poisoned")) := by
  simp [runAction, h]

/-- What a step that was not refused committed: the action's body ran
(as `ExecR`) against the configuration as the event found it, its sends
were answered, and the slots are the old ones updated by the answers and
then the writes, in order. The commands it asked for are appended. This
holds of a poisoned commit too: it keeps the state it committed. -/
theorem runAction_commit {p o c name args rows c' out}
    (h : runAction p o c name args rows = (c', out)) (hout : ∀ e, out ≠ .refused e) :
    ∃ a fx answered, p.actions.find? (·.name == name) = .some a ∧
      ExecR (actionEnv p c rows) (actionLocals a args) a.body {} fx ∧
      Answered o (splitSends p c fx.sends).1 answered ∧
      c'.slots = applyWrites c.slots (answered ++ fx.writes) ∧
      c'.commands = c.commands ++ fx.commands := by
  have refused : ∀ {e}, (c, Outcome.refused e) = (c', out) → False := fun h' => by
    simp only [Prod.mk.injEq] at h'; exact hout _ h'.2.symm
  simp only [runAction] at h
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  next a ha =>
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  next fx hx =>
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  next answered hans =>
  have hA : Answered o (splitSends p c fx.sends).1 answered := Answered.of_mapM (by
    intro m src vs r hr
    simp only [Except.bind_ok_iff] at hr
    obtain ⟨v, hv, hr⟩ := hr
    refine ⟨v, hv, ?_⟩
    split at hr
    · obtain ⟨_, h', _⟩ := Except.bind_ok_iff.mp hr; cases h'
    · exact (Except.pure_ok_iff.mp hr).symm) hans
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  split at h
  · exact (refused h).elim
  split at h
  · simp only [Prod.mk.injEq] at h; obtain ⟨rfl, -⟩ := h
    exact ⟨a, fx, answered, ha, exec_sound hx, hA, rfl, rfl⟩
  · simp only [Prod.mk.injEq] at h; obtain ⟨rfl, -⟩ := h
    exact ⟨a, fx, answered, ha, exec_sound hx, hA, rfl, rfl⟩

/-- An action whose body has no derivation is refused. -/
theorem runAction_stuck {p o c name args rows a}
    (ha : p.actions.find? (·.name == name) = .some a)
    (hx : ∀ fx, ¬ ExecR (actionEnv p c rows) (actionLocals a args) a.body {} fx) :
    ∃ e, runAction p o c name args rows = (c, .refused e) := by
  match h : runAction p o c name args rows with
  | (c', .refused e) => exact ⟨e, by rw [runAction_refused h]⟩
  | (c', .ok) =>
    obtain ⟨a', fx, _, ha', hfx, _⟩ := runAction_commit h (by intro e he; cases he)
    rw [ha] at ha'; cases ha'; exact (hx _ hfx).elim
  | (c', .poisoned e) =>
    obtain ⟨a', fx, _, ha', hfx, _⟩ := runAction_commit h (by intro e he; cases he)
    rw [ha] at ha'; cases ha'; exact (hx _ hfx).elim

/-! ## The last write wins -/

theorem lookup_setSlot {x y : String} {w : Value} :
    ∀ {s : List (String × Value)}, lookup x (setSlot s y w) = (lookup x s).map fun u => if x = y then w else u
  | [] => rfl
  | (z, u) :: s => by
    have ih := @lookup_setSlot x y w s
    simp only [setSlot, List.map_cons] at ih ⊢
    by_cases hxz : x = z <;> by_cases hyz : y = z <;> simp_all [lookup] <;> intro h <;> simp_all

theorem lookup_append {x : String} :
    ∀ {l₁ l₂ : List (String × Value)}, lookup x (l₁ ++ l₂) = (lookup x l₁).or (lookup x l₂)
  | [], _ => by simp [lookup]
  | (y, v) :: l₁, l₂ => by
    by_cases h : x = y <;> simp [lookup, h, lookup_append]

/-- A slot after writes: the last write to it, if any, else its old value;
a write to a name that is not a slot is dropped. -/
theorem lookup_applyWrites {x : String} :
    ∀ {slots ws : List (String × Value)},
      lookup x (applyWrites slots ws) = (lookup x slots).map fun u => (lookup x ws.reverse).getD u
  | slots, [] => by simp [applyWrites, lookup]
  | slots, (y, w) :: ws => by
    have ih := @lookup_applyWrites x (setSlot slots y w) ws
    simp only [applyWrites, List.foldl_cons] at ih ⊢
    rw [ih, lookup_setSlot, List.reverse_cons, lookup_append]
    cases lookup x slots with
    | none => rfl
    | some u =>
      cases hr : lookup x ws.reverse with
      | some v => simp
      | none => by_cases h : x = y <;> simp [lookup, h]

theorem applyWrites_last {x v} {slots ws : List (String × Value)}
    (hw : lookup x ws.reverse = .some v) (hs : (lookup x slots).isSome) :
    lookup x (applyWrites slots ws) = .some v := by
  rw [lookup_applyWrites, hw]
  cases h : lookup x slots <;> simp_all

theorem lookup_reverse_none {x : String} :
    ∀ {ws : List (String × Value)}, lookup x ws = .none → lookup x ws.reverse = .none
  | [], _ => rfl
  | (y, w) :: ws, h => by
    by_cases hx : x = y
    · simp [lookup, hx] at h
    · simp [lookup, hx] at h
      simp [List.reverse_cons, lookup_append, lookup_reverse_none h, lookup, hx]

theorem applyWrites_unwritten {x} {slots ws : List (String × Value)} (hw : lookup x ws = .none) :
    lookup x (applyWrites slots ws) = lookup x slots := by
  rw [lookup_applyWrites, lookup_reverse_none hw]
  cases lookup x slots <;> rfl

theorem applyWrites_names {slots ws : List (String × Value)} :
    (applyWrites slots ws).map (·.1) = slots.map (·.1) := by
  induction ws generalizing slots with
  | nil => rfl
  | cons yw ws ih =>
    simp only [applyWrites, List.foldl_cons] at ih ⊢
    rw [ih]
    simp only [setSlot, List.map_map]
    congr 1; funext ⟨z, u⟩; simp only [Function.comp]; split <;> rfl

/-- After a commit, a slot holds the last value written to it by the
action (an answered send counting as a write before all the others). -/
theorem runAction_last_write {p o c name args rows c' out x v}
    (h : runAction p o c name args rows = (c', out)) (hout : ∀ e, out ≠ .refused e)
    (hs : (lookup x c.slots).isSome) :
    ∃ a fx answered, p.actions.find? (·.name == name) = .some a ∧
      ExecR (actionEnv p c rows) (actionLocals a args) a.body {} fx ∧
      Answered o (splitSends p c fx.sends).1 answered ∧
      (lookup x (answered ++ fx.writes).reverse = .some v → lookup x c'.slots = .some v) ∧
      (lookup x (answered ++ fx.writes) = .none → lookup x c'.slots = lookup x c.slots) := by
  obtain ⟨a, fx, ans, ha, hx, hA, hsl, -⟩ := runAction_commit h hout
  refine ⟨a, fx, ans, ha, hx, hA, fun hw => ?_, fun hw => ?_⟩
  · rw [hsl]; exact applyWrites_last hw hs
  · rw [hsl]; exact applyWrites_unwritten hw

/-- A commit keeps the configuration's slot names: writes only update. -/
theorem runAction_slot_names {p o c name args rows c' out}
    (h : runAction p o c name args rows = (c', out)) :
    c'.slots.map (·.1) = c.slots.map (·.1) := by
  cases out with
  | refused e => rw [runAction_refused h]
  | ok | poisoned =>
    obtain ⟨_, _, _, _, _, _, hsl, -⟩ := runAction_commit h (by intro e he; cases he)
    rw [hsl, applyWrites_names]

/-- Reads during an action see the pre-state: for the body
`x = e₁; y = x`, the committed `y` is the value `x` had before the action,
not `e₁`. (`x` is a root state and not a parameter; `y` is a root slot.) -/
theorem runAction_reads_prestate {p o c name args rows c' out a x y e₁ st}
    (h : runAction p o c name args rows = (c', out)) (hout : ∀ e, out ≠ .refused e)
    (ha : p.actions.find? (·.name == name) = .some a)
    (hbody : a.body = [.assign x e₁, .assign y (.var x)])
    (hparam : lookup x (actionLocals a args) = .none)
    (hst : p.states.find? (·.name == x) = .some st) (hown : st.owner = .none)
    (hy : isRootState p y = true) (hys : (lookup y c.slots).isSome) :
    lookup y c'.slots = lookup x c.slots := by
  obtain ⟨a', fx, ans, ha', hx, hA, hsl, -⟩ := runAction_commit h hout
  rw [ha] at ha'; cases ha'
  rw [hbody] at hx
  -- The second statement writes `x`'s pre-state value to `y`.
  have second : ∀ {fx₁}, ExecR (actionEnv p c rows) (actionLocals a args) [.assign y (.var x)] fx₁ fx →
      fx₁.sends = [] → fx.sends = [] ∧ ∃ v, lookup x c.slots = .some v ∧ fx = fx₁.write y v := by
    intro fx₁ h₂ hs₁
    cases h₂ with
    | assignRow _ hr => exact absurd hy (by simpa [actionEnv] using hr)
    | assignRoot hv _ hn =>
      cases hn
      cases hv with
      | «local» hl => rw [hparam] at hl; cases hl
      | global _ _ hg =>
        simp only [Env.global, actionEnv, hst, hown] at hg
        exact ⟨hs₁, _, Option.elim_err_ok.mp hg, rfl⟩
  have hw : ∃ ws v, fx.sends = [] ∧ lookup x c.slots = .some v ∧ fx.writes = ws ++ [(y, v)] := by
    cases hx with
    | assignRoot _ _ h₂ =>
      obtain ⟨hs, v, hv, rfl⟩ := second h₂ rfl
      exact ⟨_, v, hs, hv, rfl⟩
    | assignRow _ _ _ _ h₂ =>
      obtain ⟨hs, v, hv, rfl⟩ := second h₂ rfl
      exact ⟨_, v, hs, hv, rfl⟩
  obtain ⟨ws, v, hs, hv, hwr⟩ := hw
  rw [hs] at hA; cases hA
  rw [hsl, hv]
  exact applyWrites_last (by simp [hwr, lookup]) hys

/-- A poisoned configuration refuses every event. -/
theorem dispatch_poisoned {p o c target event payload} (h : c.poisoned = true) :
    dispatch p o c target event payload = (c, .refused (.refused "poisoned")) := by
  simp [dispatch, h]

/-- … and every clock advance that moves the clock forward. -/
theorem advance_poisoned {p o c t} (h : c.poisoned = true) (ht : ¬ t < c.now) :
    advance p o c t = (c, .refused (.refused "poisoned")) := by
  simp only [advance, ht, ite_false]
  rw [show timerFireLimit = 4095 + 1 from rfl, advance.go]
  simp [h]

end Contract
