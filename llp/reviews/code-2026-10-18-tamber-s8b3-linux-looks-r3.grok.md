# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `9760771d8..57f577fd1`, the commit and its rounds 1 and 2.
- **Method:** one brief (sha256 `ca9635e1545f642d2e6227781a303360dc6fc5473361e1ece00f5401f475c767`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: a sampled animation shipped as a reader track; should-fix: a currentcolor look under a lowered colour transition; nits: inset shading, coverage). QUEUE entries judged acceptable..
- **Disposition:** post-round fixes. 1: an engine-sampled animation is never also a reader track (`layer_tracks`). 2: as Astra's 2. 4: the header narrowed to the CPU painter, a focus pixel added. Declined: 3, an inset border's shading of a look colour (a nit; solid borders, the design's, are right).

---

**RIGHT WITH CHANGES**

Linux composes base, then `:hover`, then `:focus-visible`, then the press, and the round-2 fixes for the cache, clears, the disabled hold, and a lowered scale with no opacity track all hold. Android still plays a force-sampled animation on a layer that a sibling transition keeps open, so that opacity is applied twice.

## Findings

1. **BLOCKING** — A force-sampled opacity animation is still shipped to the Canvas reader. `host/linux/src/lower.rs:318` (caused by `host/linux/src/lower.rs:539` and `host/linux/src/press.rs:47`).

   `paints_over` makes `playable` fail for the whole animation set, and `set_node_sampled(true)` follows (`lower.rs:138`). A scale or translate transition on that same node still lowers: `play_transitions` asks `playable` about that one property, so `lowered_mask` stays `LOWER_TRANSFORM`. `box_layer` then stores alpha `1` (`host/linux/src/paint/layer.rs:86`) and the painter records the engine sample, look included, with `push_opacity` (`host/linux/src/paint.rs:1015`). `layer_tracks` still walks `animation_plays`, which ignores `node_sampled` (`motion/src/engine/animate.rs:227`), and encodes that opacity animation. The implicit underlying is `base.opacity`, and `host/linux/src/canvas.rs:1344` copied that from the layer alpha of `1`. `Layers.sample` writes the track onto `RenderNode.alpha` every frame (`host/android/canvas/src/main/java/dev/exact/canvas/Layers.kt:317`). The recorded group already carries the opacity, and the layer alpha carries it again. While the look holds, the product is look opacity times the animation. The same loop emits a sampled translate, scale, or rotate, with the implicit keyframe taken from the current sample stored in the layer base.

   Fix: in `layer_tracks`, when `self.engine.node_sampled(node_u64(key))` is set, skip the `animation_plays` loop and still emit `self.played`. The engine already samples those animations into the recording and into the layer base on each repaint. This is a different hole from the queued “track already playing when the look arrives” case: here the look is present at handover, the animation is sampled from the start, and the track is emitted anyway.

2. **SHOULD-FIX** — On a text node, a lowered `color` transition leaves a look’s `currentcolor` background or border at the target color. `host/linux/src/press.rs:52`, `host/linux/src/lower.rs:441`, `host/linux/src/paint/presented.rs:246`.

   `recolors` allows `Property::Color` for a childless `Text` node. `paints_over(Color)` is true only for `PressColor` or a look that names `TextColor`. A look background or border stored as `currentcolor` is the mask bit set and the value `None` (`kernel/src/style/tests.rs` around the `BackgroundColor` currentcolor case), so it does not count. The transition is handed to the reader, which presents the target and recolors glyphs in place (`Layers.kt:332`). `looked` bakes `current` from `shown.colors` at record time, so the wash or border snaps to the end color while the glyphs ease. A box is fine: its own `color` is not lowered, and round 2’s in-flight read is what `states` uses.

   Fix: in `paints_over(Property::Color)`, also return true when any look has `BackgroundColor` or a border-side color with the mask bit set and the value `None`. The transition stays on the engine, and the glyphs and the look both follow the sample.

3. **NIT** — A look’s border color skips inset shading. `host/linux/src/paint/presented.rs:282`.

   `border_colors` shades an `Inset` side (`kernel/src/style/border.rs:52`). `looked` writes the flat `rgba` of the look color. `:hover { border-color: red }` on `border-style: inset` paints an unshaded red. Solid borders, which the design example and the tests use, are unchanged.

   Fix: resolve the look color, then shade it with the node’s border style for that side, using the same shade `border_colors` uses. A look cannot name `border-style`, so the node’s style is the one that applies.

4. **NIT** — The look tests still do not cover what the header claims. `host/linux/src/presenter/looks_tests.rs:1` and `:40`.

   The header says Android paints what this recording shows. `boot` uses `PainterChoice::Cpu`, so the suite never builds a layer and cannot see finding 1. There is no `:focus-visible` pixel, no `currentcolor` pixel, and no assertion that a sampled animation is absent from `layer_tracks`. The eased-in check at line 86 only requires a red channel between 127 and 250. The clear test matches its own claim (a hover `background-color: unset` becomes page white).

   Fix: narrow the header to the CPU recording, add a `:focus-visible` pixel, and add a test that `layer_tracks` omits `animation_plays` when `node_sampled` is set and a scale transition is in `self.played`.

## Earlier rounds

Round 1 is fixed. The presentation cache is seeded from `unpresented` and `press::present` runs on the read (`host/linux/src/host.rs:663`). Resting hover goes through `hover_at` → `refresh_looks(Some(hit))`, and `follow_pointer` does the same when no button is down (`host/linux/src/presenter/events.rs:517`). The one-frame lag matches `#139`. Focus membership requires `LOOK_FOCUS_VISIBLE` and a ring (`host/linux/src/presenter/feedback.rs:96`). A lone opacity track is no longer lowered when a look or a press row names opacity.

Round 2 is fixed except where finding 1 and finding 2 say otherwise. Layer alpha is `1` unless the reader owns opacity, which removes the double fade for a scale transition with no opacity animation. A held contact refreshes looks from `hit()` without hover events (`events.rs:519`). Clears use `mask.union(written)`: unset background is transparent, unset border is `currentcolor`, unset shadow is none, unset opacity is `1`. The engine-sampled `currentcolor` reads `shown.colors` first (`presented.rs:246`). A disabled node forces press progress to `0`, so scale, press paint, and press opacity drop at once (`press.rs:98`). Shown opacity is clamped before the absolute lerp and before the D7 share (`press.rs:99`).

Both QUEUE entries are acceptable. The “look or press row arrives while a delegated opacity or colour track already plays” note matches the code: `play_transitions` keeps an entry that `is_played` until it ends, and `paints_over` is consulted only for a new handover. Reclaiming at the curve position, as a hold does, is the right later fix. It is a different case from finding 1. The inline-run note is also right: the paint walk has no box for a run, Apple drops the run’s looks, and the web shows them. Leaving refuse-versus-carry to theme2 matches the Apple B2 deferral.

No caps-driven move changes behavior. `press::scale` is the only caller of that math, from `present`.
