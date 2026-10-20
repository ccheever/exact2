# Code review: Stage 11 C2: the terminal held to its column (exact-terminal layout and plan), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6b9d8def04`.
- **Method:** one brief (sha256 `ca36a05ab97e4010f76aea45b4c8ed908d2441131638a3f3dbe5a96b015c26a4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one P2 (the even check required cells on graphical hosts), two P3s (negative radii unclamped; messages)
- **Disposition:** all taken in 61ef26cdd3

One blocking regression remains.

1. **P2 — `even` now fails graphical hosts.** [scale-check.mjs:251](/tmp/rv-s11c2t-2/scripts/scale-check.mjs:251) requires cells whenever `even: true`, but graphical layouts omit cells. Shared expectations now fail on otherwise correct graphical hosts. Restrict this requirement to the terminal.

2. **P3 — Negative resolved radii need clamping.** [cli.rs:181](/tmp/rv-s11c2t-2/host/terminal/src/cli.rs:181): on a 32×32 box, `calc(50% - 20px)` reports −4, whereas Linux’s `resolve_radii` clamps it to zero. Clamp both axes before scalar/pair encoding. Compare/env resolution otherwise follows `at_basis` correctly.

3. **P3 — Even-check diagnostics are imprecise.** [scale-check.mjs:255](/tmp/rv-s11c2t-2/scripts/scale-check.mjs:255) compares **opposite edges**, so “each edge” overstates the condition. The preceding “not in the reply” message also covers present but noninteger cells.

The nested elliptical JSON and its explicit rejection are correct. Cell retention, the 50% assertion, and QUEUE revision look sound. `terminalVerb` handles ordinary bare/non-table verbs and extra separators; internal whitespace survives, while outer whitespace is trimmed.

Read-only review; no builds or tests run.
