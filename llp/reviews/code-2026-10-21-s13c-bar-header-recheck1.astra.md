# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), narrow re-check after round 2, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `db94c10a4d`.
- **Method:** one brief (sha256 `inline prompt (the narrow re-check's brief was given on the command line)`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P2s, a P3)
- **Disposition:** taken in e4f1f18553 (derive stack, label precedence, labelledby resolution)

**VERDICT: WRONG**

1. **P2 — Derive detection depends on operand order.** [contract/lower/src/anatomy/library.rs:676](/tmp/iosfix-rv-bi3/contract/lower/src/anatomy/library.rs:676)  
   With `derive lit = selected`, `derive fade = (lit ? 1 : 0.5)`, and `opacity=([lit].length * fade)`, visiting `lit` outside a conditional marks it seen. Traversing `fade` then skips expanding `lit` inside its conditional, so `selected` never reaches the check. Reversing the multiplication detects it. The cycle guard must distinguish these contexts or track the active recursion stack.

2. **P2 — The new label guard rejects correctly named buttons.** [contract/lower/src/anatomy/library.rs:789](/tmp/iosfix-rv-bi3/contract/lower/src/anatomy/library.rs:789)  
   `aria-label=""` with descendant `text (selected ? "On" : "Off")` now fails, although the empty label falls back to that text. Likewise, `aria-labelledby="state-label"` referencing that descendant’s ID fails despite its accessible name expressing the state. Attribute presence alone cannot determine whether descendant text names the button. Both are regressions.

3. **P2 — An overridden label still satisfies the state check.** [contract/lower/src/anatomy/library.rs:790](/tmp/iosfix-rv-bi3/contract/lower/src/anatomy/library.rs:790)  
   `aria-labelledby` referencing constant `"Home"` plus `aria-label=(selected ? "On" : "Off")` passes through `says("aria-label")`, although the referenced name wins. This leaves the round-2 accessible-name issue incompletely addressed.

4. **P3 — Changed lines fail formatting.** [contract/lower/src/anatomy/library.rs:669](/tmp/iosfix-rv-bi3/contract/lower/src/anatomy/library.rs:669)  
   The pinned rustfmt’s read-only check exits 1, including differences in the new function and conditionals. The required formatting check remains blocked.

The specific direct-derive, shadowing, text-binding, literal-hidden-text, and both-arm fixes otherwise look correct. The native iOS selected-trait change and humans documentation also look correct.

The unchanged library-gate fixture compiled successfully using an existing compiler from a checkout with matching rule and package sources. Added tests and UIKit tests were not executed; their setup requires writes. No files were changed.
