/-
What `Contract.Binary64` computes, proved.

Values are compared as integers: a finite double `y` is `y.scaled / 2^1074`
and the exact result of an operation is `A / (D * 2^1074)` for integers
`A`, `D > 0` (`a + b`: `A = a.scaled + b.scaled`, `D = 1`; `a * b`:
`A = a.scaled * b.scaled`, `D = 2^1074`; `a / b`: `A = a.scaled * 2^1074`,
`D = b.scaled`), so `|x - y| ≤ |x - y'|` is `|A - y.scaled * D| ≤ |A -
y'.scaled * D|` with no rational arithmetic.

* `round_nearest`: `round` gives a double nearest the exact value among
  all finite doubles, and `round_tie_even` a tie's even significand;
  `roundMag_eq_none_iff`: it overflows exactly from `2^1024 - 2^970`.
* `round_mono`: rounding is monotone; `roundMag_scale`: it depends on the
  value alone.
* `round_exact`: a representable value is its own rounding; so
  `ofNat_scaled`: every natural number up to `2^53` is a double exactly,
  `add_ofNat`/`sub_ofNat`: integer arithmetic there is exact, and
  `add_one_saturates`: `2^53 + 1` rounds back to `2^53`.
* `add_nearest`, `mul_nearest`, `div_nearest`: each operation is its exact
  result correctly rounded.
-/
import Contract.Binary64

namespace Contract
namespace F64

/-- `|x - y|` on naturals. -/
def dist (x y : Nat) : Nat := x - y + (y - x)

theorem natAbs_sub (x y : Nat) : ((x : Int) - y).natAbs = dist x y := by unfold dist; omega

/-! ## Fields -/

/-- A significand and unit exponent `make` encodes faithfully. -/
structure Valid (m p : Nat) : Prop where
  lt : m < 2 ^ 53
  norm : 2 ^ 52 ≤ m ∨ p = 0
  le : p ≤ 2045

theorem word_lt (x : F64) : x.word < 2 ^ 64 := x.bits.toNat_lt

theorem make_word {s m p} (h : Valid m p) :
    (make s m p).word = (if s then 2 ^ 63 else 0) +
      if m < 2 ^ 52 then m else (p + 1) * 2 ^ 52 + (m - 2 ^ 52) := by
  have := h.lt; have := h.le
  simp only [make, word]
  rw [UInt64.toNat_ofNat']
  apply Nat.mod_eq_of_lt
  split <;> split <;> omega

theorem make_sign {s m p} (h : Valid m p) : (make s m p).sign = s := by
  have := h.lt; have := h.le
  simp only [sign, make_word h]
  cases s <;> simp <;> split <;> omega

theorem make_bexp {s m p} (h : Valid m p) :
    (make s m p).bexp = if m < 2 ^ 52 then 0 else p + 1 := by
  have := h.lt; have := h.le
  simp only [bexp, make_word h]
  cases s <;> simp <;> split <;> omega

theorem make_frac {s m p} (h : Valid m p) :
    (make s m p).frac = if m < 2 ^ 52 then m else m - 2 ^ 52 := by
  have := h.lt; have := h.le
  simp only [frac, make_word h]
  cases s <;> simp <;> split <;> omega

theorem make_isFinite {s m p} (h : Valid m p) : (make s m p).isFinite = true := by
  have := h.le
  simp only [isFinite, make_bexp h]; split <;> simp <;> omega

theorem make_isNaN {s m p} (h : Valid m p) : (make s m p).isNaN = false := by
  have := h.le
  simp only [isNaN, make_bexp h]; split <;> simp <;> omega

theorem make_isInf {s m p} (h : Valid m p) : (make s m p).isInf = false := by
  have := h.le
  simp only [isInf, make_bexp h]; split <;> simp <;> omega

theorem make_mag {s m p} (h : Valid m p) : (make s m p).mag = m * 2 ^ p := by
  have := h.lt
  simp only [mag, sig, uexp, make_bexp h, make_frac h]
  by_cases hm : m < 2 ^ 52
  · have hp : p = 0 := h.norm.resolve_left (by omega)
    simp [hm, hp]
  · simp only [hm, ↓reduceIte]
    simp only [Nat.add_one_ne_zero, ↓reduceIte, Nat.add_sub_cancel]
    congr 1; omega

theorem make_scaled {s m p} (h : Valid m p) :
    (make s m p).scaled = if s then -((m * 2 ^ p : Nat) : Int) else ((m * 2 ^ p : Nat) : Int) := by
  simp only [scaled, make_sign h, make_mag h]

theorem zero_valid : Valid 0 0 := ⟨by decide, .inr rfl, by decide⟩

theorem sig_lt (x : F64) : x.sig < 2 ^ 53 := by
  have : x.frac < 2 ^ 52 := Nat.mod_lt _ (by decide)
  simp only [sig]; split <;> omega

theorem uexp_le {x : F64} (h : x.isFinite = true) : x.uexp ≤ 2045 := by
  have : x.bexp < 2048 := Nat.mod_lt _ (by decide)
  simp only [isFinite, bne_iff_ne, ne_eq] at h
  simp only [uexp]; omega

/-- Below `2^52` a significand's unit is the smallest. -/
theorem sig_norm (x : F64) : 2 ^ 52 ≤ x.sig ∨ x.uexp = 0 := by
  simp only [sig, uexp]; split <;> omega

theorem valid_of_isFinite {x : F64} (h : x.isFinite = true) : Valid x.sig x.uexp :=
  ⟨sig_lt x, sig_norm x, uexp_le h⟩

/-- The largest finite magnitude, scaled: `(2^53 - 1) * 2^2045`. -/
theorem mag_le {x : F64} (h : x.isFinite = true) : x.mag ≤ (2 ^ 53 - 1) * 2 ^ 2045 := by
  have h1 : x.sig ≤ 2 ^ 53 - 1 := by have := sig_lt x; omega
  have h2 : 2 ^ x.uexp ≤ 2 ^ 2045 := Nat.pow_le_pow_right (by decide) (uexp_le h)
  exact Nat.mul_le_mul h1 h2

theorem not_nan_inf_finite {x : F64} (hn : x.isNaN = false) (hi : x.isInf = false) :
    x.isFinite = true := by
  simp only [isNaN, isInf, isFinite, Bool.and_eq_false_iff, beq_eq_false_iff_ne, ne_eq,
    bne_eq_false_iff_eq, bne_iff_ne] at *
  omega

theorem finite_not_nan {x : F64} (h : x.isFinite = true) : x.isNaN = false := by
  simp only [isNaN, isFinite, bne_iff_ne, ne_eq] at *; simp [h]

theorem finite_not_inf {x : F64} (h : x.isFinite = true) : x.isInf = false := by
  simp only [isInf, isFinite, bne_iff_ne, ne_eq] at *; simp [h]

/-! ## Nearest integer, ties to even -/

theorem rne_cases (a d : Nat) :
    (rne a d = a / d ∧ 2 * (a % d) ≤ d) ∨ (rne a d = a / d + 1 ∧ d ≤ 2 * (a % d)) := by
  unfold rne
  dsimp only
  split
  · exact .inl ⟨rfl, by omega⟩
  · split
    · exact .inr ⟨rfl, by omega⟩
    · split
      · exact .inl ⟨rfl, by omega⟩
      · exact .inr ⟨rfl, by omega⟩

/-- `k * d` is at least `(f + 1) * d` above or `f * d` below. -/
theorem mul_side (k f d : Nat) : k * d ≤ f * d ∨ f * d + d ≤ k * d := by
  rcases Nat.lt_or_ge f k with h | h
  · exact .inr (by have := Nat.mul_le_mul_right d h; rwa [Nat.succ_mul] at this)
  · exact .inl (Nat.mul_le_mul_right d h)

/-- `k * d` is at least `(f + 2) * d` above, `(f - 1) * d` below, or one of
the two nearest multiples. -/
theorem mul_side' (k f d : Nat) :
    k * d + d ≤ f * d ∨ k = f ∨ k = f + 1 ∨ f * d + 2 * d ≤ k * d := by
  rcases Nat.lt_or_ge k f with h | h
  · exact .inl (by have := Nat.mul_le_mul_right d h; rwa [Nat.succ_mul] at this)
  · rcases Nat.lt_or_ge (f + 1) k with h' | h'
    · refine .inr (.inr (.inr ?_))
      have := Nat.mul_le_mul_right d h'
      rw [Nat.succ_mul, Nat.succ_mul] at this; omega
    · omega

/-- **The nearest multiple.** `rne a d * d` is a multiple of `d` nearest `a`. -/
theorem rne_nearest {a d : Nat} (hd : 0 < d) (k : Nat) :
    dist a (rne a d * d) ≤ dist a (k * d) := by
  have hdm := Nat.div_add_mod a d
  have hr := Nat.mod_lt a hd
  rw [Nat.mul_comm] at hdm
  have hs := mul_side k (a / d) d
  unfold dist
  rcases rne_cases a d with ⟨he, h2⟩ | ⟨he, h2⟩ <;> rw [he]
  · omega
  · rw [Nat.succ_mul]; omega

/-- **Ties to even.** Another multiple as near is a tie, and then `rne`'s is
even. -/
theorem rne_tie {a d : Nat} (hd : 0 < d) {k : Nat} (hk : k ≠ rne a d)
    (he : dist a (rne a d * d) = dist a (k * d)) : rne a d % 2 = 0 := by
  have hdm := Nat.div_add_mod a d
  have hr := Nat.mod_lt a hd
  rw [Nat.mul_comm] at hdm
  have hs := mul_side' k (a / d) d
  have hs1 : (a / d + 1) * d = a / d * d + d := Nat.succ_mul _ _
  unfold dist at he
  unfold rne at hk he ⊢
  dsimp only at hk he ⊢
  by_cases h1 : 2 * (a % d) < d
  · simp only [h1, ↓reduceIte] at hk he
    rcases hs with hs | rfl | rfl | hs
    · omega
    · exact absurd rfl hk
    · rw [hs1] at he; omega
    · omega
  · by_cases h2 : d < 2 * (a % d)
    · simp only [h1, h2, ↓reduceIte, ↓reduceIte] at hk he
      rcases hs with hs | rfl | rfl | hs
      · rw [hs1] at he; omega
      · rw [hs1] at he; omega
      · exact absurd rfl hk
      · rw [hs1] at he; omega
    · simp only [h1, h2, ↓reduceIte]
      split <;> omega

theorem rne_scale (a d c : Nat) (hc : 0 < c) : rne (a * c) (d * c) = rne a d := by
  unfold rne
  dsimp only
  rw [Nat.mul_div_mul_right _ _ hc, Nat.mul_mod_mul_right]
  have e1 : 2 * (a % d * c) = 2 * (a % d) * c := by rw [Nat.mul_assoc]
  have i1 : 2 * (a % d) * c < d * c ↔ 2 * (a % d) < d := Nat.mul_lt_mul_right hc
  have i2 : d * c < 2 * (a % d) * c ↔ d < 2 * (a % d) := Nat.mul_lt_mul_right hc
  simp only [e1, i1, i2]


/-! ## The unit exponent -/

/-- `p` is the unit exponent of the binade of `a / d`. -/
def UlpSpec (a d p : Nat) : Prop := a < d * 2 ^ (p + 53) ∧ (p = 0 ∨ d * 2 ^ (p + 52) ≤ a)

theorem ulpExp_spec {a d : Nat} (ha : 0 < a) (hd : 0 < d) : UlpSpec a d (ulpExp a d) := by
  have la1 := Nat.log2_self_le (Nat.pos_iff_ne_zero.mp ha)
  have la2 := Nat.lt_log2_self (n := a)
  have ld1 := Nat.log2_self_le (Nat.pos_iff_ne_zero.mp hd)
  have ld2 := Nat.lt_log2_self (n := d)
  unfold ulpExp UlpSpec
  dsimp only
  generalize a.log2 = la at *
  generalize d.log2 = ld at *
  split
  · rename_i h
    obtain ⟨hp, hlt⟩ := h
    refine ⟨?_, .inr ?_⟩
    · have : la - (ld + 52) - 1 + 53 = la - (ld + 52) + 52 := by omega
      rw [this]; exact hlt
    · have e : ld + 1 + (la - (ld + 52) - 1 + 52) = la := by omega
      have : d * 2 ^ (la - (ld + 52) - 1 + 52) < 2 ^ (ld + 1) * 2 ^ (la - (ld + 52) - 1 + 52) :=
        Nat.mul_lt_mul_of_pos_right ld2 (Nat.two_pow_pos _)
      rw [← Nat.pow_add, e] at this
      omega
  · rename_i h
    refine ⟨?_, ?_⟩
    · have h1 : 2 ^ (la + 1) ≤ 2 ^ ld * 2 ^ (la - (ld + 52) + 53) := by
        rw [← Nat.pow_add]; exact Nat.pow_le_pow_right (by decide) (by omega)
      have h2 : 2 ^ ld * 2 ^ (la - (ld + 52) + 53) ≤ d * 2 ^ (la - (ld + 52) + 53) :=
        Nat.mul_le_mul_right _ ld1
      omega
    · by_cases h0 : la - (ld + 52) = 0
      · exact .inl h0
      · exact .inr (Nat.le_of_not_lt fun h' => h ⟨by omega, h'⟩)

theorem ulp_le {a d p q : Nat} (hp : UlpSpec a d p) (hq : q = 0 ∨ d * 2 ^ (q + 52) ≤ a) : q ≤ p := by
  rcases hq with rfl | hq
  · exact Nat.zero_le _
  · have : 2 ^ (q + 52) < 2 ^ (p + 53) := Nat.lt_of_mul_lt_mul_left (Nat.lt_of_le_of_lt hq hp.1)
    have := (Nat.pow_lt_pow_iff_right (by decide)).mp this
    omega

theorem ulp_unique {a d p q : Nat} (hp : UlpSpec a d p) (hq : UlpSpec a d q) : p = q :=
  Nat.le_antisymm (ulp_le hq hp.2) (ulp_le hp hq.2)

theorem ulpSpec_scale {a d p c : Nat} (hc : 0 < c) : UlpSpec (a * c) (d * c) p ↔ UlpSpec a d p := by
  unfold UlpSpec
  have e : ∀ k, d * c * 2 ^ k = d * 2 ^ k * c := fun k => Nat.mul_right_comm _ _ _
  rw [e, e, Nat.mul_lt_mul_right hc, Nat.mul_le_mul_right_iff hc]

theorem ulpExp_scale {a d c : Nat} (ha : 0 < a) (hd : 0 < d) (hc : 0 < c) :
    ulpExp (a * c) (d * c) = ulpExp a d :=
  ulp_unique ((ulpSpec_scale hc).mp (ulpExp_spec (Nat.mul_pos ha hc) (Nat.mul_pos hd hc)))
    (ulpExp_spec ha hd)

/-! ## Rounding a magnitude -/

/-- The quotient at the unit exponent is a significand: below `2^53`, and
at least `2^52` unless the unit is the smallest. -/
theorem quot_bounds {a d p : Nat} (hd : 0 < d) (hs : UlpSpec a d p) :
    a / (d * 2 ^ p) < 2 ^ 53 ∧ (p = 0 ∨ 2 ^ 52 ≤ a / (d * 2 ^ p)) := by
  have hD : 0 < d * 2 ^ p := Nat.mul_pos hd (Nat.two_pow_pos _)
  have e53 : d * 2 ^ p * 2 ^ 53 = d * 2 ^ (p + 53) := by rw [Nat.mul_assoc, ← Nat.pow_add]
  have e52 : d * 2 ^ p * 2 ^ 52 = d * 2 ^ (p + 52) := by rw [Nat.mul_assoc, ← Nat.pow_add]
  constructor
  · rw [Nat.div_lt_iff_lt_mul hD, Nat.mul_comm, e53]
    exact hs.1
  · rcases hs.2 with h | h
    · exact .inl h
    · exact .inr ((Nat.le_div_iff_mul_le hD).mpr (by rw [Nat.mul_comm, e52]; exact h))

theorem roundMag_some {a d m q : Nat} (hd : 0 < d) (hs : UlpSpec a d (ulpExp a d))
    (h : roundMag a d = some (m, q)) :
    Valid m q ∧ m * 2 ^ q = rne a (d * 2 ^ ulpExp a d) * 2 ^ ulpExp a d ∧
      (rne a (d * 2 ^ ulpExp a d) % 2 = 0 → m % 2 = 0) := by
  have hb := quot_bounds hd hs
  have hc := rne_cases a (d * 2 ^ ulpExp a d)
  unfold roundMag at h
  dsimp only at h
  generalize ulpExp a d = p at *
  generalize rne a (d * 2 ^ p) = m0 at *
  by_cases h53 : m0 = 2 ^ 53
  · subst h53
    simp only [↓reduceIte] at h
    split at h
    · rename_i hq
      simp only [Option.some.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      refine ⟨⟨by decide, .inl (by decide), hq⟩, ?_, fun _ => by decide⟩
      rw [← Nat.pow_add, ← Nat.pow_add]; congr 1; omega
    · exact absurd h (by simp)
  · simp only [h53, ↓reduceIte] at h
    split at h
    · rename_i hq
      simp only [Option.some.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      refine ⟨⟨by omega, ?_, hq⟩, rfl, id⟩
      rcases hb.2 with h0 | h0
      · exact .inr h0
      · exact .inl (by omega)
    · exact absurd h (by simp)

theorem roundMag_none {a d : Nat} (h : roundMag a d = none) :
    2045 < (if rne a (d * 2 ^ ulpExp a d) = 2 ^ 53 then ulpExp a d + 1 else ulpExp a d) := by
  unfold roundMag at h
  dsimp only at h
  generalize rne a (d * 2 ^ ulpExp a d) = m0 at *
  by_cases h53 : m0 = 2 ^ 53 <;> simp only [h53, ↓reduceIte] at h ⊢ <;> split at h <;>
    first | omega | exact absurd h (by simp)

/-- A finite magnitude is a multiple of `2^p`, or below `2^52 * 2^p`. -/
theorem mag_grid {y : F64} (p : Nat) :
    (∃ k, y.mag = k * 2 ^ p) ∨ (0 < p ∧ y.mag < 2 ^ 52 * 2 ^ p) := by
  rcases Nat.lt_or_ge y.uexp p with h | h
  · refine .inr ⟨by omega, ?_⟩
    have h1 : y.sig * 2 ^ y.uexp < 2 ^ 53 * 2 ^ y.uexp :=
      Nat.mul_lt_mul_of_pos_right (sig_lt y) (Nat.two_pow_pos _)
    have h2 : 2 ^ 53 * 2 ^ y.uexp ≤ 2 ^ 52 * 2 ^ p := by
      have : 53 + y.uexp ≤ 52 + p := by omega
      rw [← Nat.pow_add, ← Nat.pow_add]; exact Nat.pow_le_pow_right (by decide) this
    exact Nat.lt_of_lt_of_le h1 h2
  · refine .inl ⟨y.sig * 2 ^ (y.uexp - p), ?_⟩
    rw [Nat.mul_assoc, ← Nat.pow_add, Nat.sub_add_cancel h]; rfl

/-- **Nearest, on magnitudes.** -/
theorem roundMag_nearest {a d m q : Nat} (ha : 0 < a) (hd : 0 < d) (h : roundMag a d = some (m, q))
    (y : F64) : dist a (m * 2 ^ q * d) ≤ dist a (y.mag * d) ∧
      (dist a (m * 2 ^ q * d) = dist a (y.mag * d) → y.mag ≠ m * 2 ^ q → m % 2 = 0) := by
  have hs := ulpExp_spec ha hd
  obtain ⟨-, hM, hev⟩ := roundMag_some hd hs h
  generalize hp : ulpExp a d = p at *
  have hD : 0 < d * 2 ^ p := Nat.mul_pos hd (Nat.two_pow_pos _)
  have eM : m * 2 ^ q * d = rne a (d * 2 ^ p) * (d * 2 ^ p) := by
    rw [hM, Nat.mul_comm d, Nat.mul_assoc]
  rw [eM]
  rcases mag_grid (y := y) p with ⟨k, hk⟩ | ⟨hp0, hlt⟩
  · have ek : y.mag * d = k * (d * 2 ^ p) := by rw [hk, Nat.mul_comm d, Nat.mul_assoc]
    rw [ek]
    refine ⟨rne_nearest hD k, fun he hne => hev (rne_tie hD (k := k) ?_ he)⟩
    rintro rfl; exact hne (by rw [hk, hM])
  · have h52 : d * 2 ^ (p + 52) ≤ a := hs.2.resolve_left (by omega)
    have e52 : d * 2 ^ (p + 52) = 2 ^ 52 * (d * 2 ^ p) := by
      rw [Nat.pow_add, Nat.mul_comm (2 ^ p), ← Nat.mul_assoc, Nat.mul_comm d, Nat.mul_assoc]
    have hlt' : y.mag * d < 2 ^ 52 * (d * 2 ^ p) := by
      have := Nat.mul_lt_mul_of_pos_right hlt hd
      rwa [Nat.mul_assoc, Nat.mul_comm (2 ^ p) d] at this
    have hn := rne_nearest hD (a := a) (2 ^ 52)
    unfold dist at hn ⊢
    refine ⟨by omega, fun he _ => by omega⟩


/-! ## Overflow -/

theorem roundMag_eq_none_iff' {a d : Nat} : roundMag a d = none ↔
    2045 < (if rne a (d * 2 ^ ulpExp a d) = 2 ^ 53 then ulpExp a d + 1 else ulpExp a d) := by
  unfold roundMag
  dsimp only
  split <;> simp <;> omega

/-- **Overflow.** `a / d` rounds past the largest finite exactly when it
is at least `2^1024 - 2^970` (scaled: `(2^54 - 1) * 2^2044`), where the
largest finite and `2^1024` are equally near and `2^1024`'s significand is
the even one. -/
theorem roundMag_eq_none_iff {a d : Nat} (ha : 0 < a) (hd : 0 < d) :
    roundMag a d = none ↔ (2 ^ 54 - 1) * (d * 2 ^ 2044) ≤ a := by
  have hs := ulpExp_spec ha hd
  have hb := quot_bounds hd hs
  rw [roundMag_eq_none_iff']
  generalize ulpExp a d = p at *
  have hD : 0 < d * 2 ^ p := Nat.mul_pos hd (Nat.two_pow_pos _)
  have hdm := Nat.div_add_mod a (d * 2 ^ p)
  have hr := Nat.mod_lt a hD
  have hX : 0 < d * 2 ^ 2044 := Nat.mul_pos hd (Nat.two_pow_pos _)
  -- The binade against the threshold.
  have small : p ≤ 2044 → d * 2 ^ (p + 53) ≤ 2 ^ 53 * (d * 2 ^ 2044) := fun h => by
    have : 2 ^ (p + 53) ≤ 2 ^ (53 + 2044) := Nat.pow_le_pow_right (by decide) (by omega)
    rw [Nat.pow_add 2 53 2044] at this
    have := Nat.mul_le_mul_left d this
    rwa [Nat.mul_left_comm] at this
  have big : 2046 ≤ p → 2 ^ 54 * (d * 2 ^ 2044) ≤ d * 2 ^ (p + 52) := fun h => by
    have : 2 ^ (54 + 2044) ≤ 2 ^ (p + 52) := Nat.pow_le_pow_right (by decide) (by omega)
    rw [Nat.pow_add 2 54 2044] at this
    have := Nat.mul_le_mul_left d this
    rwa [Nat.mul_left_comm] at this
  have at45 : p = 2045 → d * 2 ^ p = 2 * (d * 2 ^ 2044) := fun h => by
    subst h; rw [Nat.pow_succ, Nat.mul_comm (2 ^ 2044) 2, Nat.mul_left_comm]
  have up : a / (d * 2 ^ p) % 2 = 1 → d * 2 ^ p ≤ 2 * (a % (d * 2 ^ p)) →
      rne a (d * 2 ^ p) = a / (d * 2 ^ p) + 1 := fun h1 h2 => by
    unfold rne; dsimp only
    have : ¬ 2 * (a % (d * 2 ^ p)) < d * 2 ^ p := by omega
    simp only [this, ↓reduceIte]
    split
    · rfl
    · simp; omega
  have hc := rne_cases a (d * 2 ^ p)
  generalize rne a (d * 2 ^ p) = m0 at *
  generalize d * 2 ^ 2044 = X at *
  generalize a / (d * 2 ^ p) = f at *
  generalize a % (d * 2 ^ p) = r at *
  constructor
  · intro h
    by_cases hp : 2046 ≤ p
    · have := big hp; have := hs.2.resolve_left (by omega); omega
    · have hp45 : p = 2045 := by split at h <;> omega
      have h53 : m0 = 2 ^ 53 := by split at h <;> omega
      have := at45 hp45
      generalize d * 2 ^ p = D at *
      have hf : f = 2 ^ 53 - 1 := by omega
      subst hf
      omega
  · intro h
    by_cases hp : 2046 ≤ p
    · split <;> omega
    · by_cases hp4 : p ≤ 2044
      · have := small hp4; have := hs.1; omega
      · have hp45 : p = 2045 := by omega
        have := at45 hp45
        generalize d * 2 ^ p = D at *
        have hf : f = 2 ^ 53 - 1 := by
          rcases Nat.lt_or_ge f (2 ^ 53 - 1) with hf | hf
          · have := Nat.mul_le_mul_left D (show f ≤ 2 ^ 53 - 2 by omega)
            omega
          · omega
        subst hf
        have := up (by decide) (by omega)
        subst this
        simp only [↓reduceIte]
        omega

/-! ## Rounding depends on the value alone -/

theorem roundMag_scale {a d c : Nat} (ha : 0 < a) (hd : 0 < d) (hc : 0 < c) :
    roundMag (a * c) (d * c) = roundMag a d := by
  unfold roundMag
  dsimp only
  rw [ulpExp_scale ha hd hc, Nat.mul_right_comm d c, rne_scale _ _ _ hc]

/-! ## The doubles `round` gives -/

theorem round_of_some {s a d m q} (hd : 0 < d) (ha : 0 < a) (h : roundMag a d = some (m, q)) :
    round s a d = make s m q ∧ Valid m q := by
  refine ⟨by unfold round; rw [h], (roundMag_some hd (ulpExp_spec ha hd) h).1⟩

theorem round_of_none {s a d} (h : roundMag a d = none) : round s a d = inf s := by
  unfold round; rw [h]

theorem inf_isInf (s : Bool) : (inf s).isInf = true := by cases s <;> decide +kernel
theorem inf_sign (s : Bool) : (inf s).sign = s := by cases s <;> decide +kernel
theorem inf_isNaN (s : Bool) : (inf s).isNaN = false := by cases s <;> decide +kernel
theorem nan_isNaN : nan.isNaN = true := by decide +kernel

theorem key_make {s m q} (hv : Valid m q) :
    (make s m q).key = if s then -((m * 2 ^ q : Nat) : Int) else ((m * 2 ^ q : Nat) : Int) := by
  simp only [key, make_isInf hv, Bool.false_eq_true, ↓reduceIte, make_scaled hv]

theorem key_inf (s : Bool) :
    (inf s).key = if s then -((2 ^ 2100 : Nat) : Int) else ((2 ^ 2100 : Nat) : Int) := by
  simp only [key, inf_isInf, inf_sign, ↓reduceIte, Int.natCast_pow]; rfl

/-- The largest finite magnitude is below `2^2100`, the infinities' key. -/
theorem mag_lt_key {x : F64} (h : x.isFinite = true) : x.mag < 2 ^ 2100 :=
  Nat.lt_of_le_of_lt (mag_le h) (by decide +kernel)

theorem round_false_key {a d : Nat} (ha : 0 < a) (hd : 0 < d) :
    ∃ M : Nat, M ≤ 2 ^ 2100 ∧ (round false a d).key = M ∧
      ∀ m q, roundMag a d = some (m, q) → M = m * 2 ^ q := by
  cases h : roundMag a d with
  | none => exact ⟨2 ^ 2100, Nat.le_refl _, by rw [round_of_none h, key_inf]; rfl, by simp⟩
  | some mq =>
    obtain ⟨m, q⟩ := mq
    obtain ⟨hr, hv⟩ := round_of_some (s := false) hd ha h
    refine ⟨m * 2 ^ q, ?_, by simp [hr, key_make hv], by simp⟩
    have := mag_lt_key (make_isFinite (s := false) hv)
    rw [make_mag hv] at this; omega

/-- **Monotonicity.** A larger magnitude never rounds smaller. -/
theorem round_mono {a d a' d' : Nat} (ha : 0 < a) (hd : 0 < d) (ha' : 0 < a') (hd' : 0 < d')
    (h : a * d' ≤ a' * d) : (round false a d).key ≤ (round false a' d').key := by
  obtain ⟨M, hM, hk, hMs⟩ := round_false_key ha hd
  obtain ⟨M', hM', hk', hMs'⟩ := round_false_key ha' hd'
  rw [hk, hk']
  cases h' : roundMag a' d' with
  | none =>
    have : M' = 2 ^ 2100 := by
      have := congrArg key (round_of_none (s := false) h')
      rw [hk', key_inf] at this
      exact Int.ofNat.inj this
    omega
  | some mq' =>
    obtain ⟨m', q'⟩ := mq'
    cases h0 : roundMag a d with
    | none =>
      exfalso
      have t := (roundMag_eq_none_iff ha hd).mp h0
      have t' : (2 ^ 54 - 1) * (d' * 2 ^ 2044) * d ≤ a' * d := by
        have e : (2 ^ 54 - 1) * (d' * 2 ^ 2044) * d = (2 ^ 54 - 1) * (d * 2 ^ 2044) * d' := by
          generalize (2 : Nat) ^ 54 - 1 = c; generalize (2 : Nat) ^ 2044 = P; ac_rfl
        rw [e]; exact Nat.le_trans (Nat.mul_le_mul_right d' t) h
      have := (roundMag_eq_none_iff ha' hd').mpr (Nat.le_of_mul_le_mul_right t' hd)
      rw [h'] at this; cases this
    | some mq =>
      obtain ⟨m, q⟩ := mq
      have eM := hMs m q h0
      have eM' := hMs' m' q' h'
      obtain ⟨-, hv⟩ := round_of_some (s := false) hd ha h0
      obtain ⟨-, hv'⟩ := round_of_some (s := false) hd' ha' h'
      apply Int.ofNat_le.mpr
      apply Nat.le_of_not_lt
      intro hlt
      have n1 := (roundMag_nearest ha hd h0 (make false m' q')).1
      have n2 := (roundMag_nearest ha' hd' h' (make false m q)).1
      rw [make_mag hv'] at n1
      rw [make_mag hv] at n2
      rw [← eM, ← eM'] at n1 n2
      have l1 : M' * d < M * d := Nat.mul_lt_mul_of_pos_right hlt hd
      have l2 : M' * d' < M * d' := Nat.mul_lt_mul_of_pos_right hlt hd'
      unfold dist at n1 n2
      have i1 : M * d + M' * d ≤ 2 * a := by omega
      have i2 : 2 * a' ≤ M * d' + M' * d' := by omega
      have j1 := Nat.mul_le_mul_right d' i1
      have j2 := Nat.mul_le_mul_right d i2
      have e1 : (M * d + M' * d) * d' = (M * d' + M' * d') * d := by
        rw [Nat.add_mul, Nat.add_mul]; ac_rfl
      have e2 : 2 * a * d' = 2 * (a * d') := Nat.mul_assoc _ _ _
      have e3 : 2 * a' * d = 2 * (a' * d) := Nat.mul_assoc _ _ _
      have heq : a * d' = a' * d := by omega
      have := roundMag_scale ha hd hd' (c := d')
      rw [heq, Nat.mul_comm d d', roundMag_scale ha' hd' hd, h0, h'] at this
      simp only [Option.some.injEq, Prod.mk.injEq] at this
      obtain ⟨rfl, rfl⟩ := this
      omega

/-! ## Exactness -/

/-- **A representable value is its own rounding.** -/
theorem round_exact {a d : Nat} (ha : 0 < a) (hd : 0 < d) {y : F64} (hy : y.isFinite = true)
    (h : a = y.mag * d) : ∃ m q, roundMag a d = some (m, q) ∧ m * 2 ^ q = y.mag := by
  cases h0 : roundMag a d with
  | none =>
    exfalso
    have t := (roundMag_eq_none_iff ha hd).mp h0
    have hy' : y.mag ≤ (2 ^ 54 - 2) * 2 ^ 2044 := by
      have := mag_le hy
      rw [show (2 : Nat) ^ 2045 = 2 ^ 2044 * 2 by rw [← Nat.pow_succ], Nat.mul_comm (2 ^ 2044) 2,
        ← Nat.mul_assoc] at this
      exact Nat.le_trans this (Nat.le_of_eq (by congr 1))
    have := Nat.mul_le_mul_right d hy'
    have e : (2 ^ 54 - 2) * 2 ^ 2044 * d = (2 ^ 54 - 2) * (d * 2 ^ 2044) := by
      rw [Nat.mul_assoc, Nat.mul_comm (2 ^ 2044) d]
    have hX : 0 < d * 2 ^ 2044 := Nat.mul_pos hd (Nat.two_pow_pos _)
    rw [e] at this
    generalize d * 2 ^ 2044 = X at *
    omega
  | some mq =>
    obtain ⟨m, q⟩ := mq
    refine ⟨m, q, rfl, ?_⟩
    have n := (roundMag_nearest ha hd h0 y).1
    rw [← h] at n
    unfold dist at n
    exact Nat.eq_of_mul_eq_mul_right hd (by omega)

/-- Every `m * 2^p` with `0 < m ≤ 2^53` and `p ≤ 2044` is a double. -/
theorem exists_valid : ∀ (p m : Nat), 0 < m → m ≤ 2 ^ 53 → p ≤ 2044 →
    ∃ m' q, Valid m' q ∧ m' * 2 ^ q = m * 2 ^ p := by
  intro p
  induction p with
  | zero =>
    intro m hm hle _
    by_cases h : m = 2 ^ 53
    · exact ⟨2 ^ 52, 1, ⟨by decide, .inl (Nat.le_refl _), by decide⟩, by subst h; rfl⟩
    · exact ⟨m, 0, ⟨by omega, .inr rfl, by decide⟩, rfl⟩
  | succ p ih =>
    intro m hm hle hp
    by_cases h1 : m < 2 ^ 52
    · obtain ⟨m', q, hv, he⟩ := ih (2 * m) (by omega) (by omega) (by omega)
      refine ⟨m', q, hv, ?_⟩
      rw [he, Nat.pow_succ, Nat.mul_comm (2 ^ p) 2, Nat.mul_comm 2 m, Nat.mul_assoc]
    · by_cases h : m = 2 ^ 53
      · refine ⟨2 ^ 52, p + 2, ⟨by decide, .inl (Nat.le_refl _), by omega⟩, ?_⟩
        subst h
        rw [← Nat.pow_add, ← Nat.pow_add, show 52 + (p + 2) = 53 + (p + 1) by omega]
      · exact ⟨m, p + 1, ⟨by omega, .inl (by omega), by omega⟩, rfl⟩

/-- **Small integers are exact.** Every natural number up to `2^53` is a
double, and `ofNat` gives it. -/
theorem ofNat_spec {n : Nat} (hn : n ≤ 2 ^ 53) :
    (ofNat n).isFinite = true ∧ (ofNat n).sign = false ∧ (ofNat n).mag = n * 2 ^ 1074 := by
  by_cases h0 : n = 0
  · subst h0
    simp only [ofNat, ↓reduceIte]
    exact ⟨make_isFinite zero_valid, make_sign zero_valid, by rw [make_mag zero_valid, Nat.zero_mul, Nat.zero_mul]⟩
  · obtain ⟨m', q', hv, he⟩ := exists_valid 1074 n (by omega) hn (by omega)
    have hy := make_isFinite (s := false) hv
    have ha : 0 < n * 2 ^ 1074 := Nat.mul_pos (by omega) (Nat.two_pow_pos _)
    obtain ⟨m, q, hr, hmq⟩ := round_exact ha (Nat.zero_lt_one) hy
      (by rw [make_mag hv, he, Nat.mul_one])
    obtain ⟨hround, hv2⟩ := round_of_some (s := false) Nat.zero_lt_one ha hr
    simp only [ofNat, h0, ↓reduceIte, hround]
    refine ⟨make_isFinite hv2, make_sign hv2, ?_⟩
    rw [make_mag hv2, hmq, make_mag hv, he]

theorem ofNat_scaled {n : Nat} (hn : n ≤ 2 ^ 53) :
    (ofNat n).scaled = ((n * 2 ^ 1074 : Nat) : Int) := by
  obtain ⟨-, hs, hm⟩ := ofNat_spec hn
  simp [scaled, hs, hm]


/-! ## Signs -/

/-- `±a` as an integer. -/
def signed (s : Bool) (a : Nat) : Int := if s then -(a : Int) else a

theorem neg_word (x : F64) :
    (neg x).word = if x.word < 2 ^ 63 then x.word + 2 ^ 63 else x.word - 2 ^ 63 := by
  have := word_lt x
  simp only [neg, word]
  rw [UInt64.toNat_ofNat']
  apply Nat.mod_eq_of_lt
  simp only [word] at this
  by_cases h : x.bits.toNat < 2 ^ 63 <;> simp only [h, ↓reduceIte] <;> omega

theorem neg_sign (x : F64) : (neg x).sign = !x.sign := by
  have := word_lt x
  simp only [sign, neg_word]
  rw [← decide_not]
  apply decide_eq_decide.mpr
  simp only [word] at this ⊢
  by_cases h : x.bits.toNat < 2 ^ 63 <;> simp only [h, ↓reduceIte] <;> omega

theorem neg_bexp (x : F64) : (neg x).bexp = x.bexp := by
  have := word_lt x
  simp only [bexp, neg_word]
  by_cases h : x.word < 2 ^ 63 <;> simp only [h, ↓reduceIte] <;> omega

theorem neg_frac (x : F64) : (neg x).frac = x.frac := by
  have := word_lt x
  simp only [frac, neg_word]
  by_cases h : x.word < 2 ^ 63 <;> simp only [h, ↓reduceIte] <;> omega

theorem neg_isNaN (x : F64) : (neg x).isNaN = x.isNaN := by simp only [isNaN, neg_bexp, neg_frac]
theorem neg_isInf (x : F64) : (neg x).isInf = x.isInf := by simp only [isInf, neg_bexp, neg_frac]
theorem neg_isFinite (x : F64) : (neg x).isFinite = x.isFinite := by simp only [isFinite, neg_bexp]
theorem neg_mag (x : F64) : (neg x).mag = x.mag := by
  simp only [mag, sig, uexp, neg_bexp, neg_frac]
theorem neg_scaled (x : F64) : (neg x).scaled = -x.scaled := by
  simp only [scaled, neg_sign, neg_mag]
  cases x.sign <;> simp

theorem make_zero_scaled (s : Bool) : (make s 0 0).scaled = 0 := by
  rw [make_scaled zero_valid]; cases s <;> rfl

/-! ## Correct rounding, signed -/

theorem round_finite {s a d} (hf : (round s a d).isFinite = true) :
    ∃ m q, roundMag a d = some (m, q) := by
  cases h : roundMag a d with
  | none =>
    rw [round_of_none h] at hf
    have := inf_isInf s
    simp only [isInf, isFinite, Bool.and_eq_true, beq_iff_eq, bne_iff_ne, ne_eq] at this hf
    exact absurd this.1 hf
  | some mq => exact ⟨mq.1, mq.2, rfl⟩

/-- **Round to nearest.** When `±a / (d * 2^1074)` rounds to a finite
double, no finite double is nearer. -/
theorem round_nearest {s a d} (ha : 0 < a) (hd : 0 < d) (hf : (round s a d).isFinite = true)
    {y : F64} (_hy : y.isFinite = true) :
    (signed s a - (round s a d).scaled * d).natAbs ≤ (signed s a - y.scaled * d).natAbs := by
  obtain ⟨m, q, h⟩ := round_finite hf
  obtain ⟨hr, hv⟩ := round_of_some (s := s) hd ha h
  have n := (roundMag_nearest ha hd h y).1
  have n0 := (roundMag_nearest ha hd h (make false 0 0)).1
  simp only [make_mag zero_valid, Nat.zero_mul] at n0
  rw [hr, make_scaled hv]
  simp only [scaled, signed]
  have e1 : ((m * 2 ^ q : Nat) : Int) * (d : Int) = ((m * 2 ^ q * d : Nat) : Int) :=
    (Int.natCast_mul _ _).symm
  have e2 : ((y.mag : Nat) : Int) * (d : Int) = ((y.mag * d : Nat) : Int) :=
    (Int.natCast_mul _ _).symm
  generalize m * 2 ^ q = M at *
  unfold dist at n n0
  cases s <;> cases y.sign <;> simp only [Int.neg_mul, e1, e2, Bool.false_eq_true, ↓reduceIte] <;>
    omega

/-- **Ties to even.** A finite double as near as the rounding, of another
value, makes the rounding's significand even. -/
theorem round_tie_even {s a d} (ha : 0 < a) (hd : 0 < d) (hf : (round s a d).isFinite = true)
    {y : F64} (_hy : y.isFinite = true) (hne : y.scaled ≠ (round s a d).scaled)
    (he : (signed s a - (round s a d).scaled * d).natAbs = (signed s a - y.scaled * d).natAbs) :
    (round s a d).sig % 2 = 0 := by
  obtain ⟨m, q, h⟩ := round_finite hf
  obtain ⟨hr, hv⟩ := round_of_some (s := s) hd ha h
  have n := roundMag_nearest ha hd h y
  have n0 := (roundMag_nearest ha hd h (make false 0 0)).1
  simp only [make_mag zero_valid, Nat.zero_mul] at n0
  have hsig : (make s m q).sig = m := by
    have := make_mag (s := s) hv
    simp only [mag] at this
    have hu : (make s m q).uexp = q := by
      simp only [uexp, make_bexp hv]; split
      · exact (hv.norm.resolve_left (by omega)).symm
      · omega
    rw [hu] at this
    exact Nat.eq_of_mul_eq_mul_right (Nat.two_pow_pos _) this
  rw [hr, hsig]
  rw [hr, make_scaled hv] at he hne
  simp only [scaled, signed] at he hne
  have e1 : ((m * 2 ^ q : Nat) : Int) * (d : Int) = ((m * 2 ^ q * d : Nat) : Int) :=
    (Int.natCast_mul _ _).symm
  have e2 : ((y.mag : Nat) : Int) * (d : Int) = ((y.mag * d : Nat) : Int) :=
    (Int.natCast_mul _ _).symm
  have hmd : 0 < y.mag → 0 < y.mag * d := fun h => Nat.mul_pos h hd
  have hmd0 : y.mag = 0 → y.mag * d = 0 := fun h => by rw [h, Nat.zero_mul]
  refine n.2 ?_ ?_
  all_goals
    generalize m * 2 ^ q = M at *
    simp only [dist] at n n0 ⊢
    cases hys : y.sign <;> cases s <;>
      simp only [hys, Int.neg_mul, e1, e2, Bool.false_eq_true, ↓reduceIte] at he hne <;> omega

/-! ## The operations -/

theorem add_finite {a b : F64} (ha : a.isFinite = true) (hb : b.isFinite = true) :
    a + b = if a.scaled + b.scaled = 0 then make (a.sign && b.sign) 0 0
      else round (decide (a.scaled + b.scaled < 0)) (a.scaled + b.scaled).natAbs 1 := by
  show add a b = _
  simp only [add, finite_not_nan ha, finite_not_nan hb, finite_not_inf ha, finite_not_inf hb,
    Bool.or_false, Bool.false_eq_true, ↓reduceIte]

/-- **`+` is correctly rounded.** -/
theorem add_nearest {a b : F64} (ha : a.isFinite = true) (hb : b.isFinite = true)
    (hr : (a + b).isFinite = true) {y : F64} (hy : y.isFinite = true) :
    (a.scaled + b.scaled - (a + b).scaled).natAbs ≤ (a.scaled + b.scaled - y.scaled).natAbs := by
  rw [add_finite ha hb] at hr ⊢
  split
  · rename_i h; rw [h, make_zero_scaled]; exact Nat.zero_le _
  · rename_i h
    simp only [h, ↓reduceIte] at hr
    have := round_nearest (Int.natAbs_pos.mpr h) Nat.zero_lt_one hr hy
    have es : signed (decide (a.scaled + b.scaled < 0)) (a.scaled + b.scaled).natAbs =
        a.scaled + b.scaled := by
      unfold signed; split <;> rename_i h' <;> simp at h' <;> omega
    rw [es] at this
    simpa using this

theorem sub_eq (a b : F64) : a - b = a + -b := rfl

/-- **`-` is correctly rounded.** -/
theorem sub_nearest {a b : F64} (ha : a.isFinite = true) (hb : b.isFinite = true)
    (hr : (a - b).isFinite = true) {y : F64} (hy : y.isFinite = true) :
    (a.scaled - b.scaled - (a - b).scaled).natAbs ≤ (a.scaled - b.scaled - y.scaled).natAbs := by
  have hb' : (-b).isFinite = true := by show (neg b).isFinite = true; rw [neg_isFinite]; exact hb
  have := add_nearest ha hb' hr hy
  rwa [show (-b).scaled = -b.scaled from neg_scaled b, ← Int.sub_eq_add_neg] at this

theorem scaled_eq (x : F64) : x.scaled = signed x.sign x.mag := rfl

/-- **`*` is correctly rounded**: `a.scaled * b.scaled / 2^1074` is the
exact product, scaled. -/
theorem mul_nearest {a b : F64} (ha : a.isFinite = true) (hb : b.isFinite = true)
    (hr : (a * b).isFinite = true) {y : F64} (hy : y.isFinite = true) :
    (a.scaled * b.scaled - (a * b).scaled * 2 ^ 1074).natAbs ≤
      (a.scaled * b.scaled - y.scaled * 2 ^ 1074).natAbs := by
  have hab : a.scaled * b.scaled = signed (a.sign != b.sign) (a.mag * b.mag) := by
    simp only [scaled, signed, Int.natCast_mul]
    cases a.sign <;> cases b.sign <;> simp [Int.neg_mul, Int.mul_neg]
  have hm : a * b = if a.mag = 0 ∨ b.mag = 0 then make (a.sign != b.sign) 0 0
      else round (a.sign != b.sign) (a.mag * b.mag) (2 ^ 1074) := by
    show mul a b = _
    simp only [mul, finite_not_nan ha, finite_not_nan hb, finite_not_inf ha, finite_not_inf hb,
      Bool.or_false, Bool.false_eq_true, ↓reduceIte]
    split <;> simp_all
  rw [hab]
  rw [hm] at hr ⊢
  split
  · rename_i h
    rw [make_zero_scaled, Int.zero_mul, Int.sub_zero]
    have : a.mag * b.mag = 0 := by rcases h with h | h <;> simp [h]
    simp [signed, this]
  · rename_i h
    simp only [h, ↓reduceIte] at hr
    have := round_nearest (Nat.mul_pos (by omega) (by omega)) (Nat.two_pow_pos 1074) hr hy
    simpa [Int.natCast_pow] using this

/-- **`/` is correctly rounded**: the exact quotient `|a| / |b|`, signed,
is `signed _ (a.mag * 2^1074) / (b.mag * 2^1074)`. -/
theorem div_nearest {a b : F64} (ha : a.isFinite = true) (hb : b.isFinite = true) (hb0 : b.mag ≠ 0)
    (hr : (a / b).isFinite = true) {y : F64} (hy : y.isFinite = true) :
    (signed (a.sign != b.sign) (a.mag * 2 ^ 1074) - (a / b).scaled * b.mag).natAbs ≤
      (signed (a.sign != b.sign) (a.mag * 2 ^ 1074) - y.scaled * b.mag).natAbs := by
  have hd : a / b = if a.mag = 0 then make (a.sign != b.sign) 0 0
      else round (a.sign != b.sign) (a.mag * 2 ^ 1074) b.mag := by
    show div a b = _
    simp only [div, finite_not_nan ha, finite_not_nan hb, finite_not_inf ha, finite_not_inf hb,
      Bool.or_false, Bool.false_eq_true, ↓reduceIte, hb0]
  rw [hd] at hr ⊢
  split
  · rename_i h
    rw [make_zero_scaled, Int.zero_mul, Int.sub_zero]
    simp [signed, h]
  · rename_i h
    simp only [h, ↓reduceIte] at hr
    exact round_nearest (Nat.mul_pos (by omega) (Nat.two_pow_pos _)) (by omega) hr hy

/-! ## Integer arithmetic -/

/-- **Integer addition is exact up to `2^53`.** -/
theorem add_ofNat {m n : Nat} (h : m + n ≤ 2 ^ 53) : ofNat m + ofNat n = ofNat (m + n) := by
  obtain ⟨fm, sm, -⟩ := ofNat_spec (n := m) (by omega)
  obtain ⟨fn, sn, -⟩ := ofNat_spec (n := n) (by omega)
  rw [add_finite fm fn, ofNat_scaled (by omega), ofNat_scaled (by omega), sm, sn]
  by_cases h0 : m + n = 0
  · have hm : m = 0 := by omega
    have hn : n = 0 := by omega
    subst hm hn; simp [ofNat]
  · have e : ((m * 2 ^ 1074 : Nat) : Int) + ((n * 2 ^ 1074 : Nat) : Int) =
        (((m + n) * 2 ^ 1074 : Nat) : Int) := by rw [Nat.add_mul, Int.natCast_add]
    have hp : 0 < (m + n) * 2 ^ 1074 := Nat.mul_pos (by omega) (Nat.two_pow_pos _)
    rw [e]
    simp only [ofNat, h0, ↓reduceIte, Int.natAbs_natCast]
    have h1 : (((m + n) * 2 ^ 1074 : Nat) : Int) ≠ 0 := by exact_mod_cast Nat.pos_iff_ne_zero.mp hp
    have h2 : decide ((((m + n) * 2 ^ 1074 : Nat) : Int) < 0) = false :=
      decide_eq_false (Int.not_lt.mpr (Int.natCast_nonneg _))
    simp only [h1, h2, ↓reduceIte]

/-- **Integer subtraction is exact up to `2^53`.** -/
theorem sub_ofNat {m n : Nat} (h : n ≤ m) (hm : m ≤ 2 ^ 53) : ofNat m - ofNat n = ofNat (m - n) := by
  obtain ⟨fm, sm, -⟩ := ofNat_spec (n := m) hm
  obtain ⟨fn, sn, -⟩ := ofNat_spec (n := n) (by omega)
  have fn' : (-ofNat n).isFinite = true := by
    show (neg _).isFinite = true; rw [neg_isFinite]; exact fn
  rw [sub_eq, add_finite fm fn', show (-ofNat n).scaled = -(ofNat n).scaled from neg_scaled _,
    ofNat_scaled hm, ofNat_scaled (by omega), sm]
  by_cases h0 : m - n = 0
  · have : m = n := by omega
    subst this
    simp only [Int.add_right_neg, ↓reduceIte,
      Bool.false_and, Nat.sub_self]
    rfl
  · have e : ((m * 2 ^ 1074 : Nat) : Int) + -((n * 2 ^ 1074 : Nat) : Int) =
        (((m - n) * 2 ^ 1074 : Nat) : Int) := by
      rw [Nat.sub_mul, Int.natCast_sub (Nat.mul_le_mul_right _ h), Int.sub_eq_add_neg]
    have hp : 0 < (m - n) * 2 ^ 1074 := Nat.mul_pos (by omega) (Nat.two_pow_pos _)
    rw [e]
    simp only [ofNat, h0, ↓reduceIte, Int.natAbs_natCast]
    have h1 : (((m - n) * 2 ^ 1074 : Nat) : Int) ≠ 0 := by exact_mod_cast Nat.pos_iff_ne_zero.mp hp
    have h2 : decide ((((m - n) * 2 ^ 1074 : Nat) : Int) < 0) = false :=
      decide_eq_false (Int.not_lt.mpr (Int.natCast_nonneg _))
    simp only [h1, h2, ↓reduceIte]

/-- **Past `2^53`, adding one does nothing**: `2^53 + 1` is a tie between
`2^53` and `2^53 + 2`, and `2^53`'s significand is the even one. -/
theorem add_one_saturates : ofNat (2 ^ 53) + ofNat 1 = ofNat (2 ^ 53) := by
  have : (ofNat (2 ^ 53) + ofNat 1).bits = (ofNat (2 ^ 53)).bits := by decide +kernel
  cases h1 : ofNat (2 ^ 53) + ofNat 1; cases h2 : ofNat (2 ^ 53)
  rw [h1, h2] at this; simp only at this; rw [this]

/-! ## Order -/

theorem le_iff {a b : F64} : a ≤ b ↔ a.isNaN = false ∧ b.isNaN = false ∧ a.key ≤ b.key := by
  show le a b = true ↔ _
  simp [le, and_assoc]

theorem lt_iff {a b : F64} : a < b ↔ a.isNaN = false ∧ b.isNaN = false ∧ a.key < b.key := by
  show lt a b = true ↔ _
  simp [lt, and_assoc]

theorem ofNat_key {n : Nat} (hn : n ≤ 2 ^ 53) : (ofNat n).key = ((n * 2 ^ 1074 : Nat) : Int) := by
  obtain ⟨hf, -, -⟩ := ofNat_spec hn
  simp only [key, finite_not_inf hf, Bool.false_eq_true, ↓reduceIte, ofNat_scaled hn]

theorem ofNat_isNaN {n : Nat} (hn : n ≤ 2 ^ 53) : (ofNat n).isNaN = false :=
  finite_not_nan (ofNat_spec hn).1

/-- **Integers up to `2^53` compare as integers.** -/
theorem ofNat_le_ofNat {m n : Nat} (hn : n ≤ 2 ^ 53) (hm : m ≤ 2 ^ 53) :
    ofNat m ≤ ofNat n ↔ m ≤ n := by
  rw [le_iff, ofNat_key hm, ofNat_key hn]
  simp only [ofNat_isNaN hm, ofNat_isNaN hn, true_and, Int.ofNat_le]
  exact Nat.mul_le_mul_right_iff (Nat.two_pow_pos _)

theorem ofNat_lt_ofNat {m n : Nat} (hn : n ≤ 2 ^ 53) (hm : m ≤ 2 ^ 53) :
    ofNat m < ofNat n ↔ m < n := by
  rw [lt_iff, ofNat_key hm, ofNat_key hn]
  simp only [ofNat_isNaN hm, ofNat_isNaN hn, true_and, Int.ofNat_lt]
  exact Nat.mul_lt_mul_right (Nat.two_pow_pos _)

end F64
end Contract
