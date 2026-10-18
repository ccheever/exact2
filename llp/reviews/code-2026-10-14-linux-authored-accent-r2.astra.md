# Code review: Linux: the authored accent, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `1a4a48c90cf1b5b6656257d457e29f39ca99238ac7092c37427681b6df8d844e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, three low; Grok: four medium, one low.
- **Disposition:** fixed in round 2, motion reverted; the rest named in QUEUE items (1)-(6)

---

No high-severity findings. Beyond the three acknowledged gaps, I found seven medium and three low issues.

1. **Medium — Self-referential accents still fail outside a recursive paint walk.** [roles.rs:434](/tmp/rv-accent2/kernel/src/style/roles.rs:434), [paint.rs:1328](/tmp/rv-accent2/host/linux/src/paint.rs:1328).  
   Under a green ancestor and an intervening `accent-color="AccentColor"`, `computed_row` returns the role; `authored_accent` then preserves the ambient scope. Independent calls from motion, hosted fields, or region capture have no enclosing green scope, so they use the app tint. Resolve the effective accent through logical ancestry before opening the scope.

2. **Medium — Shared motion now applies the web’s tint rules on macOS.** [motion.rs:328](/tmp/rv-accent2/kernel/src/motion.rs:328).  
   `authored_accent` uses `web_accent`, which includes `LinkText`, `-exact-link`, and `-exact-symbol`. On macOS those resolve to AppKit colors, not the authored accent. A transition therefore targets the accent and snaps to the AppKit color when the authored row resumes painting. Pass the host’s tint-resolution policy into shared motion.

3. **Medium — Accent substitution bypasses wide-color and profile handling.** [motion.rs:262](/tmp/rv-accent2/kernel/src/motion.rs:262).  
   An `AccentColor` backed by `accent-color="color(display-p3 1 0 0)"` reaches `color()` as `Role`. It misses the `Wide` branch and resolves to clipped RGBA8 instead of Oklab; a profiled accent likewise misses the discrete-color branch. Substitute the effective `ColorValue` before checking its kind.

4. **Medium — Inline shadows and strokes use the paragraph’s accent.** [text_shadow.rs:45](/tmp/rv-accent2/host/linux/src/paint/text_shadow.rs:45).  
   `run_looks` resolves each run’s explicit shadow/stroke color without opening that run’s scope. A green inline run inside a red paragraph can now have green glyphs but red `AccentColor` shadows or strokes. Open the run’s effective accent scope inside the mapping closure.

5. **Medium — Native button face parts ignore their own accents.** [button.rs:660](/tmp/rv-accent2/host/linux/src/paint/button.rs:660).  
   Title/subtitle colors and symbol tint resolve under the button’s scope. Its children are excluded from the normal painter walk, so a child’s authored accent never takes effect. For example, a green title using `color="AccentColor"` inside a red native button remains red. Resolve each face part using its source child’s effective accent.

6. **Medium — Retained-region image tint remains outside the scope.** [region.rs:282](/tmp/rv-accent2/host/linux/src/paint/region.rs:282).  
   The new guard closes immediately after `BoxPaint::capture`; `image_tint` runs afterward. A template image using `-exact-tint-color="AccentColor"` therefore captures the app tint despite a local authored accent. Extend the node’s scope through payload construction.

7. **Medium — The Android window background disagrees with the painted root.** [window.rs:203](/tmp/rv-accent2/host/linux/src/presenter/window.rs:203).  
   `chrome()` resolves the root background without its accent scope. A root with green `accent-color` and `background-color="AccentColor"` paints green, while the window/inset background receives the app tint. Keep the root node available and resolve its background inside its scope.

8. **Low — The new region call triggers Clippy.** [source.rs:144](/tmp/rv-accent2/host/linux/src/content_region/source.rs:144).  
   `node` is already `&NodeRef`; `accent_of(&node)` triggers `clippy::needless_borrow`, failing workspace lint under `-D warnings`. Pass `node`. I confirmed this lint with the pinned 1.97.0 Clippy driver using an stdin reproducer.

9. **Low — The added `kept` case is never asserted.** [accent.rs:22](/tmp/rv-accent2/host/linux/tests/it/accent.rs:22).  
   Removing the self-reference fix would not fail this test. Assert `kept` is green, and cover an independent motion/region call; the current test exercises none of the newly scoped entry points.

10. **Low — The diff reverses unrelated Tamber documentation.** [1121-tamber.rfc.md:4](/tmp/rv-accent2/llp/1121-tamber.rfc.md:4).  
    It marks commit 7’s CLI/docs work pending and removes its implementation record, although that implementation remains in HEAD. Restore the accurate status and section.

The three acknowledged gaps remain, **medium severity each**:

- [paint/svg.rs:314](/tmp/rv-accent2/host/linux/src/paint/svg.rs:314): SVG descendants still paint under the outer SVG’s accent. Substitute colors using each item’s effective accent during scene resolution.
- [paint/region.rs:763](/tmp/rv-accent2/host/linux/src/paint/region.rs:763): retained SVG and gradient replay still resolves roles without the captured node’s accent. Preserve substituted colors or captured accent metadata; merely widening the capture-time scope will not fix replay.
- [roles.rs:496](/tmp/rv-accent2/kernel/src/style/roles.rs:496): an alias such as `CanvasText = LinkText`, when the theme also supplies `AccentColor`, can become the theme’s concrete tint before local substitution. Preserve the alias until authored-accent resolution.

`caps`, `cargo fmt --all -- --check`, and `git diff --check` passed. The thread-bound guard is sound. I found no new wasm execution path through `color_targets`; web paint transitions remain browser-owned. Performance was not measured, and runtime tests were not run because this read-only worktree has no build artifacts. No files changed.
