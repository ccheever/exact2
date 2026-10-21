# Code review: @exact/lists, ListRow and SwitchRow (stage 13 C), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at the branch's WIP tip then (since rebased; the commit "WIP stage 13 C: ListRow's leading symbol and trailing accessory one node each").
- **Method:** one brief (sha256 `e07c5d9a5625808221162f53ffa239567a60b10d73cb95b3f38c8182916222a9`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** should-fixes (no verdict line asked)
- **Disposition:** taken: an action row its own branch (symbol and title); one trailing accessory node; the leading-symbol and action-cell classifier gaps went to theme2, who ruled them classifier fixes (16:37), landed as 0efbaee726 and 536a2f3213

- **SHOULD-FIX — [list-row.contract:29](/tmp/iosfix-rv-l1/packages/lists/list-row.contract:29), [switch-row.contract:20](/tmp/iosfix-rv-l1/packages/lists/switch-row.contract:20):** Interpolated symbol sources bypass `grouped::symbol()`, which accepts only string literals. Inlining preserves these templates even with literal props. Consequently, stylesheet-based hosts miss the platform’s leading-symbol sizing and spacing. The untinted destructive symbol also misses the grouped sheet’s destructive tint.

- **SHOULD-FIX — [list-row.contract:27](/tmp/iosfix-rv-l1/packages/lists/list-row.contract:27):** The conditional children survive inlining, so `action_cell()` sees six children instead of its recognized title-plus-optional-symbol shape. An `accessory="none"` row therefore gets label-coloured text on the web; the gate’s “Help” row should use the platform’s action tint. Make the action face recognizable to grouped lowering.

- **SHOULD-FIX — [list-row.contract:35](/tmp/iosfix-rv-l1/packages/lists/list-row.contract:35):** The separate checkmark branch follows the chevron branch. Grouped lowering counts both when assigning child positions, so the chevron never qualifies as the trailing accessory and misses its platform dimensions and weight. Use one final conditional accessory position or correct the classifier.

Static review only; no builds or tests run. The acknowledged gallery failures are excluded.
