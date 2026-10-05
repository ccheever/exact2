# Code review: hsl()/hwb(), the vocab contextual test, the data-module bake catch (048bef229..22f472476), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `22f472476`.
- **Method:** one brief (sha256 `5a115b462db0e938b60ee6760c7cbc0732df8adbd03a752d8cd8225ca50a1a6f`), the same one sent to grok; round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** both fixed: `none` is refused rather than flattened to 0 (#1); an angle is a hue only, alpha and saturation/lightness refuse one, a percentage hue is refused, with rejection cases (#2).

---

LAND WITH FIXES

1. **P2 — `none` loses its meaning in animated colours.** [motion/src/hue.rs:55](/tmp/rv-r2/motion/src/hue.rs:55) immediately converts missing components to zero. For a linear animation from `hsl(0 100% 50% / none)` to `hsl(0 100% 50% / 50%)`, CSS interpolation borrows the other endpoint’s alpha; halfway should remain 50% opaque. This implementation instead interpolates from transparent, producing approximately 25% opacity. [motion/src/animation/parse.rs:344](/tmp/rv-r2/motion/src/animation/parse.rs:344) also serializes the flattened value into web keyframes, so the browser cannot recover the missing component. **Fix:** preserve missing-component information through interpolation and serialization, or explicitly reject `none` in animated colours until supported.

2. **P2 — Component parsing accepts invalid CSS, creating static/dynamic rendering differences.** [motion/src/hue.rs:67](/tmp/rv-r2/motion/src/hue.rs:67) converts angles into ordinary numbers, which saturation/lightness and alpha then accept at lines 23 and 37. Consequently, `hsl(0 100deg 50%)` and `hsl(0 100% 50% / 1deg)` become red instead of being rejected. The parser also accepts legacy `hsl(none, 100%, 50%)`. Although inherited from the canvas parser, these mistakes now affect ordinary styles and keyframes. Static JS styles pass through the kernel at [host/web-js/src/style.rs:152](/tmp/rv-r2/host/web-js/src/style.rs:152), while dynamic bindings pass strings to browser CSS at [host/web-js/rt.js:722](/tmp/rv-r2/host/web-js/rt.js:722), where invalid values disappear. **Fix:** distinguish angles, numbers, percentages and missing components; allow angles only for hue and `none` only in modern syntax. Add rejection cases.

No additional correctness findings in the contextual test or documentation. The shared hue parser does not widen existing motion/kernel RGB differences. Ordinary hue normalization, interior whitespace, units, byte rounding and HWB normalization match canvas; no concrete cross-host determinism issue emerged. The guide’s catch returned `[]` in an in-memory prelude check. Native bake failure and named scratch-store persistence are supported by the traced code. Cargo and browser runs were not performed under the read-only constraints.