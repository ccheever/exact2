# Code review: LLP 1121 stage 11 A1, package designs in the compiler, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e60e4d1e1f`.
- **Method:** one brief (sha256 `dd2c7c05dad016ba3b31aeb823a571364aa7e7f54440caae4c62e58c9e01d723`), blind to the other reviewer; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 5 SHOULD-FIX).
- **Disposition:** taken: references judged from the one fold's resolution (no standalone fold), vendored manifests as inputs, `line N` only unbanded, a base name the app sets nameable.

Two blocking reference-isolation defects remain.

- **BLOCKING — [theme/packages.rs:198](contract/lower/src/theme/packages.rs:198): isolated checking does not preserve name resolution.** `reading.face: family = appFace` passes alone as a literal font-family name. After merging, an app declaration `appFace: family = serif` turns it into a forbidden app-token reference. The checked result is discarded and the original text resolves again globally.

- **BLOCKING — [theme/packages.rs:197](contract/lower/src/theme/packages.rs:197): profile stand-ins bypass reference checking.** `stand_in_profiles` clears all `when` clauses and metadata. A package token with a `color(--profile …)` default and a nondefault `when …: appBrand` therefore escapes isolation checking; the merged fold can resolve `appBrand` successfully.

- **SHOULD-FIX — [theme/packages.rs:186](contract/lower/src/theme/packages.rs:186): permitted shared names are missing.** `reading.gap = space.md` is refused even when the app defines `space.md`: neither `alone` nor `vocab()` supplies it. Likewise, `reading.ink = label` cannot resolve an app’s bare `label` role override, although `-exact-label` works. `shared()` now only formats an error message; it grants no reference exemption.

- **SHOULD-FIX — [sources/packages.rs:27](contract/cli/src/sources/packages.rs:27): vendored manifests are untracked inputs.** Discovery reads nested `package.json` files without adding them to `consulted`. With existing package designs, changing a vendored manifest’s name or `"tamber"` target can leave Cargo and the resident compiler using a stale plan.

- **SHOULD-FIX — [theme/packages.rs:205](contract/lower/src/theme/packages.rs:205): app-axis errors are attributed to package files.** `alone` contains cloned app axes, but every resulting diagnostic is assigned the package’s source ID. An invalid app axis produces spurious package-file diagnostics, preceding the correctly located app error.

- **SHOULD-FIX — [theme/packages.rs:234](contract/lower/src/theme/packages.rs:234): `unband()` rewrites arbitrary numbers.** It scans every digit sequence, including identifiers and values. An unknown token named `missing268435456` is reported as `missing0`, even in an app without packages. Restrict conversion to actual diagnostic line references.

- **SHOULD-FIX — [sources/packages.rs:111](contract/cli/src/sources/packages.rs:111): collision handling narrows the supplied ruling.** Yes: refusing only when both packages ship designs differs from “two linked packages with one last segment are refused.” The stated ruling does not contain that exception.

- **SHOULD-FIX, A2 — [tamber_serve.rs:735](contract/cli/src/tamber_serve.rs:735): server folding still excludes packages.** A valid app token such as `surface = reading.card` compiles through A1 but fails the server’s design fold. This is a functional compatibility gap, beyond merely omitting package tokens from the editor.

What checks out: band bounds and increments cover names, defaults, `when` headings and values without overlapping params or synthetic lines. I found no stale-`SOURCES` path in current call ordering: diagnostics become concrete spans before subsequent merges, and threads are isolated. Header/prefix checks, forbidden declarations, whole-token replacement, manifest-based names/home paths, containment, and explicit missing-file refusal look sound. Discovery is deterministic and includes transitive, vendored-relative and `exact.json` packages. Ordinary missing-design watching is safe. Motion, lint, warning and icon folds receive packages; app-only params handling remains appropriate.

There are **nine tests**, not seven. They cover basic merge and refusals, but the duplicate-location test fails during the isolated fold and never tests band mapping. Important gaps include the findings above, merged-fold default/`when` failures across multiple packages, repeated compiles, symlinks and named/missing designs, vendored/native-module discovery, and bindable retention.

Read-only review; no files changed. `git diff --check HEAD^ HEAD` passed. Builds/tests were not run.
