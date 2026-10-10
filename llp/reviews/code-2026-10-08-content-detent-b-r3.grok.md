I'll review the tip commit against the round-2 notes, starting with the project instructions and both review files.The tip commit is `2089d51ec`. I'll read the changed paths against both round-2 findings, including the documented ancestor limitation.## Should-fix

**A `width: auto` content-box parent still feeds the sheet’s `vh` padding into the trial.** `kernel/src/kernel/geometry.rs:179`

`column` is only `display: flex; flex-direction: column` (`contract/lower/src/tags.rs:69`). Width stays `auto`, and content-box is the default, so a block-level column fills the sheet: the border box stays the sheet width and `padding-left: 10vh` comes out of the content box. The trial always does the opposite for content-box. It treats the published border box as specified content plus the last edges and grows it by the new edges:

```179:184:kernel/src/kernel/geometry.rs
            // Its authored width is the content box's under `content-box`
            // (CSS's default): that stays and the edges grow around it;
            // under `border-box` the border box stays.
            if own.box_sizing == crate::BoxSizing::ContentBox {
                width += pad.left + pad.right + border.left + border.right - before;
            }
```

On a 390-point sheet that is 844 tall, then 220: padding goes 84.4 → 22, and the stand-in’s content width goes 305.6 → 368. Wrapping text changes the trial height, the sheet follows, and it walks again. `width: 100%` is the case that formula gets right, and it is the only content-box case the new test builds (`host/apple/tests/it/fit_content.rs:582`). That arm stays green on the previous trial too, because a stretched in-flow child’s width never included that padding.

Keep the border box when the used width is the fill (`width: auto`, or a flex/stretch size). Put the screen-resolved padding inside it. Grow the border box only when a definite content-box width was actually the used content size. Add the auto case next to the `width: 100%` one.

**The stand-in still drops siblings that set the route’s width.** `kernel/src/kernel/geometry.rs:223`

Astra’s round-2 case is unchanged: a 400-point row, a 200-point sibling, and a `flex: 1; min-width: 0` fit route lay out at 200, and the trial lays the route out at 400 because the copied parent’s only child is the route.

```222:223:kernel/src/kernel/geometry.rs
        let holder_node = tree.new_leaf(outer, parent.unwrap_or(slot), false);
        tree.set_children(holder_node, &[root]);
```

The route’s text wraps onto fewer lines and the detent comes up short. It does not hunt. §9.11’s review log calls this “not built”; it is not the limitation the lead accepted, and it is not in `QUEUE.md`. Keep the siblings that share the parent’s main axis, with the route’s block size left indefinite.

## Round-2

Fixed: the button face uses `env_for` (`kernel/src/arena/button.rs:142`); the region certificate does too (`kernel/src/region/state.rs:839`); `Env::screened` installs the screen’s segment grid (`kernel/src/style/env.rs:339`) and the host keeps that grid separate from the sheet’s (`ExactViewIOS.swift:318`, sent before the resize at `:348`); `rebooted` sends the screen before insets and segments (`:391`); the comments in `fold.rs` and `Session.swift` match that. Per-route `vh`, both stack orders, a phone’s flat 1×1 grid, and web/Linux/macOS (they never set `screen`) are unchanged. The ancestor `vh` write-up matches Astra’s clamp example in §9.11 and `QUEUE.md:101`. SVG lengths staying 0 is still the queued item at `QUEUE.md:102`.

The new tests fail on the previous commit where they have a tooth. The button face would be 10% of the sheet (20 at 200, 4.4 at 44) against 84.4. The segment test would read the sheet’s 100-point row instead of the screen’s 400, including after the detent is cleared. The padding test’s border-box arm would emit a new extent at 44 / 200 / 844; its content-box arm would not. The region certificate has no new test. I did not execute them.

Nothing blocks landing.
