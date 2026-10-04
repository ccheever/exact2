# Code review: LLP 1095 (reviewed as 1091), platform colours, round 3 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" --sandbox read-only`, one fresh session in the landing worktree.
- **Date:** 2026-10-04.
- **Commits:** the reviewed SHA is the squashed commit before its final rebase; it is not on `origin/main`.
- **Reviewed:** the squashed commit `65ba2e12b` over `3d76ccdb7`.
- **Method:** requested by Charlie. Mutually blind, identical brief (rounds 1-2 plus a round-3 section; sha256 `5c659fa0be777462d574ffdcbc241a47e925bd2b210a80fa83ccc91b7961b742`). Static and read-only. The landing agent (Claude) is not a reviewer.
- **Transcription:** codex's last message (`-o`), unedited.

---

**SHIP WITH FIXES.** Reviewed `65ba2e12b` against its parent, `3d76ccdb7`, as requested. Four new Medium issues remain.

A/G below identify the round-2 Astra/Grok findings. “Fixed” means verified in source, not by running tests.

| Round-2 finding | Disposition and evidence |
|---|---|
| A1 / G1: sessions overwrite one another’s tint | **Fixed under §9’s revised app-wide policy.** [Session.swift:1217](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Session.swift:1217) reports `appTint`; [roles.rs:121](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/src/style/roles.rs:121) preserves omitted tint roles. The repaint loop rechecks the generation afterward. |
| A2 / G3: Swift paint paths omit view tint | **Fixed for the reported paths.** [NodeText.swift:93](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/NodeText.swift:93) forwards tint through plain text, inline runs and text paint; box shadows and palettes also forward it. |
| A3 / G3: symbols remain stale after tint changes | **Partially fixed.** Palette `AccentColor` and implicit hierarchical tint now affect the key. The new key logic misses explicit hierarchical `Highlight`; finding 3 below. |
| A4 / G4: bound web filters bypass colour mapping | **Fixed.** [rows.rs:253](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/web-js/src/rows.rs:253) includes `Filter`; the added test checks the emitted binding wrapper. |
| A5: box-filter colours always use light appearance | **Resolution fixed; refresh incomplete.** Both programs cross the wire, and [BoxFilter.swift:50](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/BoxFilter.swift:50) selects by appearance. Finding 2 covers the new macOS refresh omission. |
| A6: off-window startup colour transition | **Fixed.** [colors.rs:58](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/colors.rs:58) detects the first report independently of scheme state and passes the non-animated correction flag through paint resynchronization. |
| G2: in-flight resolution refills an invalidated cache | **Fixed.** [SystemColor.swift:86](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:86) stores only when the captured generation still matches. |
| G5: rebase conflicts and LLP-number collision | **Fixed in source.** Both Apple presenters call `reappear`; the schema note retains viewport kinds and role defaults; Contract’s error retains both vocabularies; the ABI retains both modules. LLP 1091 is used. The digest independently recomputes to **`0xe4afd57b5de6994a`**, matching the snapshot. |

New findings:

1. **Medium — platform references first selected after boot are never reported.**  
   [kernel/src/style/roles.rs:149](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/src/style/roles.rs:149), [Session.swift:1209](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Session.swift:1209).

   In a fresh native process, start with `background-image="none"`, then select an allowed literal such as `linear-gradient(platform-color(ios systemPinkColor, #010203), #fff)`. The initial report includes only already-interned platform references. Selecting this branch interns a new reference, but an ordinary update triggers no report. Its gradient therefore uses `#010203` until an appearance, preference or tint event causes another report. An existing report for the `system-pink` **role** does not resolve the separate platform-reference ID.

   **Fix:** register all admitted colour literals, including inactive branches and composite values, before the first report, or request resolution when new references appear. Test with separately compiled plan bytes: [colors_tests.rs:44](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/colors_tests.rs:44) compiles in the host’s process, which pre-populates the global interning table and can hide this bug.

2. **Medium — macOS does not recognize `pd` as appearance-dependent paint.**  
   [NodeViewMac.swift:743](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:743), [Gradient.swift:266](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Gradient.swift:266).

   Give a box fixed text/background colours and `filter="drop-shadow(0px 2px 4px light-dark(#ff0000, #00ff00))"`. Change that subtree’s AppKit appearance to dark while the session’s appearance stays light. The filter contains numeric `p`/`pd` programs, but `hasSchemeColor` recognizes neither `pd` nor anything inside those flat arrays. The appearance callback returns without re-rendering the filter, leaving the red shadow until another batch arrives.

   **Fix:** recognize `pd` in the appearance-dependency predicate, or explicitly re-render appearance-dependent box filters from the callback.

3. **Medium — the new symbol key excludes hierarchical `Highlight` tint.**  
   [Affordances.swift:50](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Affordances.swift:50).

   A hierarchical symbol with `tint-color="Highlight"` resolves `@tint/0.2` into a concrete `symbolTint`. Consequently, `symbolTint == nil` is false and `bakedTintKey` is empty. Changing inherited tint from red to green reapplies the style, but leaves the image key unchanged, so its hierarchical configuration retains red.

   **Fix:** include inherited tint in the key whenever the hierarchical `tint_color` names tint, including `Highlight`, as well as when tint is implicit.

4. **Medium — direct tint resolution discards the view’s elevated level.**  
   [SystemColor.swift:67](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:67).

   The newly forwarded view tint reaches `channels`, but its `rgba` call omits `elevated`, defaulting to base level. An inherited dynamic `UIColor` that returns different colours for base and elevated presentations therefore paints `AccentColor`/`Highlight` incorrectly in a sheet—even though callers pass `drawsElevated`. This affects direct Swift paint, outside §9’s declared kernel limitation.

   **Fix:** forward `elevated` through the direct `@tint` resolution path.

The checkout remains unchanged. I ran no builds, tests, browsers or simulators; compilation, actual pixels, Objective-C dispatch and notification timing remain unverified.