# Code review: the source map's `layout` on a written row (LLP 1122 D14, T5), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `2b7265ddf78e70b9655cee0e8c97febd8c3a38191933b8f043d8d0245af8f86f`), shared by both reviewers, blind to the other review; commits 386762a69f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** nothing blocks (both). Smaller points: the 1.12 bullet's splice of `layout` into stage 9 E's clause was already moved beside T5 in `e3d912aff6`; the untested `resize`/`flex`/`font-size` cases go through the same `paint_row` call (not taken: the predicate is unchanged and shared with set-row's refusal).

**Nothing blocks commit `386762a69f`.** I found no correctness regression.

- [map.rs:270](/tmp/rv-dt-t5r1/contract/cli/src/map.rs:270): classification agrees with `set-row` for emitted bindings. Shorthands use all their rows: `border="1px solid #000000"` marks even its color bindings as layout because its width affects layout. `flex` likewise marks all three expansions.
- The empty value is safe with current routing. [tags.rs:384](/tmp/rv-dt-t5r1/contract/lower/src/tags.rs:384) distinguishes `resize="none"` from `resize=fit`; the latter emits a handler, so it never reaches this map branch. Props are also excluded. State origins bypass the new field through [sites.rs:109](/tmp/rv-dt-t5r1/contract/lower/src/sites.rs:109).
- JSON punctuation is correct, and state bindings cannot gain duplicate `at` fields. A read-only comparison confirmed that removing the added block restores the previous `map.rs` byte-for-byte.
- [lib.rs:20](/tmp/rv-dt-t5r1/contract/cli/src/lib.rs:20): `paint_row`’s body is byte-identical to the old implementation. Its re-export preserves the history caller’s behavior; `tamber_history` is unchanged.

Smaller point: [source_map.rs:517](/tmp/rv-dt-t5r1/contract/cli/tests/it/source_map.rs:517) covers the requested layout/paint distinction, and its helper parses the JSON. A CSS-versus-handler `resize` case would usefully guard the empty-value assumption.

Read-only consistency checks and `git diff --check` passed. Rust tests were not run: this checkout has no build artifacts, and building would violate the read-only constraint.
