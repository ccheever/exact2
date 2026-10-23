# Code review: iOS grouped lists: a custom row's leading symbol and line as a standard cell's (Harmony), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d184372a4e`.
- **Method:** one brief (sha256 `f3fdd205b7d10e69244280a9d56ab84a374d30d9ceab0299b3b428f8c44fbc86`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in e7671be379 (slot only, marks cleared, relayout, RTL, a section's last row, the doc comment)

VERDICT: WRONG

1. **should-fix — [NodeViewIOS.swift:597](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:597): Authored image fitting is overridden.** `listSymbol` unconditionally forces unclipped `.center`, ignoring explicit `object-fit="contain"`, `"cover"` or `"scale-down"`. An authored smaller image box can now paint outside that box. Apply the list default only where the author left sizing/fitting unsaid; preserve the existing authored path.

2. **should-fix — [GroupedCustomRowsIOS.swift:16](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:16): The flag outlives its list role.** Only currently shown direct children are reconciled. A previously marked image moved into a nested container or outside the row keeps `listSymbol`; becoming standard or removing the projection also leaves it set. `restore()` restores hierarchy and frames, not this state. Track previously marked views and clear departing ones, including hidden children. Avoid clearing and reapplying every flag during each temporary restore.

3. **should-fix — [GroupedCustomRowsIOS.swift:20](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:20): Not every marked image has the fixed sheet box.** A custom `column` row can start with an unsized symbol; lowering deliberately leaves that column’s children unchanged. This code nevertheless marks it, making its large glyph overflow a box measured at medium scale. Thus the intrinsic mismatch is harmless for the intended 24×20 slot, but not for everything this predicate accepts. Restrict eligibility to the intended sheet slot or reconcile measurement too; changing only the queued intrinsic report is insufficient because the kernel ignores reports for symbols it already measures.

4. **should-fix — [GroupedListIOS.swift:606](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:606): Separator geometry lacks an update trigger.** A conditional leading symbol or changed padding can move the content inset while the row remains custom and keeps its height. The model remains equal, so `update()` skips the snapshot, and `mount()` invalidates only for a height change. The new separator value can therefore remain cached until another invalidation. Track inset changes and invalidate after the data-source update. The test’s explicit `invalidateLayout()` masks this path.

5. **should-fix — [GroupedCustomRowsIOS.swift:30](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:30): Physical left coordinates are used as directional leading insets.** The calculation matches `carry` in LTR. In RTL, UIKit measures `leading` from the right, so asymmetric padding or an offset/narrower row produces the wrong inset. Derive the appropriate edge in cell coordinates using the effective direction. [Apple’s directional-inset definition](https://developer.apple.com/documentation/uikit/nsdirectionaledgeinsets/init%28top%3Aleading%3Abottom%3Atrailing%3A%29-6wnda?changes=__4).

6. **should-fix — [GroupedListIOS.swift:606](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:606): The override also changes section boundaries.** It applies to every custom row in every appearance, including the last row of a `.grouped` section. That section’s bottom separator should remain full width, as documented in LLP 1084 §2; this replaces its leading inset with 16/56. Preserve UIKit’s section-boundary configuration and adjust interior separators.

7. **nit — [Affordances.swift:64](/tmp/iosfix-rv-gcr1/host/apple/Sources/ExactKit/Affordances.swift:64): The existing cache-key documentation now describes the wrong property.** Move it back above `symbolLookKey`. The duplicated Harmony incident history in the new helper and test can also be shortened.

The remaining checks look sound: `mount()` repeats marking without cell reconfiguration; the guarded setter and deferred intrinsic queue do not introduce synchronous re-entry or an unchanged-state loop. Authored font size, weight and rendering still apply. Ordinary no-symbol rows resolve to their margin; `owned` separators remain hidden; automatic insets support per-edge overrides. Package access and all line caps are fine. tvOS’s runtime model remains disabled.

Dynamic Type glyph sizing follows the published body size, but full alignment across categories is unproven: UIKit’s [reserved symbol space scales with the category](https://developer.apple.com/documentation/uikit/uilistcontentconfiguration-swift.struct/imageproperties-swift.struct/reservedlayoutsize), while this slot stays fixed. The 17-point test and AppKit build also do not qualify Catalyst behavior.

Read-only review; no tests run.
