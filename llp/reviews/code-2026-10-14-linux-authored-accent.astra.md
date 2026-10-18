# Code review: Linux: a role drawn in the accent reads the nearest authored accent-color, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `da4e7b31690b85f2d8271efc9568c51cadfc812627f2b1f5393793efa364ec41`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: eight medium, one low; Grok: one high, two medium, three low.
- **Disposition:** the scope opened at the other paths in round 1; SVG descendants, region replay and theme aliases named

---

No high-severity findings. I found eight medium-severity issues and one low-severity issue.

1. **Medium — Motion resolves colours before the scope exists.** [kernel/src/motion.rs:326](/tmp/rv-accent/kernel/src/motion.rs:326), [host/linux/src/paint.rs:896](/tmp/rv-accent/host/linux/src/paint.rs:896)  
   `color_targets()` resolves transition endpoints outside painting. Under a green authored accent, a transition from a literal colour to `AccentColor` therefore interpolates toward the app tint, then snaps to green when the presented override retires. Fix by supplying the node’s effective accent when computing motion targets, using the same resolution policy as painting.

2. **Medium — Inline runs use the paragraph’s accent.** [host/linux/src/paint/inline.rs:21](/tmp/rv-accent/host/linux/src/paint/inline.rs:21), [host/linux/src/paint/text_shadow.rs:45](/tmp/rv-accent/host/linux/src/paint/text_shadow.rs:45)  
   Inline nodes return before the new scope. Their palettes, backgrounds, shadows and strokes resolve inside the paragraph owner’s scope. A red-accented span naming `AccentColor` inside a green-accented paragraph paints green. Fix by establishing the effective accent separately for each inline node whose paint is resolved.

3. **Medium — SVG descendants use the outer SVG’s accent.** [host/linux/src/paint/svg.rs:440](/tmp/rv-accent/host/linux/src/paint/svg.rs:440)  
   SVG elements also bypass `node()`. `svg_item()` and `ink()` never establish their own accent context, so a rectangle with its own red accent under a green SVG paints green. Fix by carrying effective accent context through the resolved scene, including text, paint servers and instantiated content; resolving from an item’s ordinary ancestry alone would mishandle `<use>` instances.

4. **Medium — Image-slot refreshes record the surrounding scroller’s tint.** [host/linux/src/paint/rows.rs:444](/tmp/rv-accent/host/linux/src/paint/rows.rs:444)  
   `pictures_again()` calls `picture()` directly while replaying a retained row. An image naming `AccentColor` beneath a row-specific accent consequently uses the scroller’s scope during a slot refresh, although a full row recording uses the image’s correct accent. The wrong tint remains in the recorded slot. Fix by opening the image node’s effective scope before repainting its slot. This affects Android’s recording backend.

5. **Medium — Retained content does not capture its accent context.** [host/linux/src/paint/region.rs:254](/tmp/rv-accent/host/linux/src/paint/region.rs:254), [host/linux/src/content_region/source.rs:142](/tmp/rv-accent/host/linux/src/content_region/source.rs:142)  
   Region box colours and text palettes are captured outside `node()`, so they still resolve to the app tint. Conversely, gradients and SVG colours resolved during replay can read the surrounding live scope; the snapshot stores no accent context for them. Fix by capturing each node/run’s effective accent with the publication and using that captured context during replay and appearance changes, without consulting potentially retired nodes.

6. **Medium — `accent-color="AccentColor"` incorrectly resets inheritance.** [kernel/src/style/roles.rs:450](/tmp/rv-accent/kernel/src/style/roles.rs:450)  
   Clearing the scope before resolving the accent makes this declaration reset a child to the app tint. Under a red ancestor, Apple preserves red, and [css.rs:343](/tmp/rv-accent/host/web/src/css.rs:343) deliberately leaves `--exact-accent` inherited. Fix by resolving this self-reference against the ancestor’s effective tint instead of unconditionally removing all authored context.

7. **Medium — Theme aliases can bypass the authored accent.** [kernel/src/style/roles.rs:469](/tmp/rv-accent/kernel/src/style/roles.rs:469)  
   With `CanvasText = "LinkText"` and a red theme `AccentColor`, default text under a green authored accent remains red, while explicitly authored `LinkText` now becomes green. The initial accent check rejects `CanvasText`, then `painted_override()` replaces its tint alias with the theme’s fixed colour before another scoped resolution occurs. Fix by preserving the tint reference through override resolution so the authored accent gets precedence.

8. **Medium — Android’s hosted fields and bar buttons miss the scope.** [host/linux/src/presenter/hosted.rs:383](/tmp/rv-accent/host/linux/src/presenter/hosted.rs:383), [host/linux/src/presenter/bars.rs:235](/tmp/rv-accent/host/linux/src/presenter/bars.rs:235)  
   These paths serialize colours outside the painter. Explicit `AccentColor` text, placeholder and caret colours still use the app tint, while surrounding painted content and focus rings use the authored accent. Explicit bar-button colours have the same problem. Fix by establishing the corresponding node’s effective accent during serialization.

9. **Low — `AccentScope` can be moved to another thread.** [kernel/src/style/roles.rs:433](/tmp/rv-accent/kernel/src/style/roles.rs:433)  
   The guard automatically implements `Send` and `Sync`. Moving it to another thread and dropping it leaves the originating thread’s accent installed and overwrites the receiving thread’s context. Make it thread-bound with `PhantomData<Rc<()>>`, as [ThemeScope already is](/tmp/rv-accent/kernel/src/style/theme.rs:66). Ordinary same-thread nesting and panic unwinding restore correctly.

The new test covers static boxes, a border and basic nesting; it misses the cases above. Those need regression coverage, alongside dark-scheme and theme-override cases.

I found no additional issue in ordinary text shaping caches, inherited-change invalidation, placed-node re-entry, ghosts, bitmap overlays or ordinary focus-ring/select-menu handling. Apple’s Rust side, terminal and web/wasm open no accent scope and retain their previous resolution behaviour. Android and Windows share the Linux painter.

The accent check adds no allocation or lock itself; its runtime cost and the additional ancestor lookup per painted node remain unmeasured. All changed source files satisfy the cap: `style.rs` counts as **1,498 lines** under `caps.mjs`.

Read-only source review only; no edits, builds or tests.
