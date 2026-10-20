# Code review: LLP 1121 stage 11 A2 part 1, the server and contract tamber fold packages, round 1 of 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `177b63e224`.
- **Method:** one brief (sha256 `a8b96ac484db2450a04101f6e243b272156af472ea0a9ee0ed05ca47541a67a2`), blind to the other review; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, 2 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in the amended commit: the server's plans fold beside the package tokens (tamber Params.joined), write_checks' role-axis fold joins them, fold_with refuses what the build refuses (pre-fold refusals, the reference rule), relocate unbands messages, tamber check folds through fold_with and reads a packages-only app; DTCG export of an app naming a package token queued.

- **BLOCKING — mutation folding still excludes packages.** [tamber_serve.rs:1184](contract/cli/src/tamber_serve.rs:1184) and [tamber_scrub.rs:90](contract/cli/src/tamber_scrub.rs:90) pass only params into `exact_tamber::plan`. Its edit validation and before/after folds never call the new `laid`. With `brand = reading.card`, editing an unrelated app token still fails. Conversely, an app override can pass validation while breaking a package expression; the later package-aware check runs only when params exist. Scrubs also omit changed package dependents. Transactions, proposals, and scrubs need package context throughout planning.

- **SHOULD-FIX — server reads bypass package admission rules.** [tamber_params.rs:110](contract/cli/src/tamber_params.rs:110) calls `with_package_designs`, which discards merge refusals; the subsequent fold also omits `package_references`. Consequently, package `reading.card = brand` resolves against app `brand = #123456`, although the build refuses that reference. Invalid package blocks can silently disappear. Preserve those refusals and apply the package reference checks.

- **SHOULD-FIX — CLI commands still reject designs supplied entirely by packages.** [tamber.rs:106](contract/cli/src/tamber.rs:106) synthesizes an empty app design only for params; [tamber_gen.rs:521](contract/cli/src/tamber_gen.rs:521) requires a physical design file. A valid package-only app therefore fails `check/eval/ir/gen` with “has no design,” despite A1 explicitly supporting and testing this configuration.

- **NIT — diagnostic messages retain synthetic line numbers.** [tamber_params.rs:72](contract/cli/src/tamber_params.rs:72) relocates the diagnostic’s location but does not apply the build’s `packages::unband` treatment to its message. Duplicate declarations can report `reading.tamber:6` alongside “declared twice (line 268435460).”

What checks out:

- Valid read folds preserve app override precedence and add packages before params. Empty-package behavior remains unchanged.
- Package bands stop before `params::LINE`; location mapping through `files[source_id]`, with `home` fallback, is correct.
- `parse_ops` receives a filename, not the laid AST. Editors parse original app text, so banded locations cannot become splice offsets. Package-only `set-token`/remove fail; `add-token` can create an app override. Snapshots and digests remain app-only.
- Explain/uses use the merged lookup. History remains text-based; export retains its intentional app-only fold.
- CLI admission uses the app’s build when packages exist. Generated bindings include package names and retain collision checks.
- Package designs, including missing-file candidates, reach the dev watchers through the source graph.

The two new tests cover read success and one relocated refusal, but miss mutations/scrubs, package admission, package-only apps, multiple bands with params, and generated-name collisions.

Tests were not executed because they write files. `git diff --check HEAD^ HEAD` found no whitespace errors. No files were modified.
