# Code review: the Android filled button as Material 3's, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed (the read-only sandbox cannot start on `expo-build-1000`), a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `d83ae94c8`.
- **Method:** one brief (sha256 `4b80e294ef335a6d3f99db6de9333e2a4ff519e3d31b6e7168fe8bf9935a27b8`), blind to the author's reasoning. The author (Claude) is not a reviewer.
- **Answered:** 1 fixed (a disabled button keeps an authored accent, as before); 2 fixed (`controlSize` scales Material's metrics and type); 3 fixed (an authored corner style wins over the pill); 4 fixed (under the Material look a button's starting font follows the device's font scale, as a hosted field's does); 5 fixed (a subtitle's alpha is a share of the title's); 6 fixed (the Material-look test holds each).

## Review

1. **Correctness — disabled buttons discard authored accents.** [button.rs:617](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/button.rs:617) unconditionally selects the disabled Material container, ignoring `authored_accent`. A filled button with `accent-color="#ff0000" disabled=true` becomes grey. This violates author precedence and contradicts the schema’s existing Android `button-prominent:disabled` claim of exact accent support.

2. **Correctness — control sizes become ineffective.** [button.rs:174](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/button.rs:174) bypasses both `scale` and `said_size`; Material padding and minimum dimensions also ignore them. Otherwise identical filled buttons with `-exact-control-size="mini"` and `"large"` now measure and paint identically.

3. **Correctness — authored corner styles are ignored.** [button.rs:581](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/button.rs:581) chooses a pill before consulting `control_corner_style`. For example, `buttonStyle="filled" -exact-corner-style="small"` paints 20 dp corners at the default height instead of honoring the requested small corners. Explicit `border-radius` still works.

4. **Correctness — the new typography does not follow Android font scale.** [button.rs:77](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/button.rs:77) scales the control’s starting font, which remains 16: [height.rs:104](/home/ccheever/projects/exact2-review-s2/host/linux/src/height.rs:104) updates only field and textarea fonts. Increasing the device font scale therefore leaves an unauthored filled-button title at 14 logical pixels, rather than scaling its 14 sp label.

5. **Correctness — disabled subtitles escape disabled opacity.** [button.rs:799](/home/ccheever/projects/exact2-review-s2/host/linux/src/paint/button.rs:799) replaces the disabled ink’s alpha with `180`, while the new Material branch disables the subsequent fade. A disabled button with two text children paints its title at 38% opacity but its subtitle at approximately 71%, making the secondary text substantially stronger.

6. **Tests — the added assertion misses these failures.** [material_tests.rs:102](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/material_tests.rs:102) checks only height and minimum width. It does not verify button paint, exact padding, overrides, sizes, subtitles, or font scaling. The existing authored-disabled-accent test selects no Material look, so it cannot catch finding 1. I found no existing assertion rendered false under the look its test actually selects.

Metric-cell isolation: **no findings**. The new rows leave every non-Material cell unset, and their consumers explicitly select Material.

Other: **no findings**.

Static review only; builds and tests were not run because they can write files.

NOT READY