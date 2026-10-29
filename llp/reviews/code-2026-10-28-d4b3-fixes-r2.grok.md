# Code review: the Inspector's numbers and the gallery's slot sample (D4 batch 3 #1, #2), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b178ff50fe`.
- **Method:** one brief (sha256 `9ba63c8149393a4e142161212d755cc00381b2452866987a0cfd7f9caa550cd5`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

Not clean. The round-1 number fix holds for the three cases it names, and the gallery slot sample matches the ruling. The length-name pattern is wrong for `line-height` and misses several pixel rows.

1. **P2** `apps/design-tool/inspector.ts:46` — `line-height` is capped as a length. The pattern's `height` alternative matches the `-height` segment, so a unitless `LineHeight::Number` (the bare number the inspector shows; a length line-height is already a `"…px"` string) goes through `toFixed(3)` at line 55. `Math.fround(4/3)` is `1.3333333730697632`, whose shortest round-trip is `1.3333334`, and the cap shows `1.333`, which does not round-trip. Commit `58cfb9ce38` had put `line-height` in the unitless set this pattern replaced. `font-size-adjust` is not a style row. `-exact-symbol-value` does not match (it is a 0–1 number, and it should not be capped). `-exact-control-size` matches `size`, but that row is an enum string, so the cap never runs. `font-size` is a real length and is capped correctly.

2. **P2** `apps/design-tool/inspector.ts:46` — pixel rows whose names are not in the pattern lose the 3-decimal cap. They still shorten to the shortest f32 round-trip, so `16.006250381469727` shows as `16.00625` rather than `16.006`. That is `perspective` and `translate-z` (schema: px), `translate` (the x/y px pair; `translate-percent` is a percentage and should stay uncapped), `baseline-shift` (a length, percentages refused), and the SVG point rows `cx`, `cy`, `r`, `x`, `y`, `rx`, `ry`. Before this commit those names were capped because the cap was the default.

3. **P3** `scripts/design-tool-shown-numbers.test.mjs:36` — the `1e-10` case is `letter-spacing`, which is a length, so `toFixed(3)` yields `0` even if the significant-digit search does nothing. The search itself is fine: a unitless `Math.fround(1e-10)` shows `1e-10`, and `flex-grow`'s `1e30` shows `1e+30`.

What holds: shortening runs only when `Math.fround(n) === n`, so a true double such as `0.123456789123` on `-exact-press-scale` stays; `fill-opacity`, `zoom`, `scale`, and `stroke-miterlimit` are not capped; an edit still sends `editText` (`inspector.ts:97`, `inspector.contract:168`), not the shown number. The slot sample is a `column` of at least 24×24 with `-exact-fill` (a real translucent system fill) and the text `slot` in `-exact-caption2` / `-exact-secondary-label`, indented as the component's child. `generate` compiles that source, and the test expects it to.
EXIT=0
