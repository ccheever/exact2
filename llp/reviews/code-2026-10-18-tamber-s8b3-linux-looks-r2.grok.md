# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `23fd7820f` and its round-1 fixes `feeca37c0` (now `2a2fdabc1`, `29023ccd8`; same tree).
- **Method:** one brief (sha256 `776daeb72681500d758410e928ae35de65443c7b0f879538c80cab858dadd1cd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (should-fix: a lowered track doubling a look's opacity on Android, cleared looks keeping the base; nits: currentcolor in flight, test coverage).
- **Disposition:** in the round-2 commit, as Astra's 1, 4 and 5; 4: a cleared-look pixel test added. Not added: a Canvas-reader assertion (no recorder harness for layer alpha here; the change is one line, reasoned in the commit).

---

**RIGHT WITH CHANGES.** The Linux painter composes the design correctly: hover then focus-visible at once, press over both, `press_opacity_to` an absolute lerp from the opacity that is showing, `press_opacity` a share of that, and neither value written back into the presentation cache. Two holes remain on the path Android actually replays, plus a cleared-look mismatch with Apple and CSS.

Round 1, checked against `feeca37c0`:

- Cache stick (Astra 1, grok 2) is fixed. `Host::present` seeds `unpresented` and writes engine samples only (`host.rs:1434-1446`); `presented` overlays looks and press on a copy (`host.rs:663-684`). `a_looks_opacity_is_never_kept` really fails on the old `or_insert` (the second tap starts the scale transition while the hover is up).
- Resting-pointer hover (Astra 2, grok 1) is fixed. `hover_at` calls `refresh_looks(Some(hit))` (`events.rs:468`), and `follow_pointer` calls `hover_at` after a painted frame. The one-frame delay is the same #139 contract as hover handlers.
- Focus membership (grok 4) is fixed. Focus requires `LOOK_FOCUS_VISIBLE` and a ring that would show (`feedback.rs:96-103`).
- Android opacity *track* (Astra 3, grok 3) is only partly fixed. `paints_over` keeps an opacity, background, or color track on the engine when a press row or a look's mask names it (`press.rs:47-56`, `lower.rs:536-540`). A sibling lowered track still squares the opacity. That is finding 1.
- `currentcolor` (Astra 4) is fixed for the settled computed color. An in-flight color is not followed. Finding 3.

## Findings

1. **SHOULD-FIX.** A lowered scale, translate, rotate, or color track double-applies look and press opacity on Android. `box_layer` stores the already composed opacity in the layer base (`host/linux/src/paint/layer.rs:79-87`, `base[4] = p.opacity`). When that layer does not own `LOWER_OPACITY`, the painter also records a group opacity of the same value inside the layer body (`host/linux/src/paint.rs:1015-1026`, `host/linux/src/canvas.rs:767-772`, op 7). `ExactView` replays that body into the `RenderNode` (`host/android/canvas/src/main/java/dev/exact/canvas/ExactView.kt:1109-1111`), `Player` plays op 7 as `saveLayerAlpha` (`Player.kt:742-749`), and `Layers.sample` sets `node.alpha` from `base[4]` on every frame (`Layers.kt:309-321`). Hover opacity 0.8 during the test fixture's 200 ms scale transition paints as 0.64. `play_transitions` decides per property (`lower.rs:204-210`), so a scale track still opens the layer while `paints_over` correctly refuses the opacity track. The CPU painter never opens a layer, so `looks_tests.rs` cannot see it. This is the same base slot style opacity already used, so a style opacity under 1 with a lowered transform has the same shape. It becomes visible for this feature because a look or press now puts a non-1 opacity on a node whose style opacity is 1.

   Fix: when the open layer does not have `LOWER_OPACITY`, pass base opacity 1 so `RenderNode.alpha` stays 1 and the recorded group opacity is the only fade. A look or press change already re-records the row, so the group opacity updates. Leave `base[4] = p.opacity` for a layer that does own the opacity track.

2. **SHOULD-FIX.** A look that clears a color or a shadow keeps the base. The runner sends an explicit `unset` as `written` with no mask bit (`runner/src/instance/looks.rs:42-46`). `BoxPaint::looked` applies a row only when `mask.has` (`host/linux/src/paint/presented.rs:268-284`), so a cleared background, border side, or `box-shadow` stays the base's. Apple drops the key (`PressPaint.swift:301-303`: "the base's goes"). CSS `unset` is the initial: transparent background, `currentcolor` border, no shadow. Cleared opacity is already 1 (`press.rs:86-90`). The same mask-only check makes an unset opacity look fail `paints_over` (`press.rs:55-56`), so an opacity track still lowers and the reader replaces that 1. The names test a few lines above already uses `mask.union(written)` (`press.rs:82`).

   Fix: in `looked`, if `written.has(id) && !mask.has(id)`, set background to transparent, that border side to `currentcolor`, and `box-shadow` to none. In `paints_over`, treat `l.mask.union(l.written).has(row)` as the look naming the row.

3. **NIT.** A look's `currentcolor` is the node's computed text color (`host/linux/src/paint/presented.rs:243-244`, used at `266-267`), which is the settled style. A color transition in flight stays on that underlying color while the glyphs move. `box-shadow` resolving its own color is LLP 1064, not this gap.

   Fix: when the presented text color is in flight, resolve the look from that shown color; otherwise keep `computed_row(TextColor)`.

4. **NIT.** `looks_tests.rs` checks what it claims for hover, the absolute press (blue at 0.5 over the page, red border under it), the child, disabled, a finger after the gesture, and the cache. It never paints `:focus-visible`, never moves a resting pointer by scroll, never resolves `currentcolor`, and its header says Android paints this recording while every assert uses `PainterChoice::Cpu` and reads pixels only after the scale transition has ended (`looks_tests.rs:137-162`). The partial-press check (`part[0] < 250 && part[0] > 127`, line 83) only proves the pixel is between hover yellow and the settled press.

   Fix: add a focus-visible pixel, a `currentcolor` background, and a canvas-layer assert that a scale track under hover opacity 0.8 records a single fade (base alpha 1, or no second op 7). A scroll under a resting mouse should show the new hover on the frame after `follow_pointer`.
