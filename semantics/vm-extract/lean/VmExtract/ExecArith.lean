/-
The number opcodes: `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Lt`, `Le`, `Gt`, `Ge`.
-/
import VmExtract.Lemmas

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

/-- An instruction's single step refines the model's. -/
def ExecOk (i : Contract.Vm.Instr) : Prop :=
  ∀ (code : Contract.Vm.Code) (env : Contract.Vm.Env) (h : LHost) (m : machine.Machine Value Unit)
    (M : Contract.Vm.Machine) (ins : machine.Instruction) (next : Usize) r h' m',
    Rel code env h m M → encIns m.pos i = ok ins → next.val = M.pc + 1 → code[M.pc]? = some i →
    machine.exec valInst hostInst h { m with pos := next } ins = ok (r, h', m') →
    Out code env r h' m' (Contract.Vm.exec code.length env i M)

set_option maxHeartbeats 4000000 in
theorem exec_num2 (i : Contract.Vm.Instr) (op : machine.Num) (opc : Op)
    (henc : ∀ pc, encIns pc i = mkIns pc opc zero zero zero)
    (hexec : ∀ h m ins, ins.op = opc →
      machine.exec valInst hostInst h m ins = (do
        let (r, v) ← machine.num2 valInst m.stack op opc ins.pc
        let cf ← core.result.Result.Insts.CoreOpsTry.branch r
        match cf with
        | core.ops.control_flow.ControlFlow.Continue _ =>
          ok (core.result.Result.Ok none, h, { m with stack := v })
        | core.ops.control_flow.ControlFlow.Break residual => do
          let r1 ← core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual
            (Option Value) (core.convert.FromSame machine.Trap) residual
          ok (r1, h, { m with stack := v })))
    (hmodel : ∀ size env (M : Contract.Vm.Machine), Contract.Vm.exec size env i M =
      (match M.stack with
       | b :: a :: s =>
         (match a, b with
          | .num x, .num y => .ok (.run { M with pc := M.pc + 1, stack := numOp op x y :: s })
          | _, _ => .error (.typeMismatch (opName opc)))
       | _ => .error .stackUnderflow))
    (hname : opc ≠ .Call ∧ opc ≠ .NativeProps) :
    ExecOk i := by
  intro code env h m M ins next r h' m' hR hins hn hat hx
  obtain ⟨hc, he, hfx, hpos, hst, hloc, hcb⟩ := hR
  rw [henc] at hins
  obtain ⟨_, _, _, _, _, _, _, rfl⟩ := mkIns_ok hins
  rw [hexec _ _ _ rfl] at hx
  simp only [bind_eq_ok'] at hx
  obtain ⟨⟨r0, v0⟩, h2, hx⟩ := hx
  have := num2_rev hst op opc m.pos h2
  rw [hmodel]
  obtain ⟨pc, S, L, cbs, fx⟩ := M
  simp only at hfx hpos hst hloc hcb hn ⊢
  rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
  · simp only at this; subst this
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  · simp only at this; subst this
    simp at hx
    obtain ⟨rfl, rfl, rfl⟩ := hx
    simp [Out, TrapRel]
  · cases a <;> cases b <;> simp only at this ⊢ <;>
      first
      | (obtain ⟨rfl, hv⟩ := this
         simp at hx
         obtain ⟨rfl, rfl, rfl⟩ := hx
         simp only [Out, Rel, true_and]
         exact ⟨hc, he, hfx, hn, hv, hloc, hcb⟩)
      | (subst this
         simp at hx
         obtain ⟨rfl, rfl, rfl⟩ := hx
         exact ⟨_, rfl, trapRel_tm _ _ hname.1 hname.2⟩)

theorem arr_index0 (a : Array U64 3#usize) : ∃ x, a.index_usize 0#usize = ok x := by
  have : 0 < a.val.length := by simp
  exact ⟨a.val[0], by simp [Array.index_usize, Array.getElem?_Usize_eq]⟩

theorem exec_add : ExecOk .add := by
  refine exec_num2 _ .Add .Add (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_sub : ExecOk .sub := by
  refine exec_num2 _ .Sub .Sub (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_mul : ExecOk .mul := by
  refine exec_num2 _ .Mul .Mul (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_div : ExecOk .div := by
  refine exec_num2 _ .Div .Div (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_rem : ExecOk .rem := by
  refine exec_num2 _ .Rem .Rem (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_lt : ExecOk .lt := by
  refine exec_num2 _ .Lt .Lt (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_le : ExecOk .le := by
  refine exec_num2 _ .Le .Le (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_gt : ExecOk .gt := by
  refine exec_num2 _ .Gt .Gt (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

theorem exec_ge : ExecOk .ge := by
  refine exec_num2 _ .Ge .Ge (fun _ => rfl) ?_ ?_ ⟨nofun, nofun⟩
  · intro h m ins hop
    obtain ⟨x, hx⟩ := arr_index0 ins.args
    rw [machine.exec, hx, bind_ok, hop]; rfl
  · intro size env M
    obtain ⟨pc, S, L, cbs, fx⟩ := M
    rcases S with _ | ⟨b, _ | ⟨a, s⟩⟩
    · rfl
    · rfl
    · cases a <;> cases b <;> rfl

end VmExtract
