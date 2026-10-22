# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `114da001a5`.
- **Method:** one brief (sha256 `0209218f2dc012a8676595fba3851dbf05f02df4a4c447f8e7a7db4c28e1893a`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in b53b3a3389

VERDICT: WRONG

1. **blocker — Authored state surfaces are missed.** [contract/lower/src/popover_surface.rs:28](/tmp/iosfix-rv-ps1/contract/lower/src/popover_surface.rs:28) checks raw attribute names before state folding. Rows such as `:hover background-color` and `:disabled box-shadow`, including class rows, therefore receive `defaultSurface=true`. Glass remains when those authored surfaces apply, violating author precedence and the plan-identity condition. Inspect the underlying property using `state_of`, with regression coverage.

2. **should-fix — Removing `popover` leaves its default surface behind.** [Backdrop.swift:161](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/Backdrop.swift:161) never checks the live `popover` property. For the supported `popover=(expanded ? some("manual") : none)`, the compiler’s constant flag and derived metrics survive `ClearProp`. The restored inline node retains glass, clipping and shadow. Require an active popover property when applying the surface.

3. **should-fix — Zero and nonuniform authored radii are overridden.** [NodeViewIOS.swift:1108](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1108) treats `BorderPaint.clip` returning zero as “no authored radius.” It also returns zero for explicit `border-radius=0` and nonuniform/elliptical corners. These receive the default 30-point configuration. [BoxShadow.swift:224](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/BoxShadow.swift:224) similarly replaces explicit zero radii. Distinguish missing rows from zero values and preserve per-corner geometry.

4. **should-fix — Scrolling content bypasses the default corner.** [NodeViewIOS.swift:1107](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1107) applies the corner only to `materialView`. However, [GlassGroup.swift:225](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/GlassGroup.swift:225) selects the scroll container before the glass content view. A scrolling popover’s children remain outside the rounded material; full-width child backgrounds can paint square corners over it. Ordinary children are rehomed and clipped, but scrolling children need equivalent handling.

5. **should-fix — Role-less menus change under the agent.** [contract/lower/src/popover_surface.rs:24](/tmp/iosfix-rv-ps1/contract/lower/src/popover_surface.rs:24) excludes only literal `role="menu"`; the host guard also checks only the resolved role. Existing context-menu presentation extracts menu items without requiring that role ([ContextMenusIOS.swift:298](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:298)). Such menus now acquire glass and clipping in their agent-painted presentation. Align exclusion with actual menu eligibility. A bound role resolving to `"menu"` is already suppressed correctly at runtime.

6. **should-fix — The pre-26 limitation is insufficiently stated.** [Backdrop.swift:152](/tmp/iosfix-rv-ps1/host/apple/Sources/ExactKit/Backdrop.swift:152) requests glass on every supported iOS version. Below 26, the existing helper substitutes ultra-thin blur while retaining the new 30-point corner and shadow; the comparison test skips those versions. The commit records the iOS 27 measurement but does not explicitly identify this fallback as unmeasured against pre-glass UIKit. State that limitation to satisfy condition (1).

7. **should-fix — Hosted coverage misses the important integration behavior.** [PopoverSurfaceIOSTests.swift:34](/tmp/iosfix-rv-ps1/host/apple/tests/ExactKitTests/PopoverSurfaceIOSTests.swift:34) creates an empty popover and manually unhides it. It cannot detect child clipping or scrolling failures. The “authored surface” case supplies neither a background nor a flag, and the corner/interior sampling never verifies the faint exterior shadow. Add actual presentation with children, an authored background, white-page shadow samples, and an appearance change on the same node.

8. **nit — The Rust test assumes macOS without a target guard.** [popover_surface_tests.rs:42](/tmp/iosfix-rv-ps1/host/apple/src/popover_surface_tests.rs:42) unconditionally expects absent metrics, although `scale::kernel` selects the iOS column on non-macOS targets. Guard the assertion or derive its expectation from the selected column.

Base and conditional classes, component-provided surfaces, transparent backgrounds and inset shadows otherwise look correctly handled. The web’s `data-defaultsurface` attribute is inert; Mac presentation, metric kinds/columns, recycling and trait-driven shadow refresh look sound.

Read-only source review; no builds or tests run. The supplied 48-app identity comparison was not rerun.
