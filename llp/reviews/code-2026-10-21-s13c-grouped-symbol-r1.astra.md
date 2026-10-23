# Code review: grouped lists read a row's symbol however it's given (stage 13 C fix 1, theme2 16:37), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b0413f0d90`.
- **Method:** one brief (sha256 `611b9dff5e8c149bd66dcb521b3f97888353acf0f227b70487614949c342b5b4`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken: the test moved to a new module, contract/cli/tests/it/grouped_action.rs (the cap); the nit's cases added (the model's action and symbol, a template under an if in both states, refused branches, the pushing row's title)

VERDICT: WRONG

1. **blocker — [contract/cli/tests/it/grouped_list.rs:1498](/tmp/iosfix-rv-gs1/contract/cli/tests/it/grouped_list.rs:1498).** The added test takes this file to **1,567 lines**, exceeding the enforced 1,500-line limit. `caps.mjs` counts 1,568 including the final empty split entry and will report `cap:lines`. Move the regression into a smaller test module before landing.

2. **nit — [contract/cli/tests/it/grouped_list.rs:1517](/tmp/iosfix-rv-gs1/contract/cli/tests/it/grouped_list.rs:1517).** Coverage is narrower than the motivating case: it tests a template and an `if` separately, with the condition always true. Add the combined template-under-`if` case in both states, plus a rejected branch containing text or multiple symbols. The assertions correctly check title ink, but “frames” means only width/height; insets, icon tint, and the native symbol name remain unchecked. The pushing-row assertion checks the row’s colour rather than its title.

The functional trace otherwise looks sound. Every grouped `symbol()` caller uses its result for classification or name comparisons; none emits it as an image source. Templates now receive leading-symbol insets, dimensions and tint, and qualify for subtitle-stack/title-button recognition. The original expression survives, and [kernel/src/grouped.rs:228](/tmp/iosfix-rv-gs1/kernel/src/grouped.rs:228) reads the resolved source independently.

A runtime chevron/checkmark **before the title** is also a leading image to the kernel. A trailing template cannot newly become an action cell: `[text, image]` still fails classification. Dynamic trailing accessories still miss compiler accessory sizing/disclosure styling, as before; native accessory detection retains their real names. Thus the placeholder is safe for this change’s action classification, but does not solve dynamic accessory styling generally.

`leading()` handles empty branches, nested `if`s and `else if` correctly; it ignores literal `display="none"` nodes and rejects text, loops and multiple visible children. Naming and comment density match the surrounding code.

I found no affected checked-in app/example/standalone fixture. The conditional checkmarks in [scripts/fixtures/grouped-list.contract:92](/tmp/iosfix-rv-gs1/scripts/fixtures/grouped-list.contract:92) are trailing and unchanged. The existing conditional-leading-symbol test at `grouped_list.rs:319` gains the intended accent ink.

Read-only source review; no Cargo or tests run.
