/-
A callback body's end: `machine::body_end` against `Contract.Vm.bodyEnd`.
-/
import VmExtract.ExecMap

namespace VmExtract
open Aeneas Aeneas.Std Result vm_extract
open Contract (Value)

set_option maxHeartbeats 16000000

/-- `body_end`'s outcome as a step's. -/
def stepOf : core.result.Result Unit Trap → core.result.Result (Option Value) Trap
  | .Ok _ => .Ok .none
  | .Err e => .Err e

theorem usize_ne_one (u : Usize) : ((u != 1#usize) = true) ↔ u.val ≠ 1 := by
  simp [UScalar.eq_equiv]

theorem usize_ne (u v : Usize) : ((u != v) = true) ↔ u.val ≠ v.val := by
  simp [UScalar.eq_equiv]

@[simp] theorem hostInst_kept (h : LHost) (e : Unit) (v : Value) (pc : Usize) :
    hostInst.kept h e v pc = ok (.Ok (), h) := rfl
@[simp] theorem hostInst_collected (h : LHost) (out : alloc.vec.Vec Value) (e : Unit) :
    hostInst.collected h out e = ok (.list out.val, h) := rfl

theorem usize_le_max (u : Usize) : u.val ≤ Usize.max := by scalar_tac


theorem body_end_ok (code env) (h : LHost) (m : machine.Machine Value Unit)
    (M : Contract.Vm.Machine) (c : machine.Callback Value Unit) (c' : Contract.Vm.Callback)
    (cbs' : List Contract.Vm.Callback)
    (hc : h.code = code) (he : h.env = env) (hfx : h.fx = M.fx) (hpos : m.pos.val = M.pc)
    (hst : m.stack.val = M.stack.reverse) (hloc : m.locals.val = M.locals)
    (rest : List (machine.Callback Value Unit)) (k : Usize)
    (hlc : m.callbacks.val = rest ++ [c]) (hk : k.val = rest.length)
    (hcb : List.Forall₂ CbRel rest.reverse cbs') (hcc : CbRel c c')
    (hstop : M.pc = c'.stop) {r h' m'} (hb : machine.body_end valInst hostInst h m k = ok (r, h', m')) :
    Out code env (stepOf r) h' m' (Contract.Vm.bodyEnd c' cbs' M) := by
  obtain ⟨hfil, hstart, hendv, hout, hbase, hcall, hnext, xs, hlist, hlenv, hcur, hrest, hsub⟩ :=
    hcc
  obtain ⟨pc, S, L, cbsM, fx⟩ := M
  simp only at hfx hpos hst hloc hstop ⊢
  simp only [machine.body_end] at hb
  have hidx : k.val < m.callbacks.val.length := by simp [hlc, hk]
  have hck : m.callbacks.val[k.val] = c := by simp [hlc, hk]
  simp only [vec_index_eq _ _ hidx, bind_ok] at hb
  rw [hck] at hb
  obtain ⟨back, hbm, hback⟩ := vec_index_mut_eq m.callbacks k hidx
  have hset : ∀ x, (back x).val = rest ++ [x] := by
    intro x; rw [hback, hlc, hk]; simp
  simp only [Contract.Vm.bodyEnd]
  rcases S with _ | ⟨v, _ | ⟨w, S⟩⟩
  · have : (alloc.vec.Vec.len m.stack != 1#usize) = true := by
      rw [usize_ne_one]; simp [hst]
    simp only [this, if_true] at hb
    simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
    simp [Out, TrapRel, stepOf]
  case cons.cons =>
    have : (alloc.vec.Vec.len m.stack != 1#usize) = true := by
      rw [usize_ne_one]; simp [hst]
    simp only [this, if_true] at hb
    simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
    simp [Out, TrapRel, stepOf]
  have h1 : ¬ ((alloc.vec.Vec.len m.stack != 1#usize) = true) := by
    rw [usize_ne_one]; simp [hst]
  simp only [h1, Bool.false_eq_true, if_false] at hb
  rw [bind_eq_ok'] at hb
  obtain ⟨i2, hi2, hb⟩ := hb
  have hi2v := usize_add_ok' hi2
  simp only [show (2#usize).val = 2 from rfl] at hi2v
  by_cases hl : L.length = c'.base + 2
  · have hl' : ¬ ((alloc.vec.Vec.len m.locals != i2) = true) := by
      rw [usize_ne]; simp [hloc, hi2v, hbase, hl]
    simp only [hl', Bool.false_eq_true, if_false] at hb
    simp only [hl, ne_eq, not_true_eq_false, if_false]
    obtain ⟨v1, hv1, hp⟩ := pop_rev (S := [v]) (by simpa using hst) c.end
    simp only [hp, List.head?_cons, bind_ok, uncurry_apply_pair, branch_ok, hbm] at hb
    rw [hck] at hb
    simp only [List.tail_cons, List.reverse_nil] at hv1
    cases hf : c'.filter
    · -- map
      have hfc : c.filter = false := by rw [hfil, hf]
      simp only [hfc, Bool.false_eq_true, if_false, bind_ok, uncurry_apply_pair,
        branch_ok, push_eq, hostInst_kept] at hb ⊢
      split at hb
      · rename_i hpush
        have hsub' : c'.filter = true → (c'.out ++ [v]).Sublist (xs.take (c'.idx + 1)) := by
          simp [hf]
        simp only [alloc.vec.Vec.truncate, bind_ok] at hb
        rw [bind_eq_ok'] at hb
        obtain ⟨i3, hi3, hb⟩ := hb
        have hi3v : i3.val = c'.idx + 1 := by
          have := usize_add_ok' hi3; rw [this, hnext]; rfl
        have hxs := List.getElem?_eq_some_iff.mp hcur
        rcases hR : c'.rest with _ | ⟨x1, rest1⟩
        · have hlast : c'.idx + 1 = xs.length := by
            have h0 : xs.length ≤ c'.idx + 1 := List.drop_eq_nil_iff.mp (by rw [← hrest, hR])
            have := hxs.1; omega
          have hge : ¬ (i3 < c.len) := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
          simp only [hge, if_false] at hb
          simp only [alloc.vec.Vec.pop, hset, List.getLast?_append, List.getLast?_singleton,
            Option.some_or, List.dropLast_concat, bind_ok, uncurry_apply_pair] at hb
          try simp only [hfc, if_true, if_false, Bool.false_eq_true, bind_ok, uncurry_apply_pair] at hb
          simp only [hostInst_collected, bind_ok, uncurry_apply_pair, push_eq] at hb
          split at hb
          · simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
            simp only [hR, pure_bind, except_bind_ok]
            simp only [Out, Rel, stepOf, true_and]
            refine ⟨hc, he, hfx, by omega, ?_, by simp [hloc, hbase], by simpa using hcb⟩
            simp [hcall, hout]
          · simp at hb
        · have hmore : c'.idx + 1 < xs.length := by
            have h0 : 0 < (xs.drop (c'.idx + 1)).length := by rw [← hrest, hR]; simp
            simp at h0; omega
          have hlt' : i3 < c.len := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
          simp only [hlt', if_true] at hb
          rw [bind_eq_ok'] at hb
          obtain ⟨⟨rb, hb', lb⟩, hbg, hb⟩ := hb
          obtain ⟨rfl, rfl, hlb⟩ := begin_ok _ _ _ (xs := xs) (by exact hlist)
            (by simp only [hi3v]; omega) (by rw [← hlenv]; exact usize_le_max _) hbg
          try simp only [branch_ok, bind_ok, push_eq] at hb
          try simp only [uncurry_apply_pair, branch_ok, bind_ok] at hb
          · (try simp at hb); obtain ⟨rfl, rfl, rfl⟩ := hb
            have hx1 : xs[c'.idx + 1]? = some x1 := by
              rw [← List.head?_drop, ← hrest, hR]; rfl
            simp only [hR, Out, Rel, stepOf, true_and]
            have hx1' := (List.getElem?_eq_some_iff.mp hx1).2
            refine ⟨hc, he, hfx, hstart, by simpa using hv1, ?_, ?_⟩
            · simp only [hlb, alloc.vec.Vec.from_val, hloc, hbase, hi3v, hx1']
            · simp only [hset, alloc.vec.Vec.from_val, List.reverse_append, List.reverse_cons,
                List.reverse_nil, List.nil_append, List.cons_append]
              refine List.Forall₂.cons ?_ hcb
              refine ⟨by simp_all, hstart, hendv, ?_, hbase, hcall, hi3v, xs, hlist, hlenv, hx1, ?_, ?_⟩
              · simp [hout]
              · have e1 : x1 :: rest1 = xs.drop (c'.idx + 1) := by rw [← hR, hrest]
                have e2 := congrArg List.tail e1
                simpa [List.tail_drop] using e2
              · intro hft
                simpa using hsub' (by simp_all)
      · simp at hb
    · -- filter
      have hfc : c.filter = true := by rw [hfil, hf]
      simp only [hfc, if_true, valInst_kind, kindOf_bind] at hb ⊢
      cases v
      case bool b =>
        simp only [KindOf, exists_eq_left] at hb
        cases b
        · simp only [Bool.false_eq_true, if_false] at hb ⊢
          have hsub' : c'.filter = true → (c'.out ++ []).Sublist (xs.take (c'.idx + 1)) := by
            intro hft
            simp only [List.append_nil]
            exact (hsub hft).trans (List.take_sublist_take_left (by omega))
          simp only [alloc.vec.Vec.truncate, bind_ok] at hb
          rw [bind_eq_ok'] at hb
          obtain ⟨i3, hi3, hb⟩ := hb
          have hi3v : i3.val = c'.idx + 1 := by
            have := usize_add_ok' hi3; rw [this, hnext]; rfl
          have hxs := List.getElem?_eq_some_iff.mp hcur
          rcases hR : c'.rest with _ | ⟨x1, rest1⟩
          · have hlast : c'.idx + 1 = xs.length := by
              have h0 : xs.length ≤ c'.idx + 1 := List.drop_eq_nil_iff.mp (by rw [← hrest, hR])
              have := hxs.1; omega
            have hge : ¬ (i3 < c.len) := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
            simp only [hge, if_false] at hb
            simp only [alloc.vec.Vec.pop, hset, List.getLast?_append, List.getLast?_singleton,
              Option.some_or, List.dropLast_concat, bind_ok, uncurry_apply_pair] at hb
            try simp only [hfc, if_true, if_false, Bool.false_eq_true, bind_ok, uncurry_apply_pair] at hb
            split at hb
            · rename_i heq
              exfalso
              have h1 := congrArg UScalar.val heq
              simp only [alloc.vec.Vec.len_val, hlenv, hout] at h1
              have h2 := (hsub hf).length_le
              simp at h2
              have := hxs.1
              simp only [alloc.vec.Vec.length] at h1
              have hol : (c.out.val).length = c'.out.length := by rw [hout]
              omega
            · simp only [hostInst_collected, bind_ok, uncurry_apply_pair, push_eq] at hb
              split at hb
              · simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
                simp only [hR, pure_bind, except_bind_ok, List.append_nil]
                simp only [Out, Rel, stepOf, true_and]
                refine ⟨hc, he, hfx, by omega, ?_, by simp [hloc, hbase], by simpa using hcb⟩
                simp [hcall, hout]
              · simp at hb
          · have hmore : c'.idx + 1 < xs.length := by
              have h0 : 0 < (xs.drop (c'.idx + 1)).length := by rw [← hrest, hR]; simp
              simp at h0; omega
            have hlt' : i3 < c.len := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
            simp only [hlt', if_true] at hb
            rw [bind_eq_ok'] at hb
            obtain ⟨⟨rb, hb', lb⟩, hbg, hb⟩ := hb
            obtain ⟨rfl, rfl, hlb⟩ := begin_ok _ _ _ (xs := xs) (by exact hlist)
              (by simp only [hi3v]; omega) (by rw [← hlenv]; exact usize_le_max _) hbg
            try simp only [branch_ok, bind_ok, push_eq] at hb
            try simp only [uncurry_apply_pair, branch_ok, bind_ok] at hb
            · (try simp at hb); obtain ⟨rfl, rfl, rfl⟩ := hb
              have hx1 : xs[c'.idx + 1]? = some x1 := by
                rw [← List.head?_drop, ← hrest, hR]; rfl
              simp only [hR, Out, Rel, stepOf, true_and]
              have hx1' := (List.getElem?_eq_some_iff.mp hx1).2
              refine ⟨hc, he, hfx, hstart, by simpa using hv1, ?_, ?_⟩
              · simp only [hlb, alloc.vec.Vec.from_val, hloc, hbase, hi3v, hx1']
              · simp only [hset, alloc.vec.Vec.from_val, List.reverse_append, List.reverse_cons,
                  List.reverse_nil, List.nil_append, List.cons_append]
                refine List.Forall₂.cons ?_ hcb
                refine ⟨by simp_all, hstart, hendv, ?_, hbase, hcall, hi3v, xs, hlist, hlenv, hx1, ?_, ?_⟩
                · simp [hout]
                · have e1 : x1 :: rest1 = xs.drop (c'.idx + 1) := by rw [← hR, hrest]
                  have e2 := congrArg List.tail e1
                  simpa [List.tail_drop] using e2
                · intro hft
                  simpa using hsub' (by simp_all)
        · simp only [if_true] at hb ⊢
          rw [item_ok hlist c.next (by rw [hnext]; exact (List.getElem?_eq_some_iff.mp hcur).1)
            (by rw [← hlenv]; exact usize_le_max _)] at hb
          simp only [bind_ok, branch_ok, hostInst_kept, uncurry_apply_pair, push_eq] at hb
          split at hb
          · have hxi : xs[c.next.val]'(by rw [hnext]; exact (List.getElem?_eq_some_iff.mp hcur).1)
                = c'.cur := by
              have := List.getElem?_eq_some_iff.mp hcur
              simp only [hnext]; exact this.2
            rw [hxi] at hb
            have hsub' : c'.filter = true → (c'.out ++ [c'.cur]).Sublist (xs.take (c'.idx + 1)) := by
              intro hft
              rw [List.take_succ, hcur]
              exact (hsub hft).append (List.Sublist.refl _)
            simp only [alloc.vec.Vec.truncate, bind_ok] at hb
            rw [bind_eq_ok'] at hb
            obtain ⟨i3, hi3, hb⟩ := hb
            have hi3v : i3.val = c'.idx + 1 := by
              have := usize_add_ok' hi3; rw [this, hnext]; rfl
            have hxs := List.getElem?_eq_some_iff.mp hcur
            rcases hR : c'.rest with _ | ⟨x1, rest1⟩
            · have hlast : c'.idx + 1 = xs.length := by
                have h0 : xs.length ≤ c'.idx + 1 := List.drop_eq_nil_iff.mp (by rw [← hrest, hR])
                have := hxs.1; omega
              have hge : ¬ (i3 < c.len) := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
              simp only [hge, if_false] at hb
              simp only [alloc.vec.Vec.pop, hset, List.getLast?_append, List.getLast?_singleton,
                Option.some_or, List.dropLast_concat, bind_ok, uncurry_apply_pair] at hb
              try simp only [hfc, if_true, if_false, Bool.false_eq_true, bind_ok, uncurry_apply_pair] at hb
              split at hb
              · rename_i heq
                have h1 := congrArg UScalar.val heq
                simp only [alloc.vec.Vec.len_val, hlenv] at h1
                simp only [alloc.vec.Vec.length, alloc.vec.Vec.from_val, List.length_append, hout] at h1
                have hxs' : c'.out ++ [c'.cur] = xs := by
                  have hs := hsub' hf
                  rw [List.take_of_length_le (by omega)] at hs
                  exact hs.eq_of_length (by simp; omega)
                simp only [bind_ok, uncurry_apply_pair, push_eq] at hb
                split at hb
                · simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
                  simp only [hR, pure_bind, except_bind_ok]
                  simp only [Out, Rel, stepOf, true_and]
                  refine ⟨hc, he, hfx, by omega, ?_, by simp [hloc, hbase], by simpa using hcb⟩
                  simp [hcall, hlist, hxs']
                · simp at hb
              · simp only [hostInst_collected, bind_ok, uncurry_apply_pair, push_eq] at hb
                split at hb
                · simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
                  simp only [hR, pure_bind, except_bind_ok]
                  simp only [Out, Rel, stepOf, true_and]
                  refine ⟨hc, he, hfx, by omega, ?_, by simp [hloc, hbase], by simpa using hcb⟩
                  simp [hcall, hout]
                · simp at hb
            · have hmore : c'.idx + 1 < xs.length := by
                have h0 : 0 < (xs.drop (c'.idx + 1)).length := by rw [← hrest, hR]; simp
                simp at h0; omega
              have hlt' : i3 < c.len := by simp only [UScalar.lt_equiv, hi3v, hlenv]; omega
              simp only [hlt', if_true] at hb
              rw [bind_eq_ok'] at hb
              obtain ⟨⟨rb, hb', lb⟩, hbg, hb⟩ := hb
              obtain ⟨rfl, rfl, hlb⟩ := begin_ok _ _ _ (xs := xs) (by exact hlist)
                (by simp only [hi3v]; omega) (by rw [← hlenv]; exact usize_le_max _) hbg
              try simp only [branch_ok, bind_ok, push_eq] at hb
              try simp only [uncurry_apply_pair, branch_ok, bind_ok] at hb
              · (try simp at hb); obtain ⟨rfl, rfl, rfl⟩ := hb
                have hx1 : xs[c'.idx + 1]? = some x1 := by
                  rw [← List.head?_drop, ← hrest, hR]; rfl
                simp only [hR, Out, Rel, stepOf, true_and]
                have hx1' := (List.getElem?_eq_some_iff.mp hx1).2
                refine ⟨hc, he, hfx, hstart, by simpa using hv1, ?_, ?_⟩
                · simp only [hlb, alloc.vec.Vec.from_val, hloc, hbase, hi3v, hx1']
                · simp only [hset, alloc.vec.Vec.from_val, List.reverse_append, List.reverse_cons,
                    List.reverse_nil, List.nil_append, List.cons_append]
                  refine List.Forall₂.cons ?_ hcb
                  refine ⟨by simp_all, hstart, hendv, ?_, hbase, hcall, hi3v, xs, hlist, hlenv, hx1, ?_, ?_⟩
                  · simp [hout]
                  · have e1 : x1 :: rest1 = xs.drop (c'.idx + 1) := by rw [← hR, hrest]
                    have e2 := congrArg List.tail e1
                    simpa [List.tail_drop] using e2
                  · intro hft
                    simpa using hsub' (by simp_all)
          · simp at hb
      all_goals
        simp [KindOf] at hb
        casesm* _ ∧ _, Exists _
        subst_vars
        simp [Out, TrapRel, opName, stepOf]
  · have hl' : (alloc.vec.Vec.len m.locals != i2) = true := by
      rw [usize_ne]; simp [hloc, hi2v, hbase]; omega
    simp only [hl', if_true] at hb
    simp only [hl, ne_eq, not_false_eq_true, if_true]
    simp at hb; obtain ⟨rfl, rfl, rfl⟩ := hb
    simp [Out, TrapRel, stepOf]

end VmExtract
