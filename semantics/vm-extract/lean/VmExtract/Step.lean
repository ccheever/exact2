/-
The shipped machine's step refines the model's: `machine::step`, extracted
from `runner/src/machine.rs`, on a state corresponding to a
`Contract.Vm.Machine` does what `Contract.Vm.step` does.
-/
import VmExtract.BodyEnd

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option maxHeartbeats 8000000

/-- Every instruction's single step refines the model's. -/
theorem exec_ok : ∀ i, ExecOk i
  | .num b => exec_numc b
  | .bool b => exec_bool b
  | .str s => exec_str s
  | .none => exec_none
  | .unit => exec_unit
  | .some => exec_some
  | .loadSlot j => exec_loadSlot j
  | .loadDerive j => exec_loadDerive j
  | .loadResource j => exec_loadResource j
  | .loadParam j => exec_loadParam j
  | .loadItem j => exec_loadItem j
  | .loadIndex j => exec_loadIndex j
  | .loadBound j => exec_loadBound j
  | .field j => exec_field j
  | .record s n => exec_record s n
  | .list n => exec_list n
  | .add => exec_add
  | .sub => exec_sub
  | .mul => exec_mul
  | .div => exec_div
  | .rem => exec_rem
  | .neg => exec_neg
  | .eq => exec_eq
  | .ne => exec_ne
  | .lt => exec_lt
  | .le => exec_le
  | .gt => exec_gt
  | .ge => exec_ge
  | .not => exec_not
  | .concat => exec_concat
  | .jump off => exec_jump off
  | .jumpIfFalse off => exec_jumpIfFalse off
  | .jumpIfNone off => exec_jumpIfNone off
  | .unwrap => exec_unwrap
  | .call f n => exec_call f n
  | .storeSlot j => exec_storeSlot j
  | .command nm n => exec_command nm n
  | .pop => exec_pop
  | .bindLocal => exec_bindLocal
  | .loadLocal j => exec_loadLocal j
  | .dropLocal => exec_dropLocal
  | .ret => exec_ret
  | .send k s n => exec_send k s n
  | .refresh r => exec_refresh r
  | .pendingResource r => exec_pendingResource r
  | .pendingMutation k => exec_pendingMutation k
  | .failedResource r => exec_failedResource r
  | .nativeProps n => exec_nativeProps n
  | .map off => exec_map off
  | .filter off => exec_filter off

/-- Fetch and run the instruction at `m.pos`: what the model does with the
code at `M.pc`. -/
theorem fetch_exec_ok (code env h m M) (hR : Rel code env h m M) {r h' m'}
    (hs : (do
      let (r, host1) ← hostInst.fetch h m.pos
      let cf ← core.result.Result.Insts.CoreOpsTry.branch r
      match cf with
      | core.ops.control_flow.ControlFlow.Continue val =>
        match val with
        | Option.none => ok (core.result.Result.Err machine.Trap.NoResult, host1, m)
        | Option.some p =>
          let (ins, next) := p
          machine.exec valInst hostInst host1 { m with pos := next } ins
      | core.ops.control_flow.ControlFlow.Break residual => do
        let r1 ← core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual
          (Option Value) (core.convert.FromSame machine.Trap) residual
        ok (r1, host1, m)) = ok (r, h', m')) :
    Out code env r h' m'
      (match code[M.pc]? with
       | Option.some i => Contract.Vm.exec code.length env i M
       | Option.none => .error .noResult) := by
  have hR' := hR
  obtain ⟨hc, he, hfx, hpos, hst, hloc, hcb⟩ := hR
  rw [bind_eq_ok'] at hs
  obtain ⟨⟨fr, h1⟩, hfr, hs⟩ := hs
  simp only [hostInst, LHost.fetch, hc, hpos] at hfr
  cases hi : code[M.pc]? with
  | none =>
    simp only [hi, ok.injEq, Prod.mk.injEq] at hfr
    obtain ⟨rfl, rfl⟩ := hfr
    simp only [branch_ok, bind_ok, uncurry_apply_pair] at hs
    try simp at hs
    obtain ⟨rfl, rfl, rfl⟩ := hs
    simp [Out, TrapRel]
  | some i =>
    simp only [hi] at hfr
    rw [bind_eq_ok'] at hfr
    obtain ⟨ins, hins, hfr⟩ := hfr
    rw [bind_eq_ok'] at hfr
    obtain ⟨next, hnext, hfr⟩ := hfr
    simp only [ok.injEq, Prod.mk.injEq] at hfr
    obtain ⟨rfl, rfl⟩ := hfr
    simp only [branch_ok, bind_ok, uncurry_apply_pair] at hs
    exact exec_ok i code env h m M ins next r h' m' hR' hins
      (by rw [usize_add_ok' hnext]; simp [hpos]) (by exact hi) hs

/-- **The step refinement.** On a Rust state that corresponds to the model's,
whenever the extracted `machine::step` returns, the model's `step` does the
same: runs on to a corresponding state, returns the same value with the same
effects, or traps with the corresponding trap. -/
theorem step_ok (code env h m M) (hR : Rel code env h m M) {r h' m'}
    (hs : machine.step valInst hostInst h m = ok (r, h', m')) :
    Out code env r h' m' (Contract.Vm.step code env M) := by
  have hR' := hR
  obtain ⟨hc, he, hfx, hpos, hst, hloc, hcb⟩ := hR
  rw [machine.step] at hs
  unfold Contract.Vm.step
  rcases hM : M.cbs with _ | ⟨c', cbs'⟩
  · have hnil := forall2_rev_nil (hM ▸ hcb)
    have hn : ¬ (alloc.vec.Vec.len m.callbacks > 0#usize) := by simp [hnil]
    simp only [hn, if_false] at hs
    exact fetch_exec_ok code env h m M hR' hs
  · obtain ⟨c, rest, hl, hcc, hrest⟩ := forall2_rev_cons (hM ▸ hcb)
    have hn : alloc.vec.Vec.len m.callbacks > 0#usize := by simp [hl]
    simp only [hn, if_true] at hs
    obtain ⟨i1, hi1, hsub⟩ := usize_sub_eq (alloc.vec.Vec.len m.callbacks) 1#usize (by simp [hl])
    have hidx : i1.val < m.callbacks.val.length := by simp at hi1; simp [hl] at hi1 ⊢; omega
    simp only [hsub, bind_ok, vec_index_eq _ _ hidx] at hs
    have hlast : m.callbacks.val[i1.val] = c := by
      simp only [hl] at hidx ⊢
      simp at hi1; simp [hl] at hi1
      simp [List.getElem_append_right, hi1]
    rw [hlast] at hs
    have hend := hcc.2.2.1
    by_cases he' : c'.stop = M.pc
    · have hpe : m.pos = c.end := by
        apply UScalar.val_eq_imp; rw [hpos, hend, he']
      simp only [hpe, if_true] at hs
      rw [bind_eq_ok'] at hs
      obtain ⟨⟨r1, h1, m1⟩, hb, hs⟩ := hs
      have hk : i1.val = rest.length := by simp at hi1; simp [hl] at hi1; omega
      have hbo := body_end_ok code env h m M c c' cbs' hc he hfx hpos hst hloc rest i1 hl hk
        (by simpa using hrest) hcc (by omega) hb
      dsimp only
      rw [if_pos he']
      rcases r1 with _ | e
      · simp at hs; obtain ⟨rfl, rfl, rfl⟩ := hs; simpa [stepOf] using hbo
      · simp at hs; obtain ⟨rfl, rfl, rfl⟩ := hs; simpa [stepOf] using hbo
    · have hpe : ¬ (m.pos = c.end) := by
        intro h0; apply he'; rw [← hend, ← hpos, h0]
      simp only [hpe, if_false, he'] at hs ⊢
      exact fetch_exec_ok code env h m M hR' hs

end VmExtract
