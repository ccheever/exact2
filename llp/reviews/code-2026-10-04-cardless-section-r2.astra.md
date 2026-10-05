# Code review r2: a grouped-list section without its card, 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the round-1 artifacts commit.
- **Method:** one brief (sha256 `80522b31bdfe2df64b8cf795f859d0ad62b71fb21a64fd3ec6b576beef6b4ee0`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: effective card compared by row id; test moves a row into a card-less section. 2 taken: card-less custom rows keep their full height. 3 taken: contract-for-humans states the rule.

---

Static review only; no files modified or builds/tests run. Round 2 resolves the main round-1 defects, with two remaining behavior issues.

| Round-1 finding | Disposition |
|---|---|
| Astra 1: tvOS compilation | Resolved: `showsSeparators` is guarded at [GroupedListIOS.swift:260](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:260). |
| Astra 2 / Grok 2: coloured and appearance-dependent backgrounds | Acceptably narrowed: nonliteral and non-`transparent` values are refused at [grouped.rs:381](/tmp/x14-review/contract/lower/src/grouped.rs:381). |
| Astra 3 / Grok 4: sheet separators and grouped borders | Default decorations are removed correctly at [grouped.rs:438](/tmp/x14-review/contract/lower/src/grouped.rs:438) and [grouped.rs:623](/tmp/x14-review/contract/lower/src/grouped.rs:623). Authored custom-row borders have the exception below. |
| Astra 4 / Grok 3: standard-row feedback | Resolved: the handler restores both highlighted and selected backgrounds at [GroupedListIOS.swift:369](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:369). |
| Astra 5 / Grok 1: class precedence | The literal-plus-class case is explicitly refused at [grouped.rs:393](/tmp/x14-review/contract/lower/src/grouped.rs:393). This is an acceptable scope reduction; the author-guide explanation remains incomplete. |
| Astra 6: ineffective transition test | The test now exercises an unchanged standard row through both transitions at [GroupedListIOSTests.swift:245](/tmp/x14-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:245). Deferring direct UIKit separator assertions is acceptable. Selected, disabled, custom-row and refusal-variant coverage remains thin. |

1. **Should-fix — Moving an existing row between sections can retain its previous background policy.**  
   [GroupedListIOS.swift:292](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:292) computes `recarded` from changes to each **section’s** flag. If an unchanged standard row moves from carded section A to card-less section B while both flags remain unchanged, it is absent from `changed`. Its existing cell can retain A’s background and handler; the reverse move can retain the clear-background handler. Applying a diffable snapshot without animation does not force cell reconfiguration. [Apple’s explanation](https://developer.apple.com/videos/play/wwdc2021/10252/?time=121).

   **Fix:** compare previous and next effective `card` values **by row ID**, and reconfigure retained rows whose value changes. Add a test moving the same standard row ID between the two sections in both directions.

2. **Should-fix — A card-less custom row loses an explicitly authored bottom border on iOS.**  
   The sheet preserves author overrides, but [GroupedListIOS.swift:516](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:516) still subtracts every custom row’s bottom-border width from its cell height and clips the content. With native separators now disabled, nothing replaces that border.

   For example, a custom avatar row with `border-bottom-width=2` and a red border, followed by another row inside a transparent section, draws its border on the sheet but loses it—and two points of height—on iOS.

   **Fix:** for card-less sections, carry custom rows at their full authored height. Retain border subtraction where UIKit supplies the replacement separator. Cover an explicitly bordered custom row followed by another row.

3. **Nit — The author guide omits the new restrictions.**  
   [contract-for-humans.md:977](/tmp/x14-review/docs/contract-for-humans.md:977) describes the transparent form but does not say that other background expressions and any accompanying `class` are refused. The unrestricted override wording at line 994 compounds this.

   **Fix:** state the literal-only and no-class restrictions beside the example, and explain that switching card presence requires conditional sections.

I found no additional `when` or LLP 1091 interaction: `over()` visits both conditional arms, and module resolution/component substitution precede grouped lowering. The handler itself has no retain cycle: it captures only `highlights`, receives the cell as an argument, and is replaced or cleared whenever `configure` runs. Ordinary recycled-cell reuse is therefore handled; finding 1 concerns retained rows that skip configuration. Custom rows remain clear without acquiring UIKit feedback, and selected state is explicitly supported.

Verdict: LAND WITH FIXES
