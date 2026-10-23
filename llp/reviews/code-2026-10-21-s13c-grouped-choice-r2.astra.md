# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9bded1c9a5`.
- **Method:** one brief (sha256 `e6b32eb899f8116996253da403e95d39f20396b0147cb0de9c8dc17826826d9c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

VERDICT: RIGHT

No blocker, should-fix, or nit findings in HEAD.

The [kernel guard](/tmp/iosfix-rv-gc2/kernel/src/grouped.rs:500) reads the button’s own props. False survives [conversion](/tmp/iosfix-rv-gc2/runner/src/bridge.rs:195) and [SetProp emission](/tmp/iosfix-rv-gc2/runner/src/instance.rs:1047); `contains` correctly distinguishes an unselected choice from an action.

The [sheet guard](/tmp/iosfix-rv-gc2/contract/lower/src/grouped.rs:850) reads that row’s attributes after component inlining. Inlining preserves attribute names and substitutes their values. Literals, bindings and parenthesized expressions therefore agree. Attribute spreads are unsupported; styles cannot supply `aria-selected`. A component parameter only affects classification when explicitly forwarded onto the button. I found no new disagreement for supported boolean inputs.

`action` is serialized separately from `pressable`. Its only presentation consumer changes [title tint](/tmp/iosfix-rv-gc2/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1084); the [VoiceOver button trait](/tmp/iosfix-rv-gc2/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1113) and tap handling still use `pressable`. Web and Android have no additional consumer of this flag.

No existing affected grouped-list buttons were found in `apps/`, `examples/`, `packages/` or conformance fixtures. Their selection attributes belong to tabs, segmented controls or custom Calendar controls. Shelf’s [StatusControl](/tmp/iosfix-rv-gc2/apps/shelf/app.contract:625) nests its buttons inside a row, so those buttons are unaffected.

Both round-one requests are satisfied: the [test](/tmp/iosfix-rv-gc2/contract/cli/tests/it/grouped_action.rs:194) includes `unmarked`, checks both classifiers, exercises selection and retains an action control. File lengths are within the cap: `grouped_list.rs` 1,496; `grouped_action.rs` 214; compiler `grouped.rs` 1,496; kernel `grouped.rs` 755. Style matches the surrounding code.

`git diff --check` passed. No files changed; no Cargo or tests run.
