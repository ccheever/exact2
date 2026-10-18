# Code review: LLP 1121 stage 2, commit 6 (the web's app modes), round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `035e629ff29337f852949264ee1b91707ecd395343006d97457ad9eea07bc279`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five medium; Grok: one high.
- **Disposition:** Grok's high and Astra's 1-4 fixed in the round-3 commit, applied without re-review; Astra's 5 (no CSS.supports under render.mjs) named and left

---

Found **five medium-severity issues. No high- or low-severity findings.**

1. **Medium — Explicit clears lose their CSS effect outside boundaries.** [modes.js:130](/tmp/rv-s2c6r3/host/web-js/modes.js:130)  
   Returning `null` removes the declaration everywhere. A button with bound `color="inherit"` consequently takes the neutral stylesheet’s label colour instead of its parent’s colour. The wasm emitter preserves an explicit `color:unset`.  
   **Fix:** Preserve a canonical clear whenever no implied row supersedes it. Test cleared controls outside boundaries and boundaries without an inherited token.

2. **Medium — Bound and implied accents miss the new marker.** [rows.rs:369](/tmp/rv-s2c6r3/host/web-js/src/rows.rs:369), [modes.js:97](/tmp/rv-s2c6r3/host/web-js/modes.js:97)  
   Both paths suppress `--exact-accent` for accent-naming tokens, but neither sets `data-x-accent`; only literal rows receive it. With a token that is `AccentColor` in `ink=tint` and black in `ink=plain`, a bound or inherited accent on a plain boundary leaves accent consumers using the ancestor’s colour.  
   **Fix:** Maintain the marker from the effective accent row at runtime, including bindings, implied rows and clears. Add both cases to conformance.

3. **Medium — Adoption retains a stale accent companion.** [modes.js:63](/tmp/rv-s2c6r3/host/web-js/modes.js:63)  
   For a literal accent-naming token, the compiler removes the companion from the class, so `Vars` omits it. Rust-rendered HTML can nevertheless contain an inline `--exact-accent:#000000` from its initial tuple. Adoption preserves that declaration; switching to the tuple that should inherit `AccentColor` still shows black.  
   **Fix:** Include the suppressed companion in adoption cleanup. Test adopting a Rust-rendered boundary and then changing its pin.

4. **Medium — Changing between cleared accents still overwrites the implied companion.** [rows.rs:383](/tmp/rv-s2c6r3/host/web-js/src/rows.rs:383)  
   `--exact-accent` uses plain `S`, bypassing `Sx`’s clear normalization. After `unset` restores an inherited token, changing to `inherit` writes `--exact-accent:inherit`; the boundary cache then skips restoration. The probe retained `accent-color:var(--x-brand)` alongside the incorrect companion.  
   **Fix:** Normalize/cache companion clears consistently and restore both declarations together. Add an `unset → inherit` accent regression.

5. **Medium — Invalid values block implied rows during rendering.** [modes.js:123](/tmp/rv-s2c6r3/host/web-js/modes.js:123)  
   The render context has no `CSS.supports`, so an invalid bound colour is recorded as an owned row. A pinned boundary receives invalid inline CSS instead of its implied token row; the browser then inherits the parent’s colour rather than the pin’s colour until activation.  
   **Fix:** Supply render-compatible validity checking before recording ownership. Test invalid bound colours without global `CSS`.

The existing `modes.test.mjs` passed all 22 assertions; additional in-memory probes reproduced these DOM states. The other listed fixes looked sound, and inactive plans still avoid the modes import and boundary pass. No files changed; Rust builds and browser conformance were not rerun under the read-only constraint.
