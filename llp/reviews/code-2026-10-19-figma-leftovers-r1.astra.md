# Code review: the Figma plugin's leftovers after its review cap, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ddf24a4cc8`.
- **Method:** one brief (sha256 `d60ef9cfbdd6b3f47584aa1b8bdc2a4e821ec1cf3f94fce0c64d71a90bc69939`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

- **Blocking:** None found. The new `undo()` clears `dirty`, preventing a second undo in the outer catch. Missing collection/mode IDs are guarded; no new automatic data-loss path found.

- **Should-fix — [code.ts:226](/tmp/rv-fp-1/tools/figma-tamber/src/code.ts:226):** The current default is not necessarily parked. Park Light, drag surviving Dark above it, then add a context at the two-mode limit: the refusal incorrectly names active Dark as “Tamber keeps unused” and recommends deleting the collection. Carry the obstructing parked-mode IDs into the error and resolve those after undo.

- **Nits — [code.test.ts:760](/tmp/rv-fp-1/tools/figma-tamber/src/code.test.ts:760):** The drops test covers split variables, renaming and type changes, but not a renamed collection or an already-deleted variable/collection. Add those cases to verify live names and omission.

`bun test tools/figma-tamber`: **59 passed, 322 assertions**. The new tests would fail against the parent implementation. No files changed.
