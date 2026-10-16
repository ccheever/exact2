# Recipe: native views

What Contract cannot draw is a Swift view: a hyphenated tag, listed in `app.json` `modules`,
that the app's module (`modules/apple/*.swift`) makes. The Contract places, names and hears it:

```text
component Pad
  state last = ""
  action picked(step: string)
    last = step
  view
    column
      step-strip steps="16" message=picked aria-label="Kick steps" height=48
      text last testId="last"
```

```swift
import UIKit // for macOS too, import AppKit under #if os(macOS)

final class Pad: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["step-strip": ExactNativeFactory(for: Pad.self) { pad, props, events in Strip(pad, props, events) }]
    }
}
let exactModule: ExactModule.Type = Pad.self

final class Strip: ExactNativeInstance {
    private let strip = StripView() // a UIView that draws and has a tap recognizer
    private let pad: Pad
    private var ticket: ExactTicket?
    init(_ pad: Pad, _ props: [String: String], _ events: ExactNativeEvents) {
        self.pad = pad
        super.init(events: events)
        try? setProps(props)
        strip.tapped = { [weak self] step in events.message(String(step)); self?.start() }
        events.intrinsicSize(CGSize(width: 320, height: 48))
    }
    // A live ticket costs CPU; stopped, it's gone
    func start() {
        guard ticket == nil else { return }
        ticket = pad.context.frames { [weak self] f in
            guard let self, !self.strip.advance(to: f.now) else { return }
            self.ticket?.stop(); self.ticket = nil
        }
    }
    override var view: ExactNativeView { strip }
    override func setProps(_ props: [String: String]) throws { strip.steps = Int(props["steps"] ?? "") ?? 16 }
    override func destroy() { ticket?.stop() }
}
```

- **Props are strings**, the whole object again on each change. A name Contract knows
  is the box's, not the module's: an attribute or event (`value`, `size`, `name`,
  `title`, `input`, `playing`). Check a name with `contract vocab <name>`.
- **Events:** `events.message(text)` runs `message=`; `press()`, `change(v)`, `load()` likewise.
- **Time is the session's:** `context.frames` and `context.after(ms)`, never a
  `CADisplayLink` or `DispatchQueue` timer, so the agent's `clock +N` moves the animation
  and a drive repeats.
- **Touches:** recognizers on the view. A touch its `point(inside:)` declines goes to what
  is under the box (a capsule blocks only its shape), unless the box has `press=` or
  `isUserInteractionEnabled = false`.
  The agent's plain `tap` runs no recognizer: use `agent ios --touch platform`.
- **Size:** `intrinsicSize` is the natural size; an authored `width`, `height` wins.
- **Accessibility:** [native views](../contract-for-agents.md#native-views-accessibility).
- **First frame:** views attach after first paint. Only a cheap one it shows (a splash)
  says `ExactNativeFactory(firstFrame: true)`, never an engine's. The module is made with
  it, on main: make an `AVAudioEngine` on first use, not in `init`.
