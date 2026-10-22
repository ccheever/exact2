# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `683d4064e5`.
- **Method:** one brief (sha256 `270c9b833710562f0fd8d767d8aa53582dbb090717a3e2edec2eeba58303c82e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** after round 3: the per-batch re-check removed (its blocker and the shadow point with it), the scroll view's authored corner taken; to theme2: the context-menu limit

VERDICT: **WRONG**

1. **blocker — [MenusIOS.swift:149](/tmp/iosfix-rv-ps3/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:149): Context-menu invalidation can detach painted children.** This refresh runs after the final `flats.flush()` in [PresenterIOS.swift:994](/tmp/iosfix-rv-ps3/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:994). When another node’s `contextPopover` starts naming an open, marked popover, `updateMaterial()` removes its glass container. It moves child `NodeView`s, but flattened child layers only get queued for remounting through `containerChanged`. No flush follows, so those children can disappear until another batch. The refresh must finish remounting within the same batch; test a source-only change with a flattened child.

2. **should-fix — [MenusIOS.swift:149](/tmp/iosfix-rv-ps3/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:149): The same invalidation does not refresh the shadow.** `updateMaterial()` neither invalidates the popover’s display nor calls `applyShadow`. Changing only another node’s `contextPopover` therefore changes the computed `defaultSurfaceShadow` without updating the existing `shadowCaster`: the old shadow can remain after exclusion, or remain absent when exclusion clears. Shadow installation/removal happens separately in [BoxShadow.swift:209](/tmp/iosfix-rv-ps3/host/apple/Sources/ExactKit/BoxShadow.swift:209). Test the actual caster or pixels after source-only changes.

3. **should-fix — [Backdrop.swift:178](/tmp/iosfix-rv-ps3/host/apple/Sources/ExactKit/Backdrop.swift:178): Scrolling popovers with authored corners still have square content clipping.** `writesCorner` makes this helper skip clipping or reset the scroll radius to zero. For `popover="auto" border-radius=12 overflow-y="scroll"`, the glass takes the authored radius, but children live in the sibling scroll view, outside that glass’s clip. The node itself does not clip for scrolling overflow. A filled child consequently covers the rounded corner. Preserve authored clipping on the scroll container, including nonuniform corners. The current authored-zero test checks only `writesCorner`, so it cannot catch this.

The compiler’s authored-surface handling, metric kinds, inert web attribute, Mac behavior, and stated pre-26 limitation add no further finding. This was a source-only review; tests and the reported plan-byte comparison were not rerun. These unresolved round-3 items go to theme2.
