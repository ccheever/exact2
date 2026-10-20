# Code review: the web's rangeFrame caret on an empty line and in an empty field, narrow re-check 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `e9dcbde366`.
- **Method:** one brief (sha256 `cb8b03f07839f76692e6deb68af64d13f757c090e4ab0bd913c9c553e50da95a`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a web host read, the lead's 21:00Z task under theme2's 18:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX).
- **Disposition:** taken: the indentation case applies its white-space.

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — [decorations.test.mjs:218](host/web/tests/decorations.test.mjs:218): the indentation case never applies `white-space: normal`.** Its `if` changes the value but bypasses the `else` that applies the case’s CSS. It therefore runs with the textarea’s default `pre-wrap`; reverting the whitespace fix would leave this case unchanged. Apply the styles independently of the indentation branch, then confirm the previous implementation fails.

The implementation fixes look correct: `whiteSpaceCollapse = 'preserve'` retains spaces and breaks while leaving `text-wrap-mode` intact, matching [Blink’s editable-text adjustment](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/css/resolver/style_adjuster.cc). All physical and logical min/max constraints are neutralized.

The max-width case applies its styles and samples the centered second line, exposing the previous border-box cap.

Verified through source review and in-memory execution of the fixture setup with DOM stubs. Browser geometry tests were not run. HEAD and tree unchanged.
