# Code review: @exact/lists, ListRow and SwitchRow (stage 13 C), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at the branch's WIP tip then (since rebased; "WIP stage 13 C, round 1 (Astra, Grok)").
- **Method:** one brief (sha256 `693ffc38dd061b77266bf5dc274ba33c4e4876d9882b26fd704291ddfa4849f8`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker and a should-fix (no verdict line asked)
- **Disposition:** the blocker is the classifier gap fixed on main (0efbaee726); the should-fix (a destructive row's symbol kept the tint param) taken in c958c4c060, tested

I'll review the branch as it stands, read-only: the `@exact/lists` diff against `origin/main`, then the platform cells and the design rules those rows have to match.The workspace is the exact2 tree. Next I’ll read the branch diff and the cell rules it has to match.The checkmark and the single trailing accessory node match the platform cells. The action row still does not.

**BLOCKER** — An action row is not the web grouped sheet's action cell. `packages/lists/list-row.contract:30`

The `accessory="none"` button's children are an `if symbol != ""` and then the title. `action_cell` (`contract/lower/src/grouped/actions.rs:9`) accepts only a lone `text`, or a literal `symbol:` image followed by a `text`. A condition is not hidden and is not a symbol, so the check fails for every action row, including `symbol=""`. The sheet then paints the title with the label colour (`contract/lower/src/grouped.rs` sets action ink only when that check passes), unless `destructive` replaces it. iOS still sees the flattened children, sets `GroupedRow.action`, and draws the title in the tint. `sample-lr-none` (Help) is accent on iOS and label on the web. A templated symbol would also fail `symbol()`, which only reads literals — that part is the queued compiler change — but the `if` rejects the row before that, and it keeps the image off index 0, so the queued change would not reach this button either.

**SHOULD-FIX** — A destructive symbol on a chevron, value, or check row stays the label colour on the web. `packages/lists/list-row.contract:39`

That image always sets `-exact-tint-color="param.tint"` (the label role). The authored tint replaces the sheet's destructive symbol tint, so the title is red and the symbol is not. iOS applies the authored tint and then forces the destructive symbol to red (`GroupedListIOS.swift`, the destructive assignment after the symbol tint). The action branch already omits the tint so a destructive symbol can take the row colour; this branch does not.

The round 1 checkmark and trailing-node fixes hold. A check image is only in the tree when `accessory` is `check` and `selected` is true, and that image is the button's last child, so the sheet gives it the checkmark metrics and iOS reports a checkmark accessory. The chevron is that same last child and gets the disclosure metrics. `SwitchRow` is the recipe's row: a title, then one switch with `checked`, `aria-label`, and `accent-color`, and that switch is its only part. `ListRow` has no parts. Tokens, not literals, are the param defaults, and nothing here reads the host. The gate's extra rows are samples, which matches D5: these props are read by `if`, not by a class or a style `?:`. Nothing checks that an action row's title is the accent, or that the unselected check sample has no checkmark.
