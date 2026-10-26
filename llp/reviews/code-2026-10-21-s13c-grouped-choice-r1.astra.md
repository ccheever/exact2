# Code review: a grouped row that says whether it is selected is a choice, not an action cell (stage 13 C fix 2, theme2 16:37), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e957e19862`.
- **Method:** one brief (sha256 `9020e676f29bb143d7fa0ccfb3d90f24f612d81737d5df365a95c30e3bac8825`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken: the test moved to contract/cli/tests/it/grouped_action.rs (the cap); an aria-selected=true row with no checkmark added

**VERDICT: RIGHT WITH NITS**

1. **should-fix — [contract/cli/tests/it/grouped_list.rs:1400](/tmp/iosfix-rv-gc1/contract/cli/tests/it/grouped_list.rs:1400):** The addition grows this file from 1,496 to 1,554 lines, exceeding the mandatory 1,500-line cap. `caps.mjs` counts it as 1,555 because of the trailing newline. Move some grouped-list tests into another module before landing.

2. **nit — [contract/cli/tests/it/grouped_list.rs:1415](/tmp/iosfix-rv-gc1/contract/cli/tests/it/grouped_list.rs:1415):** Add an `aria-selected=true` row **without** a checkmark. Both currently tested true states have an accessory, which independently prevents action classification. The regression otherwise covers the important cases: bound false, literal false, an ordinary action, and selection changing at runtime.

The kernel logic is correct. [runner/src/bridge.rs:195](/tmp/iosfix-rv-gc1/runner/src/bridge.rs:195) preserves `false` as `PropValue::Bool(false)`, and [runner/src/instance.rs:1047](/tmp/iosfix-rv-gc1/runner/src/instance.rs:1047) emits `SetProp`. The `row` in [kernel/src/grouped.rs:353](/tmp/iosfix-rv-gc1/kernel/src/grouped.rs:353) is the button itself. `contains` therefore implements the ruling; checking only `Some(true)` would retain the original bug.

The sheet check is also correct. Component and slot expansion precedes lowering, and [contract/syntax/src/inline.rs:519](/tmp/iosfix-rv-gc1/contract/syntax/src/inline.rs:519) preserves attribute names while substituting values. Literals, bindings and parenthesized conditions all retain `aria-selected`. There is no attribute-spread mechanism; a component argument becomes a DOM attribute only through explicit forwarding. Row classes expand later, but `aria-selected` cannot legally occur in a style. I found no new classifier disagreement for valid boolean values.

Downstream behavior is sound. The Apple JSON bridge carries `action` separately from `pressable`; [GroupedListIOS.swift:1084](/tmp/iosfix-rv-gc1/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1084) uses it for title tint. VoiceOver’s button trait still follows `pressable` at [line 1113](/tmp/iosfix-rv-gc1/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1113), as does activation. No wire-format change or additional web/Android consumer of this flag appeared.

I found **no existing affected consumer** in `apps/`, `examples/`, `packages/`, corpus or conformance fixtures: their matching attributes belong to tabs or other nested controls. The Dark/System rows in [scripts/fixtures/grouped-list.contract:90](/tmp/iosfix-rv-gc1/scripts/fixtures/grouped-list.contract:90) omit `aria-selected`; their pre-existing static/live disagreement remains outside this fix.

Read-only inspection only; no files changed, Cargo or tests run.
