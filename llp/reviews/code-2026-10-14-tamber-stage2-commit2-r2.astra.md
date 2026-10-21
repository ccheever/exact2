# Code review: LLP 1121 stage 2, commit 2, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `49a36ff0b7a996be8886e8408e88054d2ce6a634563422658b5c27f04dae48a4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit (a3bc65d2f)

---

- **MAJOR — [theme.rs:340](/tmp/rv-s2c2/contract/lower/src/theme.rs:340):** Removing profile tokens before checking also removes their declaration validation. In `.tamber`, duplicate names and reserved names such as `red` can now pass; conversely, exporting a removed profile token produces `tamber-unknown-token`. Exclude their values from folding while preserving declaration checks.

- **MAJOR — [theme.rs:337](/tmp/rv-s2c2/contract/lower/src/theme.rs:337):** Profile aliases use exact spelling instead of role identity. A design declaring `-exact-secondary-label = color(--brand …)` and `ink = secondary-label` now rejects the alias as unknown. Normalize equivalent role spellings in both alias extraction and `when` reference checks.

- **MAJOR — [scan.rs:277](/tmp/rv-s2c2/contract/lower/src/theme/scan.rs:277):** The statement walker skips `Stmt::Call`, which holds expanded action calls. For `choose("light-dark(brand, card)")`, where `choose` assigns its argument to a colour-bound state, the synthetic token is still missing. The resulting runtime colour is invalid. Visit the call’s arguments and body.

- **MAJOR — [scan.rs:95](/tmp/rv-s2c2/contract/lower/src/theme/scan.rs:95):** Recursive scanning marks condition strings as strict colours. Valid code such as `color=(label == "light-dark(brand, CanvasText)" ? "brand" : "card")` now fails folding, although the compared string never supplies the colour. Strictness must follow value-producing branches.

- **MINOR — [style.rs:458](/tmp/rv-s2c2/host/web-js/src/style.rs:458):** Synthetic replacement also runs inside quoted URLs. Executing the old and new mapper confirmed that `url("https://example.com/light-dark(brand, surface.card).png")` now has its filename replaced with folded colour text. The outer function scan bypasses the URL protection in `T`.

- **MINOR — [theme.rs:379](/tmp/rv-s2c2/contract/lower/src/theme.rs:379):** Lowerer-owned design refusals still hard-code column 1. An indented `top` declaration starts at column 3; the new test checks only its line.

The re-check loop terminates and rebuilds indices consistently after `retain`. Manifest validation passes, and the JS mapper preserves numbers, `1.5px`, `"0.5"`, and ordinary URLs. Rust findings are from source inspection; available binaries predate the fixes. No edits made.

VERDICT: fix
