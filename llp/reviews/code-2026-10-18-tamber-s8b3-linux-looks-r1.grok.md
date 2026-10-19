# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `23fd7820f`, the Linux host and Android through it.
- **Method:** one brief (sha256 `51a6d08d7ab0d59233af23cf646235317791fc524c38a175d104d527d064d25e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (blocking: resting-pointer hover looks, look and press opacity written into the cache; should-fix: Android's lowered opacity track; nits: focus membership, tests).
- **Disposition:** in the round-1 commit, as Astra's 2, 1 and 3; 4: a focus marks only with a `:focus-visible` look; 5: a test of a transition under a hover and of the agent's hover added (scrolling under a resting mouse goes through the same `hover_at`).

---

**WRONG**

The still-node arithmetic matches D8: `press_opacity_to` eases from the opacity on screen (a hover or focus look included) toward an absolute target, a hand-written `press_opacity` stays a share, focus paints over hover, and the press background paints over both. A disabled node drops the looks. Two host bugs still show the wrong hover and a stuck opacity, and Android’s lowered opacity track drops the new overlay for the whole animation.

1. **BLOCKING** — Resting-pointer `:hover` looks stay on the nodes from the last move. `host/linux/src/presenter/feedback.rs:80`

Every paint calls `refresh_looks(None)` (`host/linux/src/content_region/presenter.rs:83`). That branch copies the previous `looks_shown` ids for every node that still exists. It skips a hit test of `hover_point`, so a scroll, a layout move, a row renewed under the cursor, or a `SetLooks` that adds `:hover` while the pointer rests keeps the old look and leaves the node now under the pointer without one. `follow_pointer` (`host/linux/src/presenter/events.rs:514`) updates hover handlers after the frame and returns. The focus half already re-reads `self.focus` every frame; the hover half is the one that sticks. This host’s handler path already follows content under a resting pointer (#139). `:hover` looks should use that same point.

Fix: on the per-frame path, if `hover_point` is a mouse point, rebuild the hover set from `hit` at that point and the ancestors that carry `LOOK_HOVER`; if the pointer is absent or direct, clear the hover set. Keep the focus half as written. Hit-testing here only updates looks; leave event dispatch in `follow_pointer`.

2. **BLOCKING** — Look and press opacity are written into the presentation map. `host/linux/src/host.rs:1434`

`Host::presented` (`host/linux/src/host.rs:663`) copies the map, or the style, and `press::present` (`host/linux/src/press.rs:72`) writes the look’s opacity and the press lerp into that same `opacity` field. `Host::present` and `paint_over` (`host/linux/src/paint_motion.rs:175`) insert that struct the first time a sample arrives (`or_insert`). A later `Opacity` sample overwrites the field. A scale, translate, or color sample leaves it. While the look still holds, the next read replaces opacity again and the picture matches. After the pointer leaves, the map still holds the look’s opacity, so a card hovered at 0.8 stays at 0.8 until some later opacity sample. A `press_opacity_to` insert at partial progress is the same store: the next read lerps from the already-lerped value, so the fade leaves the press curve, and progress 0 leaves the baked value on screen. A pure hover or press with no other sample stays correct, which is why `looks_tests.rs` stays green.

Fix: keep the map as engine and style samples only. Apply `press::present` on the paint and hit read, after the copy, and do not write that result back. Both seed sites should insert the style (or the existing map entry) plus the one property being sampled.

3. **SHOULD-FIX** — A lowered opacity track replaces the new overlay. `host/linux/src/paint/layer.rs:84`

With `EXACT_PAINTER=canvas` (Android’s default; `host/linux/src/lower.rs:96`), an opacity transition is played by the reader. `box_layer` stores the composed `p.opacity` as the layer base, and the painter records no group opacity when the layer owns alpha (`host/linux/src/paint.rs:1015`). The reader copies that base and then replaces alpha with the track (`host/android/canvas/src/main/java/dev/exact/canvas/Layers.kt:309`). For the whole animation the hover opacity and `press_opacity_to` are gone, and the implicit underlying (`host/linux/src/lower.rs:322`) can be the composed value. A node with no opacity track is fine: its composed opacity is an opacity op Android replays. The press scale in `base[2]` already has this shape for a scale track; the new rows are the opacity ones.

Fix: while a look names opacity or a press-opacity row is active, keep opacity on the engine so it samples the in-flight base, and record the composed opacity the CPU painter already pushes. Android replays that op. When the overlay is off, leave lowering as it is.

4. **NIT** — Focus membership treats any look as `:focus-visible`. `host/linux/src/presenter/feedback.rs:97`

`refresh_looks` keeps the focus id when `looks()` is non-empty. A node whose only look is `:hover` is marked focus-visible and dirties its row, then paints nothing for that state, because `press::present` only applies a focus look that exists. Require a `LOOK_FOCUS_VISIBLE` entry, the same way hover requires `LOOK_HOVER`.

5. **NIT** — The new pixel tests stay on one still mouse. `host/linux/src/presenter/looks_tests.rs:59`

The pixels they assert are the ones they compute: yellow at 0.8, blue at absolute 0.5 with the hover border under it, the lift back to the hover, a child still hovering the card, a disabled box staying green, a finger leaving no hover. They never scroll under a resting mouse, never focus, and never start a color or scale transition during the hover, so findings 1–3 stay green. Add those three beside the existing checks.
