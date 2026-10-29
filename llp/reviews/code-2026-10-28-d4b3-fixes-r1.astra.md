# Code review: the Inspector's numbers and the gallery's slot sample (D4 batch 3 #1, #2), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `826c199501`.
- **Method:** one brief (sha256 `2888e237002bb7f600140e3487263a0b464d4d844c72655b4e5139d63f6e3b43`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **P2 — Non-length rows lose precision** — [apps/design-tool/inspector.ts:45](apps/design-tool/inspector.ts:45). `UNITLESS` omits numeric rows the Inspector receives, including `scale`, `zoom`, SVG opacities, `stroke-miterlimit` and `-exact-symbol-value`. Reproduced: `fill_opacity=Math.fround(0.0001)` displays `0`; `zoom=Math.fround(1.0004)` displays `1`. Restrict the three-decimal cap to lengths and cover these rows in the test.

2. **P2 — True doubles do not round-trip** — [apps/design-tool/inspector.ts:52](apps/design-tool/inspector.ts:52). Live token reports return `Number(text)` without narrowing to f32 ([producer:138](host/web-js/inspect-dev.js:138)). Reproduced through that path: opacity `0.123456789123` displays `0.12345679`, and `0.999999999` displays `1`. The f32 comparison discards legitimate reported precision. Preserve double precision when the input is not a widened f32.

3. **P2 — Small and large floats retain widening artifacts** — [apps/design-tool/inspector.ts:50](apps/design-tool/inspector.ts:50). Nine decimal places cannot cover every f32 magnitude, and the integer shortcut bypasses large floats. Reproduced: `Math.fround(1e-10)` displays `1.000000013351432e-10`; `Math.fround(1e30)` displays `1.0000000150474662e+30`. Both have shorter round-tripping representations. Use a magnitude-independent search and test both cases.

Display text remains separate from edits. Negative numbers, ordinary integers, nested values and state-name mappings showed no additional regression; NaN/Infinity retain the previous JSON `null` behavior.

No gallery finding from source review: its spellings resolve through shared definitions. The sample has intrinsic/minimum width in a row and stretches across a default column. Both regression tests would reject the old code; the Bun test passes. Rust builds and visual drives were not run because they write files. No files changed.
