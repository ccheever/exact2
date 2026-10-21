# Code review: the Figma plugin's main thread without structuredClone, quick review (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8e2429efd4`.
- **Method:** one brief (sha256 `f4326e81b42ed38e54fcb6d757917821f38586c21d11ddd56e869c4b2100177c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

- **Blocking:** None demonstrated. [The clone](/tmp/rv-fsc-1/tools/figma-tamber/src/mapping.ts:256) preserves plain colours, strings, booleans and finite numbers. [`valueOf()`](/tmp/rv-fsc-1/tools/figma-tamber/src/code.ts:44) excludes undefined/null, resolves aliases and rejects alias cycles; no Date/Map/object-cycle path appears. **Finiteness remains an unchecked assumption:** neither local validation nor Figma’s documented [`number` type](https://developers.figma.com/docs/plugins/api/VariableValue/) proves NaN/±Infinity impossible; JSON would convert them to null.

- **Should-fix:** [The guard](/tmp/rv-fsc-1/tools/figma-tamber/src/mapping.test.ts:162) misses `globalThis.x`, `globalThis["x"]`, `globalThis?.x?.()`, and destructuring/indirect access through `globalThis`. In-memory Bun probes confirmed those misses and confirmed that direct calls, `typeof x`, and `x?.()` **are caught**; shadowed locals and ordinary property keys are correctly ignored. Add qualified-access coverage and fixtures. Unlisted globals also escape the finite denylist.

- **Nits:** The test name overstates coverage. It checks listed identifiers surviving bundling, and also flags harmless `typeof` feature detection.

No other unavailable global was found in the reachable main-thread code. [`doorOf()` uses `URL`](/tmp/rv-fsc-1/tools/figma-tamber/src/mapping.ts:359), but it serves the UI and is removed from the main-thread bundle.

`bun test tools/figma-tamber`: **56 passed, 0 failed**. No files changed.
