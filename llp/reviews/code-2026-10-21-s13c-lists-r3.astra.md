# Code review: @exact/lists, ListRow and SwitchRow (stage 13 C), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c958c4c060`.
- **Method:** one brief (sha256 `25487ca659bd0b8ba84e6cc1d0fffa679bdeeceb7861933b79a0cde1433d5081`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

VERDICT: RIGHT

1. **Severity: none** — [list-row.contract:39](/tmp/iosfix-rv-l3/packages/lists/list-row.contract:39) and :52 preserve leading-symbol recognition. `leading()`, `part()` and inset calculation recurse through both conditions; the kernel reads the rendered image directly. Destructive symbols receive the sheet’s tint.

2. **Severity: none** — [list-row.contract:9](/tmp/iosfix-rv-l3/packages/lists/list-row.contract:9) and [switch-row.contract:6](/tmp/iosfix-rv-l3/packages/lists/switch-row.contract:6) retain D1’s token-backed knobs. D5 remains satisfied: structural conditions introduce no variants. The AccentColor default and priority change introduce no issue.

3. **Severity: none** — [library_packages.rs:118](/tmp/iosfix-rv-l3/contract/cli/tests/it/library_packages.rs:118) checks the reported regression: destructive symbol tint equals its title’s color and differs from the plain symbol’s.

No actionable findings. Static review only; no files changed or tests run.
