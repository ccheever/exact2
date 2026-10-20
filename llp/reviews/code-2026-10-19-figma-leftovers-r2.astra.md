# Code review: the Figma plugin's leftovers after its review cap, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `44cf014020`.
- **Method:** one brief (sha256 `503e55b9fb06228c10901a7201c098c2e90bc56289bc3b642cae7329ae82374c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** None found. Undo clears `dirty` before constructing the refusal; no new double-undo, null-access or automatic data-loss path found.

**Should-fix:**

- [code.ts:227](/tmp/rv-fp-2/tools/figma-tamber/src/code.ts:227): Recovery remains impossible when every remaining mode must go. The new `Personal A/B` test asserts “remove both,” but reordering still leaves an undeletable final default. Detect this case and give collection-recreation guidance with its losses explained. The replacement refusal at [code.ts:345](/tmp/rv-fp-2/tools/figma-tamber/src/code.ts:345) has the same limitation.
- [code.ts:186](/tmp/rv-fp-2/tools/figma-tamber/src/code.ts:186): `wanted` is correct at exactly the limit, but undercounts an already-over-limit collection. With limit 2, one surviving context and two personal modes, adding the second context reports one removal; two are necessary. Include `col.modes.length - capacity` in the deficit.
- [mapping.ts:381](/tmp/rv-fp-2/tools/figma-tamber/src/mapping.ts:381): The regex matches the server’s diagnostic, but the advice misidentifies its cause. That diagnostic compares a Tamber conditional’s Light value with its base, potentially through another token. A single Figma Light mode already propagates to its twins; split pushes already reconcile conflicting twins. “Set every Light mode the same” can therefore accomplish nothing—and unrelated axes may legitimately have different Light values. The string-only test does not establish recovery.

**Nits:**

- [code.test.ts:760](/tmp/rv-fp-2/tools/figma-tamber/src/code.test.ts:760): Round 1’s deleted-variable/deleted-collection coverage remains missing. The implementation’s guards look correct.
- [mapping.test.ts:3](/tmp/rv-fp-2/tools/figma-tamber/src/mapping.test.ts:3): Remove the stray `plainWhy` import from `bun:test`; it is already imported from `./mapping`.

Round 1’s parked-mode identification, restored names, default-mode hint and token display are fixed. The new assertions would catch the previous text, but the two-personal-mode assertion locks in impossible advice.

`bun test tools/figma-tamber`: **61 passed, 330 assertions**. No files changed.
