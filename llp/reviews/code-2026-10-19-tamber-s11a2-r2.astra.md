# Code review: LLP 1121 stage 11 A2 part 1, the server and contract tamber fold packages, round 2 of 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ebd1a39072`.
- **Method:** one brief (sha256 `615c1c3e84b25de4b784a038dcdfc458085419a910d4c970111b295864cdc526`), blind to the other review; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, 3 SHOULD-FIX).
- **Disposition:** taken in 6b7feeb93e: a write's planned app design folded through fold_with (an override removed that hid a package token naming the app's is refused, tested), an inline-theme app with packages told to use contract build, tamber gen for a packages-only app, the bands' limit.

One **BLOCKING** and three **SHOULD-FIX** findings:

- **BLOCKING — [tamber_ops.rs:127](contract/cli/src/tamber_ops.rs:127): edit plans still bypass package-reference validation.** Start with app tokens `appInk=#123456`, `reading.card=#445566`, `brand=reading.card`, and package token `reading.card=appInk`. The app override makes the initial build valid. Removing that override passes the joined generic fold and `write_checks`, so a transaction commits it, although the build and subsequent reads reject the package’s reference to `appInk`. Proposals and verified scrubs have the same gap. Validate the proposed app text through `fold_with`.

- **SHOULD-FIX — [tamber.rs:107](contract/cli/src/tamber.rs:107): the missing-file fallback discards inline themes.** With inline `label="#123456"` and package `reading.card = -exact-label`, eval/IR fold the platform role instead of the app override; admission succeeds against the actual app. Load the inline design or explicitly refuse this case.

- **SHOULD-FIX — [tamber_gen.rs:521](contract/cli/src/tamber_gen.rs:521): packages-only apps still cannot generate bindings.** `gen` exits with “has no design” before loading packages, whereas the build and the newly fixed check/eval/ir accept that app.

- **SHOULD-FIX — [tamber_params.rs:164](contract/cli/src/tamber_params.rs:164): over-limit refusals enter the param band.** The 257th package’s limit diagnostic at line 1 becomes `2^29 + 1`; `relocate` interprets it as the second param, or leaves it attributed to the app design. Preserve the package location without encoding an out-of-range band.

What checks out:

- `set-token` and `remove-token` on a package-only name are refused: splicing uses the parsed app text. `add-token` writes an app override; removing an existing override restores the package definition, subject to the validation gap above. Package files stay outside snapshots and digests.
- Ordinary package bands, `source_id` file lookup, message unbanding, and param-line separation are correct. Explain/uses receive joined folds; history compares parsed source. Export’s app-only limitation is explicitly queued.
- With an app design file, generation includes package names and applies existing collision checks. CLI admission uses the app’s build. Package designs reach both dev loops’ watchers through the source graph. No no-package regression found by inspection.

The six added tests cover the basic fixes but miss these cases and successful add/remove overrides. Tests were inspected, not rerun under the no-write constraint. No files changed.
