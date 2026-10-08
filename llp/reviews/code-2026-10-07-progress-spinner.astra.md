1. **Should-fix — non-square web indicators become rotating ellipses.** [host/web/index.html:82](/private/tmp/bsky4-rv/wt-progress/host/web/index.html:82) gives the pseudo-element the full content width and height. `radial-gradient(closest-side, …)` defaults to an ellipse. Thus `progress width=80 height=20` draws an 80×20 ellipse which rotates outside its box, rather than the centred 20px circle promised by LLP 1069.001. Both web targets are affected; an ordinary stretched flex item also triggers it. [CSS gradient specification](https://drafts.csswg.org/css-images-3/#radial-gradients).
   
   Render a centred square sized to `min(100cqw, 100cqh)` with an explicit circular mask. Add rectangular and stretched-box rendering assertions; comparing the two web targets cannot catch their shared stylesheet bug.

2. **Should-fix — sizing depends on layout facts that become stale.** [host/web/src/element.rs:354](/private/tmp/bsky4-rv/wt-progress/host/web/src/element.rs:354) decides `justify-self:start` from the parent’s current display. The JS compiler omits dynamic styles while constructing that static projection.
   
   For example, put a progress inside `box width=200 display=(grid ? "grid" : "block") grid-template-columns="1fr"`, with mutable state `grid=true`. JS samples a block parent and permanently emits `justify-self:start`, producing a 20px-wide indicator box where wasm and native initially stretch it to 200px. Wasm also misses the update when a surviving parent changes from block to grid: its refresh path skips unchanged `Control` children.
   
   Make the alignment depend on the live parent display and the indicator’s position on both web executors. Test initial bound display and transitions between block, flex, grid, and absolute positioning against kernel dimensions.

3. **Nit — the `max` refusal misstates HTML.** [contract/lower/src/controls.rs:407](/private/tmp/bsky4-rv/wt-progress/contract/lower/src/controls.rs:407) says `max` makes progress determinate. `<progress max="100">` remains indeterminate; only the presence of `value` determines that. The amended LLP repeats the error. Keep the requested refusal, but describe `max` as unsupported by this implementation and correct the documentation. [HTML specification](https://html.spec.whatwg.org/multipage/form-elements.html#the-progress-element).

The basic lowering, refusal, native-widget, and Linux-paint tests are meaningful. However, the web test checks emitted CSS rather than circular geometry. Apple tests explicitly lack a session, so clock takeover and exit handling remain untested; they also do not assert the exposed accessibility role and busy state. Source tracing found no additional defect in the native sizing thresholds, colour choices, press pass-through, or final exit cleanup.

Rust formatting passed. An in-memory check of the existing web clock passed pause, seek, and infinite-animation settlement. Cargo, Swift, and browser runtime suites were not run: this read-only checkout has no build artifacts. No files changed.

**No blocker-severity defect found. I recommend fixing both web sizing defects before landing.**