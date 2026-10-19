# Code review: the source map's `layout` on a written row (LLP 1122 D14, T5), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `922498a9a9`.
- **Method:** one brief (sha256 `6dc4b92760c760ece5efca8dfa733ff4e0a6c401683e9346b09482084241507d`), shared by both reviewers, blind to the other review; commits 386762a69f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** nothing blocks (both). Three rounds, as theme2 ruled for the wire field.

**Nothing blocks `386762a69f` at tip `922498a9a9`, including the consumer changes in `bc485dd65f`.** No new correctness findings.

- Reviewed both commits with `git show`. The producer remains unchanged: [map.rs:270](/tmp/rv-dt-t5r3/contract/cli/src/map.rs:270) classifies the whole written attribute consistently with `set-row`, including shorthand expansions.
- [tamber-inspect.mjs:237](/tmp/rv-dt-t5r3/scripts/tamber-inspect.mjs:237) refuses template/style edits for layout bindings, preserves whole-value token and parameter edits, and carries `layout: true` into the selection.

In-memory consumer assertions and `git diff --check` passed. Rust and file-writing test suites were not run under the read-only constraint. Nothing changed; the working tree remains clean.
