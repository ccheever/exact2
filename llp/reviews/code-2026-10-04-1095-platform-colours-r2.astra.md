# Code review: LLP 1095 (reviewed as 1086/1087), platform colours, round 2 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" --sandbox read-only`, one fresh session in the landing worktree.
- **Date:** 2026-10-04.
- **Commits:** the reviewed SHAs are the branch before it was squashed into one commit for landing; they are not on `origin/main`.
- **Reviewed:** `feat/platform-colours-rebase` at `83834f926` (round 1 folded, rebased onto `origin/main` `3394b5292`).
- **Method:** requested by Charlie. Mutually blind to Grok 4.7, identical brief (round 1's plus a round-2 section; sha256 `9d6196b19bf5f8435be655b9727a49f5bec3ebad2e3493279b24fa1d20029fa5`). Static and read-only. The landing agent (Claude) is not a reviewer.
- **Transcription:** codex's last message (`-o`), unedited.

---

**SHIP WITH FIXES.** Reviewed `83834f926` against `origin/main` at `157c4d741`. Most round-one defects are fixed, but several colour paths still produce incorrect or stale results.

Round-one disposition follows. “Fixed” describes the original finding; related new failures are detailed afterward. **A** identifies Astra’s review; **G** identifies Grok’s.

| Finding | Status | Evidence |
|---|---|---|
| A1: tvOS selects AppKit | Fixed | New UIKit guards include tvOS in [SystemColor.swift:9](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:9) and Affordances. |
| A2: composite wire loses references | Fixed | All seven colour-bearing composite codecs use `.wire()` in [codec.rs:300](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/build/codec.rs:300); composite round-trip coverage was added. |
| A3: only first session refreshes | Fixed for identical reports | [colors.rs:48](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/colors.rs:48) compares the generation against each session’s `colors_seen`. Unequal reports remain problematic: finding 1 below. |
| A4: report relies on incidental main-thread tint lookup | Fixed | [Session.swift:1211](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Session.swift:1211) reads the session view’s tint before submitting the owner-thread report. |
| A5 / G1: unstyled web symbols black | Fixed in source | Both [symbols.js:21](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/web-js/symbols.js:21) and [glue.js:358](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/web/glue.js:358) add the zero-specificity `AccentColor` default. |
| A6: bound composite roles emit invalid CSS | Fixed for gradients/shadows | [style.rs:475](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/web-js/src/style.rs:475) maps embedded role tokens. Newly supported filters miss this path: finding 4. |
| A7: inline references freeze during decoding | Fixed | [BatchShapes.swift:240](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/BatchShapes.swift:240) retains references; `InlineText.run` resolves them using current paragraph traits. |
| A8 / G4: symbol cache ignores contrast/elevation | Fixed | [Affordances.swift:41](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Affordances.swift:41) includes contrast, elevation and system-colour generation. Tint changes remain missing: finding 3. |
| A9: macOS accent cache never invalidates | Fixed | [Session.swift:525](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Session.swift:525) observes system-colour changes, invalidates, reports and reapplies views. |
| A10: direct resolution ignores view contrast | Fixed for the reported UIKit case | [NodeViewIOS.swift:769](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:769) forwards the view’s actual contrast and level. |
| A11: box-filter shadows miss report refresh | Fixed | [colors.rs:63](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/colors.rs:63) includes referenced filter colours and referenced `currentcolor`. |
| A12: explicit filter references refused | Fixed for parsing/wire | [filter.rs:655](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/src/svg/filter.rs:655) accepts `ColorValue`; appearance handling remains defective: finding 5. |
| A13: source cap exceeded | Fixed | `kernel/src/style.rs` is 1,494 lines using caps’ counting. Every changed source file is below 1,500. |
| G2: Linux symbol uses text colour | Fixed | [paint/svg.rs:101](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/linux/src/paint/svg.rs:101) uses `style.tint_color`, including its accent default. |
| G3: stale accent and wrong window tint | Partly fixed | macOS invalidation and scalar UIKit tint resolution are fixed; findings 1–3 cover remaining paths. |
| G5: unsafe non-object Objective-C return | Fixed | [SystemColor.swift:111](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:111) checks the return encoding before `perform`. |
| G6: data admits arbitrary platform selectors | Fixed for runner admission | [bridge.rs:64](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/runner/src/bridge.rs:64) requires a plan string; the compiler adds `lower-platform-color-literal`. |
| G7: resolution failure permanently blacklists | Fixed as proposed | Conversion failures no longer blacklist; [SystemColor.swift:49](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:49) also clears refused lookups on invalidation. |
| G8: first report animates from fallback tint | Partly fixed | Reporting precedes the first scheme when attached, but an off-window boot can establish the scheme first: finding 6. |
| G9: invalid role IDs wrap | Fixed | [abi/colors.rs:47](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/abi/colors.rs:47) validates references; `role_of` uses checked indexing. |
| G10: `fill` alias contradicts §12 | Fixed through documentation | §12 now explicitly retains the alias until LLP 1081’s sweep, matching the schema. |

The remaining and new findings are:

1. **Medium — session-specific tint reports overwrite other sessions’ kernel colours.**  
   [kernel/src/style/roles.rs:83](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/src/style/roles.rs:83), [host/apple/src/colors.rs:48](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/src/colors.rs:48).

   Two embedded sessions can inherit different tints. If A reports red and B subsequently reports green, the process table contains green. An ordinary update that rebuilds A’s `AccentColor` gradient or SVG now resolves green, although A’s directly resolved colours remain red. The generation fix handles repeated **identical** reports; it cannot preserve distinct session resolutions.

   **Fix:** retain resolutions per session and pass that resolution context through kernel paint. Extend the two-session test to use different tints and perform an ordinary update after both reports.

2. **Medium — several Swift paint paths still resolve the key window’s tint instead of the requesting view’s.**  
   [InlineText.swift:61](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/InlineText.swift:61), [BoxShadow.swift:36](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/BoxShadow.swift:36), [Affordances.swift:56](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Affordances.swift:56).

   Put an `ExactView` with red tint inside a blue-tinted window. A plain `text "hello" color="AccentColor"` resolves its run through `InlineText.run` without forwarding tint, falling back to the key window’s blue. That run colour overrides the correctly resolved paragraph colour. Box shadows, text shadow/stroke colours and symbol palettes omit tint similarly.

   **Fix:** forward the owner’s tint through every scalar and composite resolver. Test actual paragraph runs and composite paint, rather than only `node.channels()`.

3. **Medium — palette symbols remain stale after an inherited tint changes.**  
   [Affordances.swift:41](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Affordances.swift:41), [SystemColor.swift:223](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/SystemColor.swift:223).

   A palette containing `AccentColor` is converted into fixed colours. Change the window tint from red to green without changing appearance: `tintColorDidChange` reapplies the style, but `symbolLookKey` remains identical, so `updateSymbol` reuses the red image. The global generation only changes through `invalidate()`, not this tint callback. Untinted hierarchical symbols also fail the callback’s explicit-reference guard.

   **Fix:** invalidate the symbol configuration when inherited tint changes, including implicit accent use, or include resolved tint in its cache key.

4. **Medium — dynamic filters bypass both role conversion and platform-literal conversion on the web JS target.**  
   [host/web-js/src/rows.rs:213](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/web-js/src/rows.rs:213).

   `C::Filter` is absent from the colour-bearing codec list. A bound `filter` switching to `drop-shadow(0px 2px 4px system-orange)` emits that unsupported keyword unchanged. An allowed literal containing `platform-color()` likewise reaches CSS unchanged. The browser drops the filter, while the static/native paths accept it.

   **Fix:** route `Filter` through `color_map(plan)`. Add a test through emitted binding code; testing the mapper alone does not exercise this omission.

5. **Medium — newly accepted box-filter colours always resolve in light appearance.**  
   [kernel/src/svg/scene/filter.rs:173](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/kernel/src/svg/scene/filter.rs:173).

   An ordinary box with `filter="drop-shadow(0px 2px 4px light-dark(#ff0000, #00ff00))"` remains red in dark appearance because the new `ColorValue` is resolved with `false`. Platform references similarly always use their light report. This extends beyond §9’s explicitly deferred **SVG** filter colours to ordinary box filters. The new tests only assert light output or successful parsing.

   **Fix:** resolve box-filter colours using the owning view’s appearance, carrying both variants to Swift if necessary.

6. **Low — off-window boot still allows the startup tint transition identified in G8.**  
   [Session.swift:1200](/Users/admin/.config/tuft/sessions/1791112488.305919/worktrees/exact2/host/apple/Sources/ExactKit/Session.swift:1200).

   Create the `ExactView`, boot its session before attaching it, then attach it to a red-tinted window. `finishBoot → rebooted → reportScheme` establishes the kernel scheme while `reportColors` returns early for lack of a window. On attachment, the first successful report invokes `colors_changed` with an established scheme and transitions from fallback blue. The standalone `EXACT_PLAN` path can follow this ordering.

   **Fix:** treat the first successful colour report as a non-animated correction independently of whether a scheme was already reported. The added off-window test creates the view **after** boot and misses this ordering.

The rebase-sensitive checks otherwise look sound: the digest recomputes to **`0xcea14a1e2bab0343`**; `tellPreferences()` preserves main’s preference call; the ABI module list preserves both sides; and `symbols.js` retains main’s lazy role loading. I found no actionable §12 naming conflict. Production Rust calls share `Owner`’s single thread, and generation increments occur under the table’s write lock; finding 1 is a resolution-context problem, not a demonstrated data race.

This was a read-only static review. I ran no builds, tests, browsers or simulators. Runtime rendering, platform dispatch and notification timing remain unverified. The checkout is unchanged.