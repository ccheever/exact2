# RFC 0005: Incremental Style Updates for Taffy

**Type:** RFC
**Status:** Implemented
**Systems:** Tooling, Layout
**Author:** Charlie Cheever / Claude
**Date:** 2026-03-22
**Revised:** 2026-03-23
**Related:**

## Summary

Reduce unnecessary work in the style update hot path by detecting when layout-affecting properties have not actually changed, and skipping the Taffy style rebuild and re-layout in those cases. Also fix a pre-existing correctness bug in the transform-only fast path that clears non-transform styles.

## Motivation

Every style update currently triggers a full `to_taffy_style()` conversion (74 fields, including grid-track vector allocation) and `taffy.set_style()` call — even when only visual properties like `opacity` or `backgroundColor` changed and layout is unaffected. For text nodes, `sync_text_measure_context()` is also called unconditionally. This is wasteful for the most common animation workloads (opacity fades, color transitions, transform-driven scroll effects).

The transform-driven workload is particularly important because the reconciler already has a dedicated transform-only fast path (`host-ops.ts:772`), but that path has a correctness bug under full-style semantics (see "Transform-Only Path Fix" below). This RFC fixes that bug as part of the same change.

## Critical Constraint: Full-Style Semantics

**The current protocol uses full-style semantics, not deltas.**

- The JS encoder (`encoder.ts:58`) converts the canonical style to a protocol payload, including every property that is `!== undefined`. Properties that are undefined are dropped.
- The encoder emits a `SET_STYLE` payload with a bitmask indicating which fields are **present** in the message (`buffer-writer.ts:326`).
- The Rust decoder **resets `StyleProps` to defaults** before applying the mask (`style_decoder.rs:68`: `*styles = StyleProps::default()`).
- Omitted fields are **cleared to defaults**, not left unchanged.

This means the bitmask is a **presence-mask**, not a change-delta. A message with "only paint bits set" can still implicitly change layout by clearing previously-set layout properties to their defaults.

**Example:** A node has `width: 200` set in JS. A later update only changes `opacity: 0.5`. The encoder emits a payload with both the `width` and `opacity` bits (because `width` is still defined on the style object). But if the reconciler were to strip the `width` for some reason, the decoder would reset `width` to `Auto` — a layout change, despite no layout bits being present in the mask.

**Therefore, the mask alone cannot determine whether layout changed.** The optimization must compare actual layout-affecting field values between the old and new `StyleProps`.

### Mask Propagation Gap

Additionally, the protocol mask is not currently propagated past the decoder boundary. `decode_style_payload()` returns only `StyleProps` (`style_decoder.rs:22`), and `ParsedOp::SetStyle` carries only `(ViewId, StyleProps)` (`parser.rs:46`). The kernel's `set_style()` method never sees the mask. Any mask-based optimization would require plumbing changes across the parser → kernel boundary. The comparison-based approach proposed below avoids this entirely.

## Field Classification

Style properties fall into three categories. **Classification is based on what the code does today**, not what CSS or React Native semantics would suggest. Fields that are stored on `StyleProps` but not currently wired into `to_taffy_style()` or `build_text_measure_context()` are classified as paint-only, even if they arguably should affect layout. Fixing missing layout wiring is out of scope for this RFC.

### Layout Fields (affect Taffy style)

These properties are read by `to_taffy_style()` (`style.rs:845-957`) and require `taffy.set_style()` when their values change:

`width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`, `padding` (all edges), `margin` (all edges), `flex_direction`, `flex_wrap`, `justify_content`, `align_items`, `align_self`, `align_content`, `flex_grow`, `flex_shrink`, `flex_basis`, `row_gap`, `column_gap`, `position_type`, `top`, `right`, `bottom`, `left`, `display`, `aspect_ratio`, `overflow` (`style.rs:865` — maps to `taffy::Overflow`), `grid_auto_flow`, `grid_template_columns`, `grid_template_rows`, `grid_column`, `grid_row`, `justify_items`, `justify_content_grid`

(~34 logical fields; `padding` and `margin` are `Edges<Dimension>` structs containing 4 sub-fields each.)

### Text Measurement Fields (affect layout indirectly)

These properties feed into `build_text_measure_context()` (`lib.rs:105-119`) and require `sync_text_measure_context()` when changed on Text nodes. Changes affect layout through text size recalculation:

`font_size`, `font_weight` (affects glyph metrics — `lib.rs:115`), `font_family`, `line_height`, `font_variant_numeric`, `number_of_lines`

**Note on `font_weight`:** While visually a paint property, `font_weight` changes glyph widths and therefore text measurement. The current `build_text_measure_context` at `lib.rs:115` explicitly includes it. It must be treated as layout-affecting for text nodes.

### Paint Fields (visual only)

These properties are stored on the `Node` and read by the renderer but do not affect Taffy layout or text measurement:

`opacity`, `background_color`, `border_color`, `border_width`, `border_radius`, `border_radius_top_left`, `border_radius_top_right`, `border_radius_bottom_left`, `border_radius_bottom_right`, `shadow_color`, `shadow_offset_x`, `shadow_offset_y`, `shadow_radius`, `shadow_opacity`, `text_color`, `font_style`, `text_align`, `text_decoration_line`, `ellipsize_mode`, `tint_color`, `backdrop_blur`, `gradient_type`, `gradient_angle`, `gradient_color_start`, `gradient_color_end`, `resize_mode`, `z_index`, `direction`, `writing_mode`, `transform_translate_x`, `transform_translate_y`, `transform_scale`, `transform_rotate`

**Design decision — `transform` is paint-only:** Taffy does not process CSS transforms, so transforms do not affect layout in Exact. This is a deliberate divergence from CSS, where transforms can affect overflow bounds and scroll containers. In Exact, scroll container bounds are computed from layout frames, not transformed frames. This decision should be revisited if Exact ever supports transform-aware scrolling.

**Note on `border_width`:** In CSS and React Native, `border-width` affects the box model (it maps to `taffy::Style::border`). However, `to_taffy_style()` does not currently map `border_width` — the Taffy `border` field receives its default (zero) via `..Default::default()` at `style.rs:955`. This is likely a missing feature, not a design choice. Classified as paint-only here to match current behavior. A follow-up should wire `border_width` into `to_taffy_style()` and reclassify it as a layout field.

**Note on `direction` and `writing_mode`:** These are stored on `StyleProps` but are not read by `to_taffy_style()` or `build_text_measure_context()`. They are currently consumed only by the platform renderer (e.g., `ExactEngine.swift:463`). Classified as paint-only to avoid forcing unnecessary Taffy rebuilds. If Taffy integration for RTL/writing-mode is added later, these should be reclassified.

**Note on `font_style`:** Classified as paint-only, consistent with `build_text_measure_context()` not including it. However, italic glyphs can have different metrics than roman. If Exact's platform text measurement callback considers `font_style`, this should be reclassified as a text measurement field.

## Design

### Layer 1: Layout-Field Value Comparison (Primary Optimization)

Compare the layout-affecting field values of the incoming `StyleProps` against the node's current `StyleProps`. If all layout-affecting fields are identical, skip `to_taffy_style()` and `taffy.set_style()`. For text nodes, also compare text measurement fields before calling `sync_text_measure_context()`.

**This approach is correct under full-style semantics** because the incoming `StyleProps` always contains explicit values for every field (defaults for fields not present in the wire message). If the layout field values match the node's current values, layout genuinely hasn't changed — regardless of which bits were present in the protocol message.

**Skip logic:**

| Condition | Taffy action | Text action |
|-----------|-------------|-------------|
| Layout fields changed | `to_taffy_style()` + `set_style()` | — |
| Text measurement fields changed (Text node) | — | `sync_text_measure_context()` |
| Both layout + text measurement changed (Text node) | `to_taffy_style()` + `set_style()` | `sync_text_measure_context()` |
| Only paint fields changed | — | — |

The two conditions are evaluated independently. `layout_changed` drives the Taffy style rebuild; `text_changed` drives the text measure sync. No explicit `mark_dirty()` call is needed in any case:

- `taffy.set_style()` internally marks the node (and ancestors) dirty.
- `sync_text_measure_context()` calls `taffy.set_node_context()` (`lib.rs:137`), which internally calls `mark_dirty()` (`taffy_tree.rs:464`), propagating up the ancestor chain.

Text measurement fields don't appear in `to_taffy_style()` — they affect layout indirectly through the measure callback. When only text fields change, `sync_text_measure_context()` alone is sufficient: it updates the measure function and dirties the node so `compute_layout` re-invokes it.

**Implementation sketch:**

```rust
pub fn set_style(&mut self, id: ViewId, styles: StyleProps) -> Result<()> {
    let (taffy_node, taffy_style_opt, text_changed) = {
        let node = self.nodes.get_mut(&id)
            .ok_or(KernelError::ViewNotFound(id))?;
        let layout_changed = !styles.layout_fields_eq(&node.styles);
        let text_changed = node.node_type == NodeType::Text
            && !styles.text_measure_fields_eq(&node.styles);
        let taffy_node = node.taffy_node;
        node.styles = styles;
        let taffy_style = if layout_changed {
            Some(node.styles.to_taffy_style())
        } else {
            None
        };
        (taffy_node, taffy_style, text_changed)
    };

    if let Some(taffy_style) = taffy_style_opt {
        self.taffy.set_style(taffy_node, taffy_style)
            .map_err(|e| KernelError::LayoutError(e.to_string()))?;
    }

    if text_changed {
        // set_node_context() internally calls mark_dirty(), so no
        // separate mark_dirty() is needed for the text-only path.
        self.sync_text_measure_context(id)?;
    }

    Ok(())
}
```

`layout_fields_eq()` compares the ~34 layout-affecting fields. `text_measure_fields_eq()` compares the 6 text measurement fields. Both are field-by-field `==` comparisons marked `#[inline]`. `StyleProps` already derives `PartialEq` (`style.rs:402`), so the field types support `==`.

**Cost:** In the paint-only case (layout fields unchanged), all ~34 field comparisons must execute before `layout_fields_eq()` returns `true` — this is the worst case for comparison cost, not the best. But it is still much cheaper than `to_taffy_style()` (which allocates `Vec` for grid tracks, constructs enum variants, maps all 74 fields) and `taffy.set_style()` (which marks the node and its ancestors dirty in the layout tree). The text-only tier avoids `to_taffy_style()` entirely — `sync_text_measure_context()` handles both the context update and the dirty marking via `set_node_context()`.

**Note on `set_style_size`:** The kernel also exposes `set_style_size()` (`lib.rs:326`) which directly mutates `width` and `height`. This always changes layout fields, so the comparison optimization does not apply to that path.

### Transform-Only Path Fix (Prerequisite)

The reconciler's existing transform-only fast path (`host-ops.ts:772`) has a correctness bug that must be fixed as part of this work. The bug:

1. `isTransformOnlyStyleUpdate(oldStyle, newStyle)` detects only transforms changed (`host-ops.ts:772`)
2. `setStyleTransform(id, x, y)` sends a `SET_STYLE` opcode with only `TRANSFORM_X | TRANSFORM_Y` bits (`buffer-writer.ts:303`)
3. The decoder resets `StyleProps` to defaults and populates only transforms (`style_decoder.rs:68`)
4. The kernel replaces the node's entire `node.styles` with this mostly-default struct (`lib.rs:311`)

For any node with non-default layout properties (e.g., explicit `width`, `flex_grow`), this silently clears those properties to defaults. The bug is masked because the next full `setStyle` call restores them, but the intermediate state is incorrect — and with Layer 1's comparison, `layout_fields_eq()` would correctly detect the change and propagate the incorrect defaults to Taffy.

**Fix:** Modify the JS reconciler's transform-only path to send a full canonical style (with transforms updated) through the normal `setStyle()` encoder path, instead of calling the special `setStyleTransform()` writer.

```typescript
// host-ops.ts — before (buggy)
if (isTransformOnlyStyleUpdate(oldStyle, newStyle)) {
  enc.setStyleTransform(instance.id, transformX, transformY);
} else {
  setStyle(enc, instance.id, newStyle);
}

// host-ops.ts — after (correct)
if (!stylesEqual(oldStyle, newStyle)) {
  setStyle(enc, instance.id, newStyle);
}
```

This trades wire-size compactness for correctness. The extra bytes are negligible for local IPC. Combined with Layer 1, this path still achieves the intended performance win: the full style is sent, the decoder produces a complete `StyleProps`, and `layout_fields_eq()` detects that layout hasn't changed, skipping Taffy entirely.

`setStyleTransform()` in `buffer-writer.ts` can be retained for future use (e.g., if a `SET_STYLE_TRANSFORM` opcode is added that the kernel handles by mutating only transform fields without a full style reset). But it should not be called from the reconciler until the kernel supports it correctly.

### Renderer Invalidation

When paint-only changes skip Taffy, the renderer still needs to know the node's visual properties changed. The current rendering pipeline reads `node.styles` directly (not Taffy output) for paint properties, so updating `node.styles` is sufficient — the renderer will see the new values on the next frame.

If a more explicit dirty-tracking mechanism is needed in the future (e.g., for partial repaints), a `style_generation: u64` counter on `Node` could be incremented on every `set_style` call, while a separate `layout_generation: u64` increments only when layout fields change. The renderer diffs against its last-rendered generation. This composes well with the snapshot-based agent API (generation numbers are monotonic and meaningful across snapshots). This is not required for Phase 1.

### Impact Assessment

Under the current encoder, `setStyle()` re-emits every defined property from the canonical style object. A node with `{width: 200, opacity: 0.5}` that changes to `{width: 200, opacity: 0.8}` emits both `width: 200` and `opacity: 0.8`. The decoded `StyleProps` will have `width: Points(200)` — matching the node's existing value. `layout_fields_eq()` returns true, and Taffy is skipped.

**Where this helps** (common cases):
1. **Animation frames** — opacity, backgroundColor, and transform changes on nodes with stable layout. The comparison detects "layout unchanged" even though layout fields are present in the wire message.
2. **Scroll-driven effects** — high-frequency visual-only updates. With the transform-only path fix, these now correctly send full styles, and Layer 1 skips Taffy.
3. **Theme/color changes** — bulk updates to paint properties across many nodes.

**Where this does not help:**
1. Updates that genuinely change layout (resizing, re-flexing, etc.).
2. First `setStyle` on a new node — the comparison against `StyleProps::default()` will show layout changed if any layout field is non-default.

### Layer 2: Protocol Delta Support (Future RFC)

If the protocol is changed to support **delta semantics** (where the bitmask means "these fields changed, others are unchanged"), then:

1. The decoder would NOT reset `StyleProps` to defaults for delta updates
2. The JS encoder would track per-node style state and send only changed fields
3. The kernel could skip Taffy based on the mask alone (a single bitwise AND)

This requires a new opcode `SET_STYLE_DELTA` (or a flag bit in the existing header) and coordination between the JS encoder and Rust decoder.

**Important invariant for Layer 2:** The JS encoder must own canonical per-node style state and guarantee exactly-once field delivery. If both the reconciler and the encoder perform diffing, double-application bugs are possible. Layer 2 also enables a correct `SET_STYLE_TRANSFORM` opcode — a delta with only transform bits is unambiguously "only transforms changed" — restoring the wire-size savings removed by the Phase 1 fix.

### Layer 3: Derived Field Caching (Low Priority)

Some Taffy fields are expensive to compute:
- `grid_template_columns` / `grid_template_rows` (allocate `Vec<TrackSizingFunction>` — `style.rs:847`)
- `justify_content` varies by display mode

These could be cached and only recomputed when their source `StyleProps` fields change. This is a micro-optimization that should be driven by profiling data.

## Implementation Plan

### Phase 1: Comparison-based skip + transform-only fix (3-4 days)

**Kernel (Rust):**
- Implement `layout_fields_eq(&self, other: &StyleProps) -> bool` on `StyleProps`, comparing the ~34 layout fields listed above
- Implement `text_measure_fields_eq(&self, other: &StyleProps) -> bool`, comparing the 6 text measurement fields
- Update `Kernel::set_style()` with skip logic (layout change → `set_style`, text-only change → `sync_text_measure_context`, paint-only → skip)
- Verify that all field types used in comparisons implement `PartialEq` correctly (spot-check `Dimension`, `Edges`, `GridPlacement`)

**JS (reconciler):**
- Remove the `isTransformOnlyStyleUpdate` / `setStyleTransform` branch from `host-ops.ts:772` and clean up the now-dead `isTransformOnlyStyleUpdate` helper and `transformX`/`transformY` extraction
- Route all style changes through the normal `setStyle()` path
- Retain `setStyleTransform()` in `buffer-writer.ts` but mark it as unused pending a future `SET_STYLE_TRANSFORM` opcode

**Tests:**
- Paint-only update → Taffy `set_style` not called, `node.styles` updated
- Layout field change → Taffy `set_style` called
- Text measurement field change on Text node → `sync_text_measure_context` called (which internally marks dirty via `set_node_context`)
- Text measurement field change on non-Text node → no text sync
- Full-style reset (all defaults) on node with non-default layout → Taffy `set_style` called
- Transform-only update on node with non-default layout → layout fields unchanged, Taffy skipped (regression test for the bug)

**Benchmark:** Measure `Kernel::set_style()` call duration for (a) 1K-node opacity animation, (b) 1K-node width animation, (c) 1K-node transform animation via full style path. Target: paint-only and text-only paths at least 5x faster than layout-change path.

### Phase 2: Protocol delta support (future RFC)
- Design `SET_STYLE_DELTA` opcode or flag
- Define the exactly-once delivery invariant for the JS encoder
- Track per-node style state in JS encoder
- Implement incremental decoder in Rust
- Re-enable wire-compact `SET_STYLE_TRANSFORM` under delta semantics

## Resolved Questions

1. **`borderWidth` boundary:** Paint-only under current code. `to_taffy_style()` does not map `border_width` to `taffy::Style::border` — it receives a zero default via `..Default::default()` at `style.rs:955`. This is likely a missing feature (CSS and React Native both treat border-width as layout-affecting). A follow-up should wire it into `to_taffy_style()` and reclassify it.

2. **`transform` classification:** Paint-only. Taffy does not process transforms. Scroll container bounds in Exact are computed from layout frames, not transformed frames. This is a deliberate CSS divergence.

3. **`overflow` classification:** Layout-affecting. It maps directly to `taffy::Overflow` at `style.rs:865`.

4. **`font_weight` classification:** Text-measurement-affecting (layout-affecting for Text nodes). It feeds `build_text_measure_context()` at `lib.rs:115` and changes glyph metrics.

5. **`StyleProps::default()` reset in decoder:** Intentional for correctness under full-style semantics. Cannot be changed without a protocol-level delta mode (Layer 2).

6. **`direction` and `writing_mode`:** Paint-only under current code. Not read by `to_taffy_style()` or `build_text_measure_context()`. Consumed only by the platform renderer (e.g., `ExactEngine.swift:463`). If Taffy integration for RTL/writing-mode is added, reclassify as layout fields.

7. **`text_align` and `ellipsize_mode`:** Paint-only. Absent from `TextMeasureContext` (`lib.rs:111`), absent from native text measurement (`PlatformTypes.swift:730`), applied during rendering (`ExactNodeRenderer.swift:485`). They are rendering directives that position text within an already-measured box.

8. **Transform-only path:** In-scope fix, not a deferred audit. The bug is established by the current code path (`host-ops.ts:772` → `buffer-writer.ts:303` → `style_decoder.rs:68` → `lib.rs:303`) and directly conflicts with the transform-driven workloads this RFC targets.

## Open Questions

1. Should Phase 1 include a `style_generation` / `layout_generation` counter on `Node` for the renderer and agent API, or defer that to a separate change?

2. For the Phase 1 benchmark, should we measure against a real animation workload (e.g., a `FlatList` scroll with opacity-based fade-in) in addition to synthetic 1K-node tests?
