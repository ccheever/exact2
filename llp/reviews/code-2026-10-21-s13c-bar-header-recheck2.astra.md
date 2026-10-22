# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), second narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e4f1f18553`.
- **Method:** one brief (sha256 `inline prompt (the narrow re-check's brief was given on the command line)`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P2s, a P3)
- **Disposition:** taken in bff4a046c4 (empty labelledby, the label's scope, ids in two arms, rustfmt on contract-lower)

VERDICT: **WRONG**

The specific derive, empty `aria-label`, descendant-label, and precedence cases are addressed. Remaining findings:

1. **P2 — Empty `aria-labelledby` suppresses fallback** — [library.rs:829](/tmp/iosfix-rv-bi4/contract/lower/src/anatomy/library.rs:829). With `aria-labelledby=""`, `.any(...)` returns false, so a state-dependent `aria-label` is ignored and a valid button is refused. Empty or whitespace-only references should fall through; labelledby precedence requires a valid IDREF. [Accessible-name computation](https://www.w3.org/TR/accname-1.2/#computation-steps).

2. **P2 — Referenced text uses the button’s scope** — [library.rs:829](/tmp/iosfix-rv-bi4/contract/lower/src/anatomy/library.rs:829). A state-dependent label outside `for selected in ["row"]` correctly names a button inside that loop whose opacity reads the component’s `selected` through `fade`. Passing the button’s `bound` hides the label’s component prop and incorrectly refuses it. Preserve the referenced text’s own bindings.

3. **P2 — Conditional labels overwrite one another** — [library.rs:655](/tmp/iosfix-rv-bi4/contract/lower/src/anatomy/library.rs:655). Both conditional arms populate one map. If the `then` arm contains a state-dependent label and its button, while `else` contains constant text with the same ID, the constant overwrites the correct label and the button is refused. These IDs never coexist at runtime; resolution must retain branch information or treat ambiguity as unresolved.

4. **P3 — Formatting still fails** — [library.rs:654](/tmp/iosfix-rv-bi4/contract/lower/src/anatomy/library.rs:654). `cargo fmt --all -- --check` exited **1** on this clean checkout, reporting differences here and elsewhere in this file.

No files changed. Integration tests were not run because their helper writes temporary fixtures.
