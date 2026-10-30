# Code review: the Mac window bar's read set, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `19aa31ba9c`.
- **Method:** one brief (sha256 `43ebea116b2c4000db28d6342b49a837fb0371a92b1cccb0ba0a079b1d770fab`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Three MAJOR gaps remain. No files changed.

The eight round-1 fixes are present. Committed regressions cover rejected headers, rejected named views, external/unresolved labels, shadow record restoration, opacity, and system-colour invalidation. The expanded shadow comparisons, indexed Back lookup, and `unplaced` explanation are also present. HEAD reports passing tests and timing measurements; I could not rerun them in this read-only checkout without built test artifacts. Coverage does not independently exercise every added shadow field.

The skip path no longer sorts all named views. I found no additional concrete performance defect.

VERDICT: CHANGES

1. **MAJOR — Named inline elements are missing from invocation dependencies.** [ToolbarReadsMac.swift:153](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:153)

   `named()` records only `ChromeIndex.named` views, but [invocation resolution](host/apple/Sources/ExactKit/Invocation.swift:93) also searches paragraph runs. A header button with unresolved `commandfor="guard"` can currently use its `popovertarget` fallback. Adding `id="guard"` to a run in an existing external paragraph suppresses that fallback. The paragraph owner is outside `reads.nodes`, so its `.paragraph` operation is skipped; the bar retains its old menu item or projection decision. Removing the inline name has the inverse problem.

   Record existing inline owners and detect referenced names introduced by paragraph updates. Add a regression with the shadow disabled.

2. **MAJOR — Secondary navigation can change declaration visibility without changing recorded inputs.** [ToolbarReadsMac.swift:126](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:126)

   Inputs capture the primary navigation selection and header-route visibility. However, [navigation gates routes under every navigation root](host/apple/Sources/ExactKit/Mac/NavigationMac.swift:164), and declarations can come from any visible toolbar carrier.

   Consider a secondary root selecting route B, which contains a declared window toolbar. Change an unread, earlier sibling A’s `navigationKey` to B’s key. Navigation selects A and hides B directly. The operation concerns no recorded node, while the primary selection and recorded header routes remain unchanged. The bar therefore keeps B’s toolbar after its owner becomes hidden.

   Capture the host-controlled visibility of declaration candidates, or the navigation decisions governing them. Extend the existing rekey regression to a secondary root.

3. **MAJOR — Window appearance changes do not invalidate resolved toolbar colours.** [ToolbarReadsMac.swift:128](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:128)

   System-colour notifications now present the bar, but effective appearance is absent from its inputs. [The appearance callback](host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:421) reports measuring traits and scheme without explicitly refreshing it. A window appearance change need not produce a system-colour notification or a relevant style mutation: an unchanged serialized `light-dark()` pair can remain unchanged on the wire.

   [Toolbar ink](host/apple/Sources/ExactKit/Mac/ToolbarInkMac.swift:9) and prominent-item tint store colours resolved during presentation. They can consequently retain the previous appearance’s colours. The shadow may refresh these during a skipped batch, so visual checks with it enabled cannot establish release correctness.

   Invalidate on effective appearance changes and test switching `NSWindow.appearance` with the shadow disabled, asserting the resulting ink and tint.
