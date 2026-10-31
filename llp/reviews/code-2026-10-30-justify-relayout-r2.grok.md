# Code review: web hosts follow a block parent's justify-items (progress), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `/tmp/dt3-rv-c2`.
- **Method:** one brief (sha256 `232574aede38b3e586834196cc8a7731bacfe2f30072d587b4d339b1d2cd39a3`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the five-commit diff read-only, starting with the round-1 fixes and then checking both hosts against CSS justify-items behavior.The diff is small; next I’ll check the kernel’s `justify-items` model and whether a parent style change actually rebuilds the progress’s host CSS.No remaining findings.

Round 1 holds. Both hosts now force `justify-self: start` only for `normal` and `stretch`, and leave every other `justify-items` value to the browser. That matches Chrome’s block behavior (measured in the kernel fixtures): `normal` and `stretch` are the values that stretch an auto-width block child; `start`, `end`, `center`, `self-start`/`self-end`, `left`/`right`, `flex-start`/`flex-end`, `baseline`, and the `safe`/`unsafe` forms all shrink to fit and align, including under RTL. Legacy forms, CSS-wide keywords, `last baseline`, functions, and the empty value are the ones the justify-items writer turns into `unset` (initial is `legacy`, which computes to `normal`), and the bound writer maps those to `initial`.

The custom property no longer leaks. Every progress parent writes `--exact-progress-aligned` (`auto` when it aligns, `initial` when it does not). `initial` is the guaranteed-invalid value, so `var(--exact-progress-aligned, var(--exact-progress-justify, start))` falls through to the display rule. A block parent then resolves to `start`; a flex or grid parent resolves to `auto`. An absolute progress still gets a literal `auto` and is not affected. The wasm host cannot go stale: a parent style change already restyles progress children through `refold` in `element.rs`, which rebuilds host CSS from the parent’s current `justify_items` (the same path as a display change). Canvas still always writes `justify-self: start`, and nothing in this diff couples it to the progress rule.

The new tests fail without the fix. The element test expects no `justify-self: start` under `start`/`center`/`end`. The emit test requires the nested `initial`, and the bound mapper (`?"initial":"auto"` and `"--exact-progress-aligned"`) only when `justify-items` is bound. The adopted spinner expects x 110 under `justify-items: center`.

Verdict: the round-1 fixes are correct, and nothing else in the diff is wrong.
