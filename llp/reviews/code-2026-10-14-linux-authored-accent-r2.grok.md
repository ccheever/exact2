# Code review: Linux: the authored accent, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `1a4a48c90cf1b5b6656257d457e29f39ca99238ac7092c37427681b6df8d844e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, three low; Grok: four medium, one low.
- **Disposition:** fixed in round 2, motion reverted; the rest named in QUEUE items (1)-(6)

---

## Medium

**[medium] A self-referential accent sticks only while an ancestor scope is already on the thread — `kernel/src/style/roles.rs:434`**

`authored_accent` treats `accent-color="AccentColor"` (and any other `web_accent` role) by keeping whatever is already in the thread-local, rather than walking to the nearest real accent. That works in the live walk because the ancestor's guard is still on the stack. It fails everywhere this commit opens a scope with no ancestor guard: `color_targets` (`kernel/src/motion.rs:328`), content-region box capture and `accented()` (`host/linux/src/content_region/source.rs:142`), a kept row's picture, a hosted field, and a bar button. A column with `accent-color="#00c800"` containing `accent-color="AccentColor"` paints green in the live walk and the app tint in those other paths, so a colour transition and a retained region disagree with the pixels. Walk ancestors for the nearest accent that is not a `web_accent` role, and use that value, instead of reading the thread-local.

**[medium] `accent-color="LinkText"` keeps the ancestor, which neither the web nor the Mac does — `kernel/src/style/roles.rs:434`**

The comment describes `accent-color="AccentColor"`. The predicate is every `web_accent` role, so `LinkText`, `-exact-link`, and `-exact-symbol` keep the ancestor too. On the web, `css.rs` suppresses the `--exact-accent` redeclaration only when the declared value is exactly `var(--exact-accent, AccentColor)`. `LinkText` is `var(--exact-linktext, var(--exact-accent, AccentColor))`, which redeclares `--exact-accent` into a cycle; that custom property becomes guaranteed-invalid, and `var(--exact-accent, AccentColor)` falls through to the system `AccentColor` keyword. On the Mac, `LinkText` is `linkColor`, an authored tint. Keep the outer accent only for the `AccentColor` role.

**[medium] A retained region's image tint is resolved after its scope ends — `host/linux/src/paint/region.rs:282`**

The new guard wraps only `BoxPaint::capture` and drops at line 257. `image_tint` runs after that, so an image with `-exact-tint-color="AccentColor"` under `accent-color="#c80000"` bakes the app tint. The same image in the live walk, and in a kept row, is green. Hold the scope across the image payload (the SVG scene is a separate, later resolve).

**[medium] A run's shadow and stroke ignore the run accent `text_palette` just applied — `host/linux/src/paint/text_shadow.rs:50`**

`text_palette` opens a scope per run, then returns. `run_looks` resolves an explicit `text-shadow` or `text-stroke` colour afterwards, under the paragraph's scope only. A span with `accent-color="#c80000"`, `color="AccentColor"`, and `text-shadow="0 1px AccentColor"` draws red glyphs and a shadow in the paragraph's accent (or the app tint). An omitted shadow colour already uses `run.color` and is fine. Open the run's scope inside `run_looks` before those two `resolve` calls (the stroke at line 57 is the same hole).

## Low

**[low] The new `accent-color="AccentColor"` case is never sampled — `host/linux/tests/it/accent.rs:22`**

The commit adds `testId="kept"` under `accent-color="#00c800"` / `accent-color="AccentColor"` and does not assert it. The test stays green if that box paints the app tint or the inner red. Assert `at(&p, "kept", 20) == [0, 200, 0, 255]`.

## None

No high-severity findings. Nothing in the hot path is worth a performance finding: the scope is a thread-local `Cell` replace per node, and `painted_accent` returns before touching it unless the value is a `web_accent` role.

Caps: nothing is over 1,500 lines. `kernel/src/style.rs` is 1,497 after the three lines added in `29468db5d`.

Apple and wasm: the web does not call `paint_sync`, and nothing in `host/web` or `host/web-js` opens a scope, so wasm colour resolution is unchanged. Apple does not call `paint_sync` either. `host/apple/src/svg_lower.rs` uses `color_targets` only for alpha, to choose sampled motion versus Core Animation. An authored accent whose alpha differs from the role's can flip that choice; opaque accents do not, and Apple still paints the tint through `@tint` rather than through this RGB.

## Still remaining

These are not fixed, and none of them is a few lines.

- An SVG descendant that authors its own accent still paints under the `svg` node's scope. `node()` returns before opening a scope for `is_svg_element()` (`host/linux/src/paint.rs:883`), and `svg_item` never opens one.
- A retained region's replayed SVG and gradients still resolve outside any scope. Replay returns before `node()`'s guard (`host/linux/src/paint.rs:870`). Gradient stops resolve in `Gradient::resolved` at paint time (`kernel/src/gradient.rs:425`), not in `BoxPaint::capture`. `resolve_svg` only builds the scene; `ink` resolves fill, stroke, and gradient stops at replay (`host/linux/src/paint/svg.rs:236`).
- A theme alias such as `CanvasText = "LinkText"` still misses the scope when it matters. Inside a live scope, `LinkText` does reach `painted_accent` on the recursive `resolve` if `AccentColor` itself is not overridden. `painted_override` (`kernel/src/style/roles.rs:496`) collapses "replaced by a tint role" to the theme's `AccentColor` value when that override exists, so the authored accent never runs. `accented()` only substitutes a value that is already a `web_accent` role, so a region palette stores `CanvasText` and resolves it later with no scope.
