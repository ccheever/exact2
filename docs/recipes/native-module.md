# Recipe: a native module in Swift

**Sound, MIDI and a transport are built in** ([guide](../contract-for-agents.md#sound-midi-and-media),
`apps/metronome`): write no module. Another platform API with no view (a measurement
hook) is the app's one module, which `app.ts` asks: `modules/apple/*.swift`, one `ExactModule` subclass that `let exactModule` names.
app.json needs nothing for it ([app-json](app-json.md)).

```swift
import Foundation

final class Engine: ExactModule {
    override func call(_ request: [String: Any]) throws -> [String: Any] {
        switch request["op"] as? String {
        case "flags": return ["autoPlay": UserDefaults.standard.bool(forKey: "AutoPlay")]
        default: throw ExactNativeRefusal("engine answers flags")
        }
    }
    override func later(_ request: [String: Any], reply: ExactReply) {
        DispatchQueue.global().async { reply.send(["ok": true]) }
    }
}
let exactModule: ExactModule.Type = Engine.self
```

```contract
shape Flags
  autoPlay: bool

shape Reply
  ok: bool

component App
  resource flags = engineFlags() as shape Flags else empty()
  mutation played as shape Reply
  action play
    send played = enginePlay()
  view
    button press=play testId="play"
      text (flags.autoPlay ? "Playing" : "Play")
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

const sources: Sources = {
  engineFlags: async (_args, _store, _storage, native) => {
    if (!native?.available) return { autoPlay: false }; // the build, or no module here
    native.watch('flags');
    try { return native.call({ op: 'flags' }) as Result<'engineFlags'>; }
    catch { return (await native.later({ op: 'flags' })) as Result<'engineFlags'>; } // the web
  },
  enginePlay: async (_args, _store, _storage, native) =>
    (await native!.later({ op: 'play' })) as Result<'enginePlay'>,
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

- **`call`** runs on main within the answer's 100 ms; throw to refuse. **`later`** replies once,
  `send` or `fail(_:)`, from any thread. Values are JSON's. The web has no `call`; it answers
  `later` only from `modules/web/index.js`.
- **`context`:** `changed("flags")` asks again every answer that called `native.watch("flags")`;
  `now()` and `after(ms)` are the session's clock; `agent` is true under the agent. Files,
  secrets: [files-and-grants](files-and-grants.md).
- **Launch arguments:** Exact passes none. `xcrun simctl launch <udid> <bundle id> -AutoPlay
  1` puts each `-Name value` in `UserDefaults`.
- **Printing:** `print`, not `NSLog`, shows in `agent ios logs` and `xcrun simctl launch --console`.
- **`app.onLine`** (app hatch) is watched from its first read: read it once, even when
  `isNew`, to hear changes.
- **Exiting:** iOS has no quit. `exit(0)` skips pending writes: keep it to a measurement
  hook, after `fflush(stdout)`.
- **A Swift edit rebuilds** (`bun exact.mjs ios`, Swift 5 mode); views from the same module are
  [native-views](native-views.md).
