# Code review: LLP 1121 stage 2, commit 6 (the web's app modes), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `f81b3d72b9b6e68eb22d3abff1483065cdfbdd8b464a155f75a1c334c669bb56`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: eight medium, one low; Grok: three high, one medium.
- **Disposition:** all fixed in the round-1 commit, except the accent-only boundary writing inherited rows, kept out to match the wasm host (and later refined)

---

No high-severity findings. I found eight medium-severity bugs and one low-severity mismatch.

1. **Medium — Cleared bindings block inherited token rows.** [modes.rs:187](/tmp/rv-s2c6r1/host/web-js/src/modes.rs:187), [modes.js:89](/tmp/rv-s2c6r1/host/web-js/modes.js:89)  
   `data-x-b` permanently marks a row as owned merely because its binding exists. When a boundary’s bound colour becomes `null`, it therefore never receives the ancestor’s implied token row. Separately, `Sx` treats `"inherit"`/`"unset"` as non-token declarations, stopping token provenance through intermediate ancestors. Both differ from the runner’s `themed.sets()` behavior. **Fix:** track whether each current value actually sets the row, and restore inherited token provenance and implied rows after clears or rejected values.

2. **Medium — Delayed adoption leaves colours frozen at rendered values.** [modes.js:46](/tmp/rv-s2c6r1/host/web-js/modes.js:46)  
   Cleanup runs once, against classes present during the first `apply()`. Rust-rendered list rows adopted in later slices receive their generated classes afterward, retaining inline literal colours that override the classes’ `var()` declarations. Subsequent `setMode` calls never clean them up. Static boundaries adopted later also receive no refresh until another commit. **Fix:** perform cleanup per adopted element and refresh modes after adoption slices. Test a pre-rendered list spanning multiple slices, followed by a mode change.

3. **Medium — JS-rendered pages omit pinned tuples and implied rows.** [modes.js:75](/tmp/rv-s2c6r1/host/web-js/modes.js:75)  
   The `__exactRender` guard suppresses all boundary processing. Consequently, `render.mjs` emits `data-x-pin` without `data-x-modes`, and omits inherited token rows on scheme boundaries. Non-default previews render using the default tuple until activation; interaction-activated pages can remain wrong indefinitely. **Fix:** perform tuple assignment and implied-row serialization during rendering, using the render DOM. Test the HTML before client activation.

4. **Medium — Re-specifying an accent can create a self-reference.** [modes.js:65](/tmp/rv-s2c6r1/host/web-js/modes.js:65)  
   An inherited accent token resolving to `AccentColor` produces `var(--exact-accent, AccentColor)`. This loop copies that text into both `accent-color` and `--exact-accent`, making the custom property reference itself and invalidating it. A nested scheme boundary then loses an ancestor’s authored tint. The wasm serializer explicitly avoids this cycle. **Fix:** preserve its accent-specific serialization rules, including varying tokens that resolve to the accent. Test under an explicitly tinted ancestor.

5. **Medium — Inherited SVG paint rows lose resolved IDs.** [modes.js:88](/tmp/rv-s2c6r1/host/web-js/modes.js:88)  
   `Sx(Sr, …)` records text before `Sr` resolves authored IDs. For a bound fill such as `url(#paint) ink`, the ancestor receives `url(#x42) var(--x-ink)`, but the boundary receives the original `url(#paint) var(--x-ink)`. The paint server disappears and its fallback colour paints instead. **Fix:** resolve references at the boundary through the same resolver used by `Sr`. Add a token-fallback paint-server case beneath a pin.

6. **Medium — Initial cleanup removes required SVG symbol styles.** [modes.js:48](/tmp/rv-s2c6r1/host/web-js/modes.js:48)  
   Cleanup also runs on fresh mounts and deletes inline declarations already containing `var()`. [emit.rs:1039](/tmp/rv-s2c6r1/host/web-js/src/emit.rs:1039) deliberately places symbol descendants’ styles inline because `<use>` clones need them. A varying token used for their fill or stroke loses that required declaration on the first refresh. **Fix:** distinguish rendered literal overrides from required inline styles; preserve or rewrite the latter. Test a themed `<symbol>` instantiated with `<use>`.

7. **Medium — Bound text-stroke shorthands bypass colour tracking.** [rows.rs:358](/tmp/rv-s2c6r1/host/web-js/src/rows.rs:358)  
   A bound `-webkit-text-stroke` emits one shorthand write, while channel matching recognizes only `-webkit-text-stroke-color`. It therefore uses plain `S`, leaving no colour channel for descendant boundaries. Their stroke colour stays inherited from the ancestor’s tuple. **Fix:** lower the shorthand into tracked longhands, or separately record its parsed colour component. Test a bound shorthand above a differently pinned subtree.

8. **Medium — Generated custom-property names can collide.** [modes.rs:76](/tmp/rv-s2c6r1/host/web-js/src/modes.rs:76), [modes.rs:204](/tmp/rv-s2c6r1/host/web-js/src/modes.rs:204)  
   A valid varying token named `i0` becomes `--x-i0`, which the colour-channel marker also writes as an integer, invalidating `color:var(--x-i0)`. Suffix allocation also checks only once: retained names `a-b-2`, `a.b`, `a-b`, in that order, produce duplicate `--x-a-b-2` properties. **Fix:** reserve the metadata namespace and retry allocation until unique. Add both collision cases.

9. **Low — Clearing a computed mode pin emits a false diagnostic.** [modes.js:54](/tmp/rv-s2c6r1/host/web-js/modes.js:54)  
   A computed `-exact-mode` value of `"unset"` correctly falls back to the surrounding tuple but logs an invalid-value warning. The runner recognizes this as a valid row clear and logs nothing. **Fix:** normalize clearing keywords before parsing pins; test the tuple and journal together.

The added tests miss these cases. Normal multi-axis propagation, bound-pin changes, and invalid-mode atomicity passed the in-memory probes; the module-loading gate matches the requested condition.

Review remained read-only. Validation used source tracing and in-memory JavaScript probes; Cargo and browser suites were not run.
