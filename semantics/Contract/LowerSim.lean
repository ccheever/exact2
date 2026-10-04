/-
The simulation relation between the semantics and the VM: how a scope's
names correspond to what the VM reads, and what the code of an expression,
an argument list, a template, a record's fields and a block must do for
the compiler to be correct. `Contract.LowerProof` proves the compiler meets
these.
-/
import Contract.LowerTypes

namespace Contract.Lower

open Vm

/-- A component-level name: `eval` answers it exactly when the VM's cell
holds a value, and the value is of the name's type. -/
def GlobalOk (sh : List Shape) (r : Result Value) (o : Option (Option Value)) (t : STy) : Prop :=
  (∀ v, r = .ok v ↔ o = some (some v)) ∧ ∀ v, o = some (some v) → VTy sh v t

/-- Name `x` resolved to `r` at type `t` reads, on the VM, what `eval`
reads for `.var x`. -/
def RefOk (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (L : List Value)
    (x : String) : Ref → STy → Prop
  | .local j, t => ∃ v, lookup x ls = some v ∧ L[j]? = some v ∧ VTy env.prog.shapes v t
  | .param j, t => ∃ v, lookup x ls = some v ∧ venv.params[j]? = some v ∧ VTy env.prog.shapes v t
  | .item d, t => ∃ v, lookup x ls = some v ∧ (venv.frames[d]?.bind (·.item)) = some v ∧
      VTy env.prog.shapes v t
  | .index d, t => ∃ k, lookup x ls = some (.num (Float.ofNat k)) ∧
      (venv.frames[d]?.bind (·.index)) = some k ∧ VTy env.prog.shapes (.num (Float.ofNat k)) t
  | .bound d, t => ∃ v, lookup x ls = some v ∧ (venv.frames[d]?.bind (·.bound)) = some v ∧
      VTy env.prog.shapes v t
  | .slot j, t => lookup x ls = none ∧ inFn = false ∧
      GlobalOk env.prog.shapes (env.global x) venv.slots[j]? t
  | .derive j, t => lookup x ls = none ∧ inFn = false ∧
      GlobalOk env.prog.shapes (env.global x) venv.derives[j]? t
  | .resource j, t => lookup x ls = none ∧ inFn = false ∧
      GlobalOk env.prog.shapes (env.global x) venv.resources[j]? t ∧
      env.prog.resources.any (·.name == x) = true ∧
      ((lookup x env.resources).isSome = true ↔ ∃ v, venv.resources[j]? = some (some v))
  | .mutation s _, t => lookup x ls = none ∧ inFn = false ∧
      GlobalOk env.prog.shapes (env.global x) venv.slots[s]? t ∧
      env.prog.resources.any (·.name == x) = false

/-- Every name the scope resolves corresponds. -/
def Agree (env : Contract.Env) (inFn : Bool) (ls : Locals) (venv : Vm.Env) (L : List Value)
    (sc : Scope) : Prop :=
  ∀ x r t, scopeLookup x sc = some (r, t) → RefOk env inFn ls venv L x r t

/-- Nothing in flight, and the same clock: the semantics answers
synchronously. -/
structure Quiet (env : Contract.Env) (venv : Vm.Env) : Prop where
  now : venv.now = env.now
  pendingResources : ∀ i : Nat, venv.pendingResources[i]?.getD false = false
  failedResources : ∀ i : Nat, venv.failedResources[i]?.getD false = false
  pendingMutations : ∀ i : Nat, venv.pendingMutations[i]?.getD false = false

theorem RefOk.append {env inFn ls venv L x r t} (h : RefOk env inFn ls venv L x r t) (ws : List Value) :
    RefOk env inFn ls venv (L ++ ws) x r t := by
  cases r <;> simp only [RefOk] at h ⊢ <;> try exact h
  obtain ⟨v, h1, h2, h3⟩ := h
  exact ⟨v, h1, by rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp h2).1]; exact h2, h3⟩

theorem RefOk.push {env inFn ls venv L x y r t w} (h : RefOk env inFn ls venv L x r t) (hne : (x == y) = false) :
    RefOk env inFn ((y, w) :: ls) venv L x r t := by
  have hl : lookup x ((y, w) :: ls) = lookup x ls := by simp [lookup, hne]
  cases r <;> simp only [RefOk, hl] at h ⊢ <;> exact h

theorem Agree.append {env inFn ls venv L sc} (h : Agree env inFn ls venv L sc) (ws : List Value) :
    Agree env inFn ls venv (L ++ ws) sc :=
  fun x r t hx => (h x r t hx).append ws

theorem Agree.push {env inFn ls venv L sc x w t} (h : Agree env inFn ls venv L sc)
    (hw : VTy env.prog.shapes w t) :
    Agree env inFn ((x, w) :: ls) venv (L ++ [w]) ((x, .local L.length, t) :: sc) := by
  intro y r u hy
  by_cases hxy : (y == x) = true
  · simp [scopeLookup, hxy] at hy
    obtain ⟨rfl, rfl⟩ := hy
    have : y = x := by simpa using hxy
    subst this
    exact ⟨w, by simp [lookup], by simp, hw⟩
  · simp only [Bool.not_eq_true] at hxy
    simp [scopeLookup, hxy] at hy
    exact ((h y r u hy).append [w]).push hxy

theorem Agree.nil {env inFn ls venv L} : Agree env inFn ls venv L [] := by
  intro x r t h; simp [scopeLookup] at h

/-- A callback's parameters. -/
theorem Agree.bind {env inFn ls venv L sc ps x i item} (h : Agree env inFn ls venv L sc)
    (hx : VTy env.prog.shapes x item) :
    Agree env inFn (bindParams ps x i ls) venv (L ++ [x, .num (Float.ofNat i)])
      (bindScope ps L.length item sc) := by
  match ps with
  | [] => exact h.append _
  | [p] =>
    have := (h.push (x := p) hx).append [.num (Float.ofNat i)]
    simpa [bindParams, bindScope] using this
  | p :: q :: _ =>
    have h1 := h.push (x := p) hx
    have h2 := h1.push (x := q) (w := .num (Float.ofNat i)) (t := .number) (by simp [VTy])
    simpa [bindParams, bindScope, Nat.add_comm] using h2

/-- A `fn`'s parameters, bound from the locals after `L`. -/
theorem Agree.fn {env venv} : ∀ {ps : List String} {vs : List Value} {ts : List STy} {L : List Value}
    {accLs : Locals} {accSc : Scope},
    Agree env true accLs venv L accSc → VTys env.prog.shapes vs ts →
    ps.length = vs.length →
    Agree env true (List.reverseAux (ps.zip vs) accLs) venv (L ++ vs)
      (List.reverseAux (fnEntries ps L.length ts) accSc)
  | [], [], _, L, _, _, h, _, _ => by simpa [List.reverseAux, fnEntries] using h
  | [], _ :: _, _, _, _, _, _, _, hl => by simp at hl
  | _ :: _, [], _, _, _, _, _, _, hl => by simp at hl
  | _ :: _, _ :: _, [], _, _, _, _, hts, _ => by simp [VTys] at hts
  | p :: ps, v :: vs, t :: ts, L, accLs, accSc, h, hts, hl => by
    have h1 := h.push (x := p) (w := v) hts.1
    have := Agree.fn (ps := ps) (vs := vs) (ts := ts) h1 hts.2 (by simpa using hl)
    simpa [List.reverseAux, fnEntries] using this

theorem fnScope_agree {env venv ps vs ts L} (hts : VTys env.prog.shapes vs ts) (hl : ps.length = vs.length) :
    Agree env true (ps.zip vs).reverse venv (L ++ vs) (fnScope ps L.length ts) := by
  have := Agree.fn (env := env) (venv := venv) (ps := ps) (accLs := []) (accSc := []) (L := L)
    Agree.nil hts hl
  exact this

/-! ## `eval` on names -/

theorem evalR_var {env inFn ls x v} :
    EvalR env inFn ls (.var x) v ↔
      lookup x ls = some v ∨ (lookup x ls = none ∧ inFn = false ∧ env.global x = .ok v) := by
  constructor
  · intro h; cases h
    · exact .inl (by assumption)
    · exact .inr ⟨by assumption, by assumption, by assumption⟩
  · rintro (h | ⟨h1, h2, h3⟩)
    · exact .local h
    · exact .global h1 h2 h3

/-! ## Strings -/

theorem join_cons (s : String) (ss : List String) : String.join (s :: ss) = s ++ String.join ss := by
  have key : ∀ (acc : String) (l : List String),
      l.foldl (fun r s => r ++ s) acc = acc ++ l.foldl (fun r s => r ++ s) "" := by
    intro acc l
    induction l generalizing acc with
    | nil => simp [String.append_empty]
    | cons x xs ih =>
      simp only [List.foldl]
      rw [ih (acc ++ x), ih ("" ++ x), String.empty_append, String.append_assoc]
  simp only [String.join, List.foldl]
  rw [key, String.empty_append]

theorem join_nil : String.join [] = "" := rfl

end Contract.Lower
