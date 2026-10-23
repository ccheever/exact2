# Code review: iOS grouped lists: a custom row's leading symbol and line as a standard cell's (Harmony), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `72465c42f6`.
- **Method:** one brief (sha256 `97b3fc31bcf2e89401923615155365e1d41f364a9ef2ad81a79f15dee8d16ad6`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in the round 3 commit: wrote() for an authored fit or size; order-independent marks checked at mount. Open, to theme2: right to left, and the fixed slot at accessibility sizes

VERDICT: **WRONG**

1. **should-fix — [GroupedCustomRowsIOS.swift:51](/tmp/iosfix-rv-gcr3/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:51): Explicit `contain` is distinguishable from the sheet’s fit.** `object_fit` retains its authorship in `style["written"]`, readable through [NodeView.wrote](/tmp/iosfix-rv-gcr3/host/apple/Sources/ExactKit/IOS/NodeExtrasIOS.swift:423). Nevertheless, an explicitly authored `object-fit: contain` in a 24×20 box is marked and rendered centred, unclipped. Respect that existing bit. Width/height provenance is omitted from the host payload, so the remaining size-only ambiguity **should go to the RFC owner**; the blanket exception is not justified under “author > platform.”

2. **should-fix — [GroupedCustomRowsIOS.swift:29](/tmp/iosfix-rv-gcr3/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:29): Mark ownership remains dependent on traversal order.** If two custom rows exchange existing symbol views, each `look` clears its previous symbol—even if the other row just marked it. One image ends unmarked, and subsequent mounts do not repair it because the cached identity already matches. There is also an offscreen variant: `update` retains the cache while the old row remains custom, and `mount` revisits only visible cells, so an image moved elsewhere can retain its mark. Reconcile obsolete marks before assigning current ones, including tracked offscreen rows.

3. **should-fix — [GroupedCustomRowsIOS.swift:74](/tmp/iosfix-rv-gcr3/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:74): The uncarried RTL calculation still assumes symmetric geometry.** For a 100-point row at x=16 inside a 370-point cell, with `box.maxX=84`, the correct leading inset is **270**; the fallback returns **32**. Furthermore, `carry` records `carried[id]` before calling `look`, while `row.superview` can still be its authored parent. Thus neither branch reliably supplies the cell width before attachment. An unchanged cached line and height can also leave `willDisplay` without a correction request. The current full-width RTL test misses this case.

4. **nit — [GroupedCustomRowsIOSTests.swift:27](/tmp/iosfix-rv-gcr3/host/apple/tests/ExactKitTests/GroupedCustomRowsIOSTests.swift:27): Dynamic Type parity remains unqualified.** The comparison fixes the font at 17 points. The glyph configuration preserves authored size, weight and rendering, but UIKit’s reserved image space scales with content-size category and changes at accessibility sizes. Add a comparison at larger sizes before claiming matching placement and clipping throughout Dynamic Type. [Apple’s documentation](https://developer.apple.com/documentation/uikit/uilistcontentconfiguration-swift.struct/imageproperties-swift.struct/reservedlayoutsize)

The intrinsic report is asynchronous; unchanged marks do not repeatedly redraw. The fixed slot preserves kernel layout and hit geometry, while the agent reports the larger native image size. Changing only `.leading` is valid: automatic inset placeholders operate per edge. [Apple](https://developer.apple.com/documentation/uikit/uilistseparatorautomaticinsets?language=objc)

Owned separators and section boundaries remain protected. tvOS does not instantiate this projection. Package access is appropriately scoped; the files remain under the cap at **1,497 / 1,482 lines**.

Static review only; no edits or builds/tests run. These unresolved items go to the RFC owner after round 3.
