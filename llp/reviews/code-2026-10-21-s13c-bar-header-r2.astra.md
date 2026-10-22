# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4f4e76812b`.
- **Method:** one brief (sha256 `a400a580cc7cc4ab304e34ae506556a07f2d2f0787037a55f46a39a3992d0565`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in db94c10a4d (direct derive, derive scope, text_says bindings/arms/hidden/label precedence, docs); a narrow re-check followed

**VERDICT: WRONG**

1. **should-fix — [contract/lower/src/anatomy/library.rs:832](/tmp/iosfix-rv-bi2/contract/lower/src/anatomy/library.rs:832): Direct derive references bypass the state check.** With `derive fade = (selected ? 1 : 0.5)` and `opacity=fade`, `reads(..., false, ...)` collects nothing. Expansion therefore never reaches `selected`, and a button labelled only `"Home"` passes. Follow derive references before collecting conditional dependencies.

2. **should-fix — [contract/lower/src/anatomy/library.rs:756](/tmp/iosfix-rv-bi2/contract/lower/src/anatomy/library.rs:756): Shadowing incorrectly removes dependencies reached through derives.** Given `derive lit = selected`, a button inside `for selected …` using `opacity=(lit ? 1 : 0.5)` still reads the component’s state through `lit`. The final `!bound…` filter discards that dependency. Direct references are already filtered by `reads`; derive dependencies must retain their declaration scope.

3. **should-fix — [contract/lower/src/anatomy/library.rs:810](/tmp/iosfix-rv-bi2/contract/lower/src/anatomy/library.rs:810): `text_says` loses descendant bindings.** Its `Each` and `Match` branches pass `bound` unchanged. Consequently, `text selected` inside `for selected in ["Home"]` incorrectly satisfies an enclosing button’s requirement to announce the component’s `selected` prop. A locally shadowed derive name causes the same mistake. Carry the binders through this walker too.

4. **should-fix — [contract/lower/src/anatomy/library.rs:809](/tmp/iosfix-rv-bi2/contract/lower/src/anatomy/library.rs:809): Text outside the accessible name counts as announcing state.** `text_says` includes `aria-hidden` subtrees, and its result is accepted even when a constant `aria-label` overrides descendant text. Either allows a button whose appearance changes with `selected` to pass while its accessible name remains `"Home"`. Check the effective accessible name, including visibility and label precedence.

5. **should-fix — [contract/lower/src/anatomy/library.rs:813](/tmp/iosfix-rv-bi2/contract/lower/src/anatomy/library.rs:813): One branch announcing state satisfies every branch.** A button with state-dependent opacity and `if small` containing state-dependent text, followed by an `else` containing constant `"Home"`, passes: `texted` finds both arms named, while `text_says` combines them with `||`. When `small=false`, selection is never announced. Require coverage across the possible branches, preserving acceptance when an unconditional sibling supplies it.

6. **nit — [docs/contract-for-humans.md:1048](/tmp/iosfix-rv-bi2/docs/contract-for-humans.md:1048), [kernel/tables/schema.json:1428](/tmp/iosfix-rv-bi2/kernel/tables/schema.json:1428): The iOS documentation contradicts the change.** Both still say iOS exposes no `aria-current`. Document the mapping to `.selected` for authored pressables, including links.

By inspection, derive cycles terminate, `texted` handles the naming-sibling/else case, and BarItem/HeaderButton plus the requested iOS link/non-pressable cases follow the rulings. No files changed or tests run.
