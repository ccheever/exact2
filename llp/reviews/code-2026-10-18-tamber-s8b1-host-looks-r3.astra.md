# Code review: `:active` as the press paint; looks on the runner and the web (LLP 1121 stage 8 B1, B1'), round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** B1's commits through `674f09af9` (`cd2778f28` and `674f09af9` new since round 2).
- **Method:** one brief (sha256 `90c0215b56e19912ad396b85ebb46f70f8235c03820b1db48219b1a61215e153`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: inspect losing an `:active` row's place and showing the internal row, a disabled box pressed by key; should-fix: `uses` locations breaking `outline`, the JS target's CSSOM writes unseen, the looks sheet rebuilt per node; nit: the guide's snap claim).
- **Disposition:** post-round fixes, with tests. 1: `tamber-inspect.mjs` `join` and `agent-inspect.mjs` carry `state` and `at` (and the part's file counts for freshness), and the inspector names the row by its part (`:active opacity`); the edit is `set-state`'s unless a token (`tamber-inspect.test.mjs`, `agent-bare.test.mjs`). 2: the key path refuses a disabled box (Chrome test). 3: `sitesAt` matches a binding's `at` (test). 4: `modes.js` calls the glue's `repaintPress` after its CSSOM writes (Chrome test through the hook). 5: the sheet is written once per batch, in a microtask. 6: the guide names the web's transition deviation; the unit assertion speaks of the emitted rules.

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Inspect still loses `:active` provenance and exposes the internal row.** [scripts/tamber-inspect.mjs:138](/tmp/rv-s8b1-r3/scripts/tamber-inspect.mjs:138) retains only `binding.origin`, discarding `state` and `at`; [scripts/agent-inspect.mjs:111](/tmp/rv-s8b1-r3/scripts/agent-inspect.mjs:111) does likewise. The design tool consequently displays `press-opacity-to` as “set here” at the node’s location. An in-memory probe reproduced this. Preserve row-level state/location through both joins and render the authored spelling, such as `:active opacity`, in [inspector.ts:66](/tmp/rv-s8b1-r3/apps/design-tool/inspector.ts:66). Include referenced binding files in freshness checks.

2. **BLOCKING — Disabled boxes still receive keyboard press paint.** [host/web/input-glue.js:176](/tmp/rv-s8b1-r3/host/web/input-glue.js:176) reaches `keyed(el)` without the disabled predicate added to pointer handling. A focused `div disabled=""` with a press handler and active opacity still gains `data-pressed`, `data-exact-painting`, and an animation on Space; the glue probe confirmed all three. Apply the same disabled-ancestor guard before keyboard activation and feedback. Add a focused-disabled-box keyboard case.

3. **SHOULD-FIX — The new `uses` locations break outlining.** [tamber_uses.rs:397](/tmp/rv-s8b1-r3/contract/cli/src/tamber_uses.rs:397) replaces node coordinates with the active row’s coordinates, but [scripts/tamber-inspect.mjs:155](/tmp/rv-s8b1-r3/scripts/tamber-inspect.mjs:155) matches only node coordinates. The design tool forwards these locations directly to `outline`; a probe returned `[]` for the active row and `[0]` for its node. Extend matching to binding `at` locations, deduplicating sites, and test `uses → outline`, including shared class rows.

4. **SHOULD-FIX — The observer misses actual JS-target live edits.** [host/web/input-glue.js:280](/tmp/rv-s8b1-r3/host/web/input-glue.js:280) observes DOM mutations, while [host/web-js/modes.js:216](/tmp/rv-s8b1-r3/host/web-js/modes.js:216) updates tokens through `CSSStyleRule.style.setProperty()`. Those CSSOM changes produce no DOM mutation records, so the cached base remains stale during press/release easing. Ancestor boundary changes also escape the current subscriptions. Invoke a paint-refresh hook after live-token and boundary updates. The new browser test changes `textContent`, so it does not exercise this production path.

5. **SHOULD-FIX — Wasm look updates rebuild the entire stylesheet once per node.** [host/web/glue.js:615](/tmp/rv-s8b1-r3/host/web/glue.js:615) joins every stored rule and assigns `textContent` for each `looks` operation. Booting or retheming N matching nodes therefore performs quadratic work. Running the actual switch body for 1,000 nodes produced 1,000 assignments totaling approximately 71 million characters for a final 142,000-character sheet, before browser parsing. Update the rule map throughout the batch and flush the stylesheet once.

6. **NIT — Author documentation still promises snapping without qualification.** [docs/contract-for-agents.md:3106](/tmp/rv-s8b1-r3/docs/contract-for-agents.md:3106) says looks are “shown at once.” The authored-transition deviation is acceptable as a bounded deferral recorded in RFC §10 and [QUEUE.md:24](/tmp/rv-s8b1-r3/QUEUE.md:24), but this guide should name it. The absence-of-`transition` assertion should describe emitted CSS, not claim that transitions cannot occur.

Round 2: Astra 1/4/5 and Grok 1/2/4 are fixed. Astra 2 and Grok 3 remain partially fixed, as above. Astra 3 is explicitly deferred.

Both digest pins match recomputation; the test moves preserve behavior. Six modes tests and the inspect editability test passed. Cargo and one filesystem-dependent modes test were sandbox-blocked; browser tests were not run. No files changed.
