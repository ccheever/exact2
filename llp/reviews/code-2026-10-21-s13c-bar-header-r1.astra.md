# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fad694e0ee`.
- **Method:** one brief (sha256 `9b0b93a7a8ce9347ada6b12ec14b6d3884ad48172a4182030507d24db685aabf`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken: derives, bindings, empty names (74328044af); hold and aria-selected to theme2 (06:40: hold dropped, aria-current=page with iOS's selected trait, 4f4e76812b)

VERDICT: **WRONG**

1. **should-fix — [contract/lower/src/anatomy/library.rs:695](/tmp/iosfix-rv-bi1/contract/lower/src/anatomy/library.rs:695): Derived labels are rejected, and derived looks bypass the rule.**  
   `derive spoken = selected ? "Selected" : "Select"` followed by `aria-label=spoken` fails when the button’s opacity directly reads `selected`. `mentions()` sees only `spoken`; the check uses the authored component, before derive expansion introduces its expressions/lets. Conversely, moving the visual conditional into a derive hides it from `looks()`. Resolve dependencies through derives on both sides.

2. **should-fix — [contract/lower/src/anatomy/library.rs:701](/tmp/iosfix-rv-bi1/contract/lower/src/anatomy/library.rs:701): A visible label cannot satisfy the state check.**  
   A button with `opacity=(selected ? 1 : 0.5)` and `text (selected ? "Selected" : "Select")` is rejected despite speaking its state. Only the button’s `aria-label` and five state attributes count. An `aria-labelledby` target that reads the prop is likewise ignored. This contradicts the documented “label that reads it” alternative.

3. **should-fix — [contract/lower/src/anatomy/library.rs:734](/tmp/iosfix-rv-bi1/contract/lower/src/anatomy/library.rs:734): Loop and match bindings are mistaken for component props.**  
   The walkers discard `Each` and `Match` bindings, and each attribute starts with an empty bound-name list. With a component prop `current: bool`, a loop `for current in items` containing a named button with `opacity=(current == "x" ? 1 : 0.5)` incorrectly demands an announcement of the component’s state. Carry lexical scope through both walkers, as the existing variant walker does.

4. **should-fix — [contract/lower/src/anatomy/library.rs:684](/tmp/iosfix-rv-bi1/contract/lower/src/anatomy/library.rs:684): The naming check accepts provably unnamed buttons.**  
   `aria-label=""` passes by attribute presence. `text ""` and `text "Go" aria-hidden=true` pass because `texted()` checks only the tag. A label in just one conditional arm also suffices when the other arm leaves the button unnamed. These are direct false negatives for the promised naming rule; check known-empty/hidden content and branch coverage.

5. **should-fix — [packages/actions/bar-item.contract:45](/tmp/iosfix-rv-bi1/packages/actions/bar-item.contract:45): The selected state lacks supported web semantics.**  
   This produces a button with `aria-selected`, without `role="tab"`. ARIA supports selected state on tabs and certain other roles, but not buttons. [WAI-ARIA](https://www.w3.org/TR/wai-aria-1.2/#aria-selected)  
   Use proper tab/tablist semantics when these are tabs, or the ruling’s permitted `aria-current` representation for navigation. Preserve the native announcement when resolving this: the current UIKit implementation explicitly does not expose `aria-current`.

6. **should-fix — [packages/actions/bar-item.contract:47](/tmp/iosfix-rv-bi1/packages/actions/bar-item.contract:47): A no-op hold changes right-click behavior.**  
   [onContextmenu](/tmp/iosfix-rv-bi1/host/web-js/rt.js:948) calls `preventDefault()` and `stopPropagation()` before invoking the action; the wasm host does the same. Consequently, every BarItem suppresses browser and ancestor context menus even when `hold` does nothing. Items without a hold action need a genuinely absent binding.

The token literals, part groups, unconditional slot placement—including `d=""`—and HeaderButton’s `id`/`testId` and small/lead variants look consistent. Direct conditional classes, expression matches, Use arguments, buttons inside `for`/`if`, and conditional `"button"` roles are traversed. Static review only; no builds, tests, or edits.
