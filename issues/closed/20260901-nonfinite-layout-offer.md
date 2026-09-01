# A non-finite layout offer publishes NaN frames

**Status:** Closed
**Resolution:** Nonfinite layout offers now return a typed refusal without publishing frames.
**Systems:** Kernel
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** `kernel/tests/review_fixes.rs` (style NaN was closed), LLP 0382

`Kernel::compute_layout` refuses a missing/non-root id but never refuses a non-finite `Offer`. `LayoutTree::compute` hands `AvailableSpace::Definite(NaN)` (or Inf) to Taffy and publishes the resulting `Frame` bits.

Style NaN was closed as finding 3 in `review_fixes.rs` (`NonFiniteStyle` / `DecodeError::NonFinite`) specifically so NaN never reached published frames. Intrinsic sizes and `Env` already fail closed. A host with a bad viewport — or `Offer::definite(f32::NAN, h)` — reopens that hole with no `LayoutError` and no test.

Fix: reject a non-finite (and, if you want CSS's used-value clamp, non-positive) definite offer as `LayoutError` before calling Taffy. Add a sibling of `non_finite_style_numbers_are_refused_on_both_ingress_paths` that asserts frames stay finite.
