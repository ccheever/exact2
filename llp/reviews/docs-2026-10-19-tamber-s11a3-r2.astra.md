# Docs review: LLP 1121 stage 11 A3, package designs as built, round 2 of 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at the commit under review.
- **Method:** one brief (sha256 `800e3625bf4b7046596b79cc0ca8a30ed13d0e2203bcdcef2ae31b68eafa9db5`), shared by both reviewers, blind to the other review; commit 141add0a04 (reviewed: 8d65da90a6). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (2 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in 141add0a04: the one-name rule as the app's over a package's, the nine base names and that a package may not declare them, where refusals point, the guide's file locations and component block and ids, the collision as an app's two packages, and that the tamber subcommands read the app's design alone (check with contract build).

No **BLOCKING** findings.

- **SHOULD-FIX — [llp/1121-tamber.rfc.md:834](llp/1121-tamber.rfc.md:834):** The retained paragraph still says an app definition resolves duplicate names from different files. That contradicts the new collision rule: two packages shipping designs under one namespace are refused before overrides apply. Rewrite this as the app-over-package override rule.

- **SHOULD-FIX — [docs/contract-for-agents.md:3554](docs/contract-for-agents.md:3554):** The tooling caveat needs a validation instruction. `tamber check`, `eval`, and `ir` also initially check the app design without package tokens ([tamber.rs:121](contract/cli/src/tamber.rs:121)). An otherwise valid app alias such as `brand = reading.card` therefore fails there; `gen` can fail too, rather than merely omit bindings. Tell authors to validate the combined design with `contract build`.

- **NIT — [llp/1121-tamber.rfc.md:857](llp/1121-tamber.rfc.md:857):** Narrow “A refusal points at the token’s line” to **these reference refusals**. Unknown-axis errors point at the `when` line; namespace collisions and unreadable files carry a file path with a default span.

The rest checks out against A1: file discovery and `"tamber"` handling, transitive/vendored packages, manifest-derived namespace and header, collision only when both ship designs, token/source identity, refused declarations, whole-token replacement, and reference restrictions—including `pair=` and direct dial reads. The server identity is explicitly attributed to A2. The guide’s examples are concrete and avoid implementation jargon.

Read-only review; tests inspected, not run.
