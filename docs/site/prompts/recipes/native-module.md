```prompt Add a native module in Swift
Give the Exact app in this folder a native module in Swift for a platform API that has no view. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/native-module.md as the source for each step. The exact2 checkout has the same page at docs/recipes/native-module.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which platform API the module calls, ask me before you write code. If it is sound, MIDI or a transport, stop and tell me: they are built in (`bun exact.mjs docs sound midi and media`), and the app needs no module. If it needs a view (a map, the camera), use the native-views recipe instead.
3. Write one `ExactModule` subclass in `modules/apple/<name>.swift`, and name it with `let exactModule`. Use `call` for an answer within 100 ms, and `later` for anything slower; it replies once with `send` or `fail`. Throw `ExactNativeRefusal` to refuse. Use the session's clock (`context.now()`, `context.after(ms)`), never a timer of your own. app.json needs nothing for the module.
4. In app.ts, ask the module from each source, as the recipe shows. When `native` is not `available` (the build, or a platform with no module), return a default value. The web has no `call`: use `later` there, answered only by `modules/web/index.js`.
5. In app.test.contract, add a test for what the app shows with the default value.
6. Run `bun exact.mjs ios` to build the Swift, then `bun exact.mjs verify ios` until it passes. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me what the module does, what the tests and verify reported, and how to see it by hand: `bun exact.mjs ios --run`. `print` output shows in `bun exact.mjs agent ios logs`; `NSLog` does not.
```
