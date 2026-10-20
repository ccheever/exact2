# Docs review: LLP 1121 stage 11 A3, package designs as built, round 1 of 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at the commit under review.
- **Method:** one brief (sha256 `db2a9c1ea1ffaccc736dcc6cc1cb8e7fa45dadce03bc21097ad5edc345896892`), shared by both reviewers, blind to the other review; commit 8d65da90a6 (first: 38e31ec3c8). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (4 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in 8d65da90a6: identity described as built (a token keeps its name; its source is <package>/<file>; the server's <package>/<file>#<token> is A2's), the collision vs the override rule, the guide's restrictions (theme only, declared by this file, pair and dials, base names the app defines), the shared last segment, the entry and "tamber" field, tamber gen vs contract types, index.contract and the outside/missing path, icons QUEUE.

No **BLOCKING** findings. These documentation fixes remain:

- **SHOULD-FIX — [llp/1121-tamber.rfc.md:847](llp/1121-tamber.rfc.md:847):** The claimed `<package>/<file>#<token>` identity is not implemented in A1. `DesignSource.home` records `<package>/<file>`; tokens remain keyed by `reading.measure`. Describe that source provenance separately, or mark the combined identity as planned.

- **SHOULD-FIX — [llp/1121-tamber.rfc.md:846](llp/1121-tamber.rfc.md:846):** The new collision rule is correct, but contradicts the preceding “unless the app’s design sets it” at lines 834–835. Two packages sharing a namespace and both shipping designs are rejected **before** overrides. An app override cannot rescue that collision.

- **SHOULD-FIX — [docs/contract-for-agents.md:3546](docs/contract-for-agents.md:3546):** Complete the author-facing restrictions: only `theme` content is supported; “its own” means tokens **declared by the package**, not app-added names such as `reading.extra`; the restriction includes `pair=` and direct dial reads. Also, A1 supplies no base-scale values: `space.md` requires an app definition. The tests explicitly exercise these distinctions.

- **SHOULD-FIX — [docs/contract-for-agents.md:3549](docs/contract-for-agents.md:3549):** Add the generated-bindings limitation before the following paragraph: `tamber gen` still omits package-only tokens because it folds the app’s design alone (`tamber_gen.rs:554`). `contract types` includes retained package tokens.

- **NIT — [docs/contract-for-agents.md:3541](docs/contract-for-agents.md:3541), [llp/1121-tamber.rfc.md:841](llp/1121-tamber.rfc.md:841):** Mention the package’s `"tamber"` override in the guide and the `index.contract` fallback in D11. The guide uses concrete examples and little jargon, but splitting its long sentences and replacing “sets any” / “only its own” would improve clarity.

Checks out: transitive and vendored discovery, package-declared names despite installation aliases, header/prefix enforcement, collisions only when both ship designs, the refused-block list, whole-token overrides including `when` lines, reference and pair checks, direct reads through `Checked.reads`, and package-file diagnostic locations.

Read-only review against A1 and its tests; no tests executed. Whitespace check passed; working tree remains clean.
