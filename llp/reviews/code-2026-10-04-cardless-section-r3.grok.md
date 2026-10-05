# Code review r3 (final): a grouped-list section without its card, 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `1a51038c067bdb4863cf1eec62a16fd228ece4fa9b2ae5bd19f7f3f91b6a7c27`), shared with astra. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND.
- **Disposition:** The nit is taken in §6.2's text. DEFERRED: a full-height custom-row test.

---

**Round 2**

- Grok 1 (section fill still painted), the blocker, is resolved. A card-less section sets the list configuration’s `backgroundColor` to `.clear` at `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:264`. Reading that configuration back from the laid-out section stays deferred: `NSCollectionLayoutSection` does not expose it, and a private decoration-view check would be brittle.
- Grok 2 (inset sheet clipped to the card radius) is resolved. An inset card-less group keeps the 16-pt side margins and skips `border-radius` at `contract/lower/src/grouped.rs:447`.
- Astra 1 (a moved row keeps the old card) is resolved. The previous card is stored by row id, and a row whose section card differs is configured again at `GroupedListIOS.swift:295`. `testARowMovedIntoACardlessSectionLosesItsCard` moves a standard row onto a card-less section. The comparison is symmetric, so the return trip takes the same path.
- Astra 2 (card-less custom row loses its own bottom border) is resolved in code. `carry` subtracts `border_width_bottom` only while the section still has a card, at `GroupedListIOS.swift:525`. The requested test, an explicitly bordered custom row followed by another row, was not added.
- Astra 3 (author guide) is resolved at `docs/contract-for-humans.md:977`: only the literal `transparent`, not beside a `class`, and a section that gains and loses its card is two sections under `when`.

The round-1 fixes those reviews already accepted (tvOS `showsSeparators` guard, literal-only refusal, no sheet separators or grouped edge borders, highlight handler, class refusal) are still in place.

**`backgroundColor`**

Yes. In the iOS 27 SDK, `UICollectionLayoutListConfiguration.backgroundColor` is “the background color of the section,” and nil means the appearance’s system color (`UICollectionLayoutList.h:89`). That system color is the card fill (`#ffffff` / `#1c1c1e` for inset grouped), not the list backdrop.

`layout()` builds a new configuration inside the compositional layout’s section provider and turns it into one section with `NSCollectionLayoutSection.list(using:layoutEnvironment:)`. `.clear` is set only when that section’s `card` is false. It is not nil, so it does not fall through to the system card. Neighbouring sections leave the property nil and keep their card fill. `collection.backgroundColor` is never written; the collection view stays transparent over the list node, whose sheet background is the grouped or plain backdrop (`contract/lower/src/grouped.rs:194`). A clear section decoration shows that backdrop. The other sections’ cards stay.

1. **Nit — The full-height custom row is untested, and §6.2 still describes the old cell.** `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:525`, `llp/1084-native-grouped-lists.rfc.md:221`

`carry` now keeps a card-less custom row’s full frame, but every UIKit height assertion still uses a carded row (`GroupedListIOSTests.swift:172` expects 61 minus the 1-pt border). §6.2 and D5 still say every custom cell is the kernel height minus the bottom border, and they never mention the section’s clear fill. A later edit that always subtracts the border would pass the suite and clip an authored border again.

**Fix:** add a card-less custom row with `border_width_bottom` of 2, followed by another row, and assert that cell’s height equals its frame while a carded custom row still loses the border width. In §6.2, state the clear section fill, the omitted inset radius, and this height exception.

Verdict: LAND
