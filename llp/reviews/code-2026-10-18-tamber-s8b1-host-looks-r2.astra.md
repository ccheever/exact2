# Code review: `:active` as the press paint; looks on the runner and the web (LLP 1121 stage 8 B1, B1'), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `529bb8896..cc98d1d21` (B1, its round-1 fixes, B1' for D8 as amended).
- **Method:** one brief (sha256 `ae0b8a0389d166e8b1b6eae3eb30376b1e0f3050c4e2285fc918d7add0d95d60`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: the JS target's looks over the press paint, a disabled box still pressed; should-fix: authored transitions animating looks, a text with a look folding, the internal row in vocab's lookup).
- **Disposition:** in the round-2 commit, with tests. 1: the JS target's rules are unscoped, as wasm's (`modes_tests.rs` asserts it). 2: the glue's pointer gate refuses a box whose `disabled` is any value but "false" (Chrome test presses it). 4: a text with a look never folds (`host/web/tests/it/looks.rs`). 5: `contract vocab -exact-press-opacity-to` describes no attribute. 3: recorded as a web deviation with a QUEUE entry (a look rule cannot keep a node's own transition out of its edges; the fix belongs in the glue).

---

**RIGHT WITH CHANGES**

1. **BLOCKING — JS look rules override the press paint.** [host/web-js/src/emit.rs:770](/tmp/rv-s8b1-r2/host/web-js/src/emit.rs:770) retains `#exact-root`, giving hover/focus rules specificity `(1,4,0)`, above the shell’s important press rules `(1,2,0)`. A hovered opacity of `0.8` therefore hides an active target of `0.6`; backgrounds fail similarly. Remove that prefix, matching wasm’s selector strength. Test actual JS-generated CSS—the browser fixture uses the weaker wasm selector.

2. **BLOCKING — Disabled boxes still show active paint.** [host/web/input-glue.js:326](/tmp/rv-s8b1-r2/host/web/input-glue.js:326) checks `[disabled='true']`, while both targets emit `disabled=""`. The in-memory probe confirms that this starts `data-pressed`, `data-exact-painting`, and an animation. Use the look rules’ disabled predicate for pointer and keyboard feedback. Extend the disabled test to attempt a press; it currently checks only hover. This round-1 finding remains incompletely fixed.

3. **SHOULD-FIX — Authored transitions animate supposedly snapping looks.** [host/web/src/host/looks.rs:35](/tmp/rv-s8b1-r2/host/web/src/host/looks.rs:35) changes ordinary CSS properties without suppressing the node’s transitions. With `transition="opacity 1s"`, entering and leaving an opacity look animates on both targets. `!important` does not prevent transitions. Bypass authored transitions for look entry/exit while preserving transitions for base changes. Test both edges with an authored transition; asserting that generated rules contain no `transition` proves neither edge snaps.

4. **SHOULD-FIX — Text folding discards visible look styling.** [host/web/src/element.rs:51](/tmp/rv-s8b1-r2/host/web/src/element.rs:51) treats an empty base style as unstyled, ignoring the new looks table. A sole `text` child with only `:hover background-color` or `opacity` becomes `display:contents`, so its look cannot paint the intended box. Exclude nodes carrying looks from both `folded` and `holds_folded`, covering wasm and JS projection. Add a fixture without base styling or handlers.

5. **SHOULD-FIX — The internal opacity row remains exposed through vocabulary lookup.** [contract/lower/src/vocab.rs:202](/tmp/rv-s8b1-r2/contract/lower/src/vocab.rs:202) hides it only from enumeration. Single-name lookup at [contract/cli/src/vocab.rs:182](/tmp/rv-s8b1-r2/contract/cli/src/vocab.rs:182) still advertises `-exact-press-opacity-to` as a supported attribute through `tags::attr`. Apply the internal-name restriction to named text/JSON lookup too, directing authors to `:active opacity`.

Both digest pins match recomputation. The test-module moves preserve behavior. Cargo testing was blocked by sandbox writes to the requested target directory; browser tests were not run. No files changed.
