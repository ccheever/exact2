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
import UIKit // AppKit on macOS, under #if os(macOS)

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

- **Props are strings**, the whole object on each change. A name Contract knows
  (`value`, `size`, `title`, `playing`) is the box's, not the module's: `contract vocab <name>`.
- **Events:** `events.message(text)` runs `message=`; `press()`, `change(v)`, `load()` likewise.
- **Time is the session's:** `context.frames`, `context.after(ms)`, never a
  `CADisplayLink` or a timer, so `clock +N` moves the animation. Timed output (notes,
  haptics) is a `sequence` (LLP 1125).
- **Touches:** recognizers on the view; one its `point(inside:)` declines goes to what is
  under the box unless it has `press=`. Plain `tap` runs none: `agent ios --touch platform`.
- **Size:** `intrinsicSize` is the natural size; an authored size wins.
- **Accessibility:** [native views](../contract-for-agents.md#native-views-accessibility).
- **Data:** `app.ts` asks: `native!.watch('state'); return native!.call({op: 'state'});`. The
  build has no module, so the resource shows its `else` row until launch: write no bake code.
- **A crash:** `agent ios logs` shows this build's newest crash report on this simulator.
- **First frame:** views attach after first paint. Only a cheap one it shows (a splash)
  says `ExactNativeFactory(firstFrame: true)`, never an engine's. The module is made with
  it, on main: make an `AVAudioEngine` on first use, not in `init`.
