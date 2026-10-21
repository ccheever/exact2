# Code review r3 (final): a grouped-list section without its card, 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `1a51038c067bdb4863cf1eec62a16fd228ece4fa9b2ae5bd19f7f3f91b6a7c27`), shared with grok. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** Landed with no further round. 1 taken: the conditional clear background is removed. Rendered on the simulator before and after: with it, the header sat on the route's white; without it, on the list's grey. DEFERRED: an automated rendered check.

---

Static review only; no files modified or builds/tests run. One should-fix remains.

1. **Should-fix — Clearing the section background removes the list canvas, not merely the card.**  
   [GroupedListIOS.swift:263](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:263)

   `UICollectionLayoutListConfiguration.backgroundColor` does apply per section—the installed SDK explicitly says so. However, its default supplies the appearance’s **list background**; the rounded cell background is controlled separately by `backgroundConfiguration`. Round 2’s claim that this property supplies another rounded card was incorrect. [Apple’s list-background documentation](https://developer.apple.com/documentation/uikit/uicollectionlayoutlistconfiguration-swift.struct/backgroundcolor), [cell-background documentation](https://developer.apple.com/documentation/uikit/uicollectionviewcell/backgroundconfiguration-rgj4).

   Setting it `.clear` leaves neighboring sections’ configurations intact, but removes the canvas beneath this section, exposing the collection view’s own background. That background is never configured in [the initializer:222](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:222). Consequently, the transparent profile section relies on a UIKit default rather than the grouped background surrounding the remaining cards. Apple’s own transparent-list example explicitly configures the underlying collection background. [Apple tutorial](https://developer.apple.com/tutorials/app-dev-training/creating-a-progress-view).

   **Fix:** remove the conditional `c.backgroundColor = .clear`. Keep the appearance’s section background, clear the card-less cells, and disable their separators. Add a rendered mixed-section check in light and dark appearances, comparing the background behind the profile with the margin beside a normal card. The existing cell-configuration assertions cannot detect this discrepancy.

The other round-2 findings are resolved:

| Finding | Disposition |
|---|---|
| Astra 1: retained row moving between sections | Resolved by comparing effective card values by row ID at [GroupedListIOS.swift:295](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:295). The new test covers entry into the card-less section; the implementation handles both directions. |
| Astra 2: custom-row border and height | Resolved: card-less rows retain their full height at [GroupedListIOS.swift:525](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:525). |
| Astra 3: author-guide restrictions | Resolved at [contract-for-humans.md:977](/tmp/x14-review/docs/contract-for-humans.md:977). |
| Grok 2: sheet corner clipping | Resolved: only carded inset groups receive the radius at [grouped.rs:450](/tmp/x14-review/contract/lower/src/grouped.rs:450). |

Deferring direct UIKit configuration inspection and separator assertions remains acceptable. The background behavior needs the rendered check described above. I found no additional landing issues.

Verdict: LAND WITH FIXES
