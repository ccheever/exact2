# Code review: grouped lists read a row's symbol however it's given (stage 13 C fix 1, theme2 16:37), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b58bfdb013`.
- **Method:** one brief (sha256 `9d5bbd97f981b4fd08955060076626d22bd99d2dd2c92feee40d9d4a6db0c2d0`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken: each icon's and title's place in its row against the literal row's; an if whose else shows a text (the sheet's ink only: the kernel reads the row as it shows now) and an else if

VERDICT: RIGHT WITH NITS

1. **nit — [contract/cli/tests/it/grouped_action.rs:128](/tmp/iosfix-rv-gs2/contract/cli/tests/it/grouped_action.rs:128):** The geometry assertion compares only icon width and height. It does not catch incorrect icon placement, title inset, or symbol tint—the other behaviors affected by `symbol()`. Compare those with the literal row, including the inset after hiding the conditional icon.

2. **nit — [contract/cli/tests/it/grouped_action.rs:84](/tmp/iosfix-rv-gs2/contract/cli/tests/it/grouped_action.rs:84):** Every conditional fixture has an empty `else`. Add a case with a valid symbol in `then` and text or two symbols in `else`; an implementation accidentally ignoring `otherwise` would currently pass. A nested `else if` case would also pin the recursive behavior.

All round-1 fixes are present: the module is registered, files remain below the cap, assertions read kernel `action` and `symbol`, the conditional template exercises both states, invalid branches are rejected, and the pushing row’s **title** retains label ink.

No correctness blocker found across the callers. The placeholder never replaces the image source; the kernel reads its evaluated name. Changes to leading-symbol metrics, insets, subtitle recognition, and title-button recognition are consistent. A dynamic chevron/checkmark **before the title** remains a leading image in the kernel. Trailing templates still lack sheet accessory recognition; that limitation predates this commit.

`leading()` correctly handles empty branches, nested conditions, `else if`, and statically hidden nodes. It conservatively rejects `for` and visible text.

I found no affected existing app or example in this checkout. The conditional checkmarks in `scripts/fixtures/grouped-list.contract` are trailing and unchanged. Naming and comment density match the surrounding code.

Read-only review; no Cargo or tests run.
