# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9636e63edd`.
- **Method:** one brief (sha256 `fb39af4d9526eba5ab0ccbbc7e421c7cdf9631af6e93fa251f01eb721d791c1a`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

VERDICT: RIGHT

No blocker, should-fix, or nit findings in HEAD.

The guard at [kernel/src/grouped.rs:500](/tmp/iosfix-rv-gc3/kernel/src/grouped.rs:500) reads the button’s own props. Boolean `false` survives binding evaluation and insertion; it is distinct from absence. `contains` implements the ruling correctly; testing for `Some(true)` would preserve the original bug.

The sheet checks the row’s own attributes at [contract/lower/src/grouped.rs:850](/tmp/iosfix-rv-gc3/contract/lower/src/grouped.rs:850), after component expansion. Forwarding preserves the attribute name; parenthesized expressions behave identically. Attribute spreads are unsupported, and classes cannot supply `aria-selected`. I found no implicit/default writer of `accessibilitySelected` or new disagreement for supported boolean values. Repeated rows and swipe content retain the attribute; grouped lists reject virtualization.

The iOS consumer uses `action` only for tint. Button traits remain governed by `pressable` at [GroupedListIOS.swift:1113](/tmp/iosfix-rv-gc3/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1113), as does activation. Serialization keeps these fields separate. macOS/tvOS use the sheet; Android does not consume this action flag.

No existing affected app/package/example/conformance rows found. Shelf’s selected buttons are nested segmented controls. The Dark/System choices at [grouped-list.contract:90](/tmp/iosfix-rv-gc3/scripts/fixtures/grouped-list.contract:90) omit `aria-selected`, so remain unchanged.

The regression test at [grouped_action.rs:217](/tmp/iosfix-rv-gc3/contract/cli/tests/it/grouped_action.rs:217) adequately covers both literals, bound false→true, selected without a checkmark, sheet colour, and an ordinary action control. Style matches surrounding code; all three files remain below 1,500 lines.

Static review only; no files edited or tests run.
