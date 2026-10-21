# Code review: @exact/lists, ListRow and SwitchRow (stage 13 C), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at the branch's WIP tip then (since rebased; "WIP stage 13 C, round 1 (Astra, Grok)").
- **Method:** one brief (sha256 `693ffc38dd061b77266bf5dc274ba33c4e4876d9882b26fd704291ddfa4849f8`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** should-fixes (no verdict line asked)
- **Disposition:** both are the classifier gaps, fixed on main (0efbaee726, 536a2f3213)

- **SHOULD-FIX — [packages/lists/list-row.contract:30](/tmp/iosfix-rv-l2/packages/lists/list-row.contract:30): Action-cell recognition still fails on the web.** The button’s children remain `[When, Text]`. `action_cell()` only accepts a text or a literal symbol followed by text; it does not recurse through `When`. Non-destructive `accessory="none"` rows therefore keep label colour instead of the action accent, contrary to lines 25–27. This remains broken even with `symbol=""`, independently of the deferred template-symbol metrics. Make the optional-symbol shape recognizable and verify the resulting action colour.

- **SHOULD-FIX — [packages/lists/list-row.contract:45](/tmp/iosfix-rv-l2/packages/lists/list-row.contract:45): An unselected check row becomes an action cell on iOS.** Removing the checkmark leaves a pressable title with no accessory or secondary text. `kernel/src/grouped.rs:494` consequently sets `action=true`, and `GroupedListIOS.swift:1084` colours its title with `.tintColor`. Selecting it restores the checkmark, clears `action`, and returns the title to label colour. Thus the new “Light” and “System” samples have different title colours on iOS without an authored colour override. Preserve choice-row presentation in both states and cover that pair on iOS.

No additional findings against D0/D1/D2/D4/D5/D6 under the stated rulings. Source review only; no builds or tests run, and no files changed.
