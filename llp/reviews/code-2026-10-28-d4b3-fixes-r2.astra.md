# Code review: the Inspector's numbers and the gallery's slot sample (D4 batch 3 #1, #2), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b178ff50fe`.
- **Method:** one brief (sha256 `9ba63c8149393a4e142161212d755cc00381b2452866987a0cfd7f9caa550cd5`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **P2 — Unitless `line-height` is still capped.** [inspector.ts:46](apps/design-tool/inspector.ts:46): numeric line-height rows are multipliers, but `height` matches `LENGTH`. Reproduced: `Math.fround(1.234567)` displays `1.235`, losing the required round-trip precision.

2. **P2 — Several actual lengths escape the cap.** [inspector.ts:46](apps/design-tool/inspector.ts:46): the pattern misses `translate`, `translate-z`, `perspective`, SVG `x/y/cx/cy/r/rx/ry`, and `baseline-shift`. These carry numeric px values. For example, a translate coordinate of `Math.fround(16.00625)` displays `16.00625`; the ruling requires `16.006`.

3. **P2 — Integer rows can display a different integer.** [inspector.ts:54](apps/design-tool/inspector.ts:54): equality with `Math.fround(n)` does not establish that a row stores an f32. `z-index` and `order` store i32 values. Reproduced: `shownValue('z-index', 100000008)` returns `"100000010"`. Integer-typed rows must bypass f32 shortening.

4. **P3 — The significant-digit search can miss the shortest representation.** [inspector.ts:54](apps/design-tool/inspector.ts:54): `shownNumber(2 ** -96)` returns `1.26217745e-29`, although the shorter `1.2621775e-29` round-trips to exactly the same f32. Checking only `toPrecision(p)` misses adjacent decimal candidates at asymmetric f32 rounding boundaries.

The existing Bun test passes, and edit text remains unchanged. No gallery finding; its test requires filesystem writes and was not run.
