```prompt Add a native view in Swift
Put a native Swift view (a map, the camera) in the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/native-views.md as the source for each step. The exact2 checkout has the same page at docs/recipes/native-views.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which view and which platform API it uses, ask me before you write code. If it is a video, use the `video` element with a `poster`, and write no Swift. If it is a Skia or Reanimated screen, build it from boxes, SVG and keyframes, and write no Swift.
3. Name the view with a hyphenated tag in `app.json` `modules`, and make it in `modules/apple/<name>.swift`: an `ExactModule` whose `views` names an `ExactNativeFactory`, and an `ExactNativeInstance` per view, as the recipe shows.
4. Follow the recipe's rules: props are strings and come all together on each change; send events with `events.message`, `press()`, `change(v)` or `load()`; give a natural size with `intrinsicSize`; take time only from `context.frames` and `context.after(ms)`, never a `CADisplayLink` or a timer. Make the view accessible as `bun exact.mjs docs native views accessibility` says.
5. In app.contract, place the view by its tag with an `aria-label`, and handle its events in actions. If app.ts reads data from the module, write no bake code: the resource shows its `else` row until launch.
6. Run `bun exact.mjs test web`, then `bun exact.mjs ios` to build the Swift, then `bun exact.mjs verify ios` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
8. Tell me the tag and what the view does, what the tests and verify reported, and how to drive it by hand: `bun exact.mjs agent ios --touch platform` for its touches, and `bun exact.mjs agent ios logs` for the newest crash report.
```
