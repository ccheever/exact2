# Recipe: native views

What Contract cannot draw is a Swift view: a hyphenated tag, listed in `app.json` `modules`,
that the app's module (`modules/apple/*.swift`) makes. The Contract places, names and hears it:

```text
component Pad
  state last = ""
  state playing = false
  action picked(step: string)
    last = step
  action toggle
    playing = not playing
  view
    column
      step-strip steps="16" playing=(playing ? "true" : "false") message=picked aria-label="Kick steps" height=48
      button press=toggle testId="play"
        text (playing ? "Stop" : "Play")
      text last testId="last"
```

```swift
import UIKit // iOS only; for macOS too, import AppKit under #if os(macOS)

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
        strip.tapped = { events.message(String($0)) }
        events.intrinsicSize(CGSize(width: 320, height: 48))
    }
    override var view: ExactNativeView { strip }
    override func setProps(_ props: [String: String]) throws {
        strip.steps = Int(props["steps"] ?? "") ?? 16
        strip.playing = props["playing"] == "true"
        if strip.playing, ticket == nil {
            // Frames only while the strip moves; it stops them once it has drawn its rest.
            ticket = pad.context.frames { [weak self] frame in
                guard let self else { return }
                if !self.strip.advance(to: frame.now) { self.ticket?.stop(); self.ticket = nil }
            }
        }
    }
    override func destroy() { ticket?.stop() }
}
```

- **Props are strings**, the whole object again on each change. A name Contract knows
  is the box's, not the module's: an attribute or event (`value`, `size`, `name`,
  `title`, `input`, `playing`). Check a name with `contract vocab <name>`.
- **Events:** `events.message(text)` runs `message=`; `press()`, `change(v)`, `load()` likewise.
- **Time is the session's:** `context.frames` and `context.after(ms)`, never a
  `CADisplayLink` or a `DispatchQueue` timer, so the agent's `clock +N` moves the animation
  and a drive repeats.
- **Frames only while something moves.** A live ticket keeps the display link running at
  the display's rate, so a ticket left on costs CPU on a still screen (one app's idle
  0.8% was its strips' tickets; stopped, 0.0%). Take one when motion starts (a prop, the
  module's engine starting) and `stop()` it once the view has drawn where it rests; a
  view that is not shown (`active` false, another tab) takes none. A Core Animation
  animation needs no ticket. Chain `after(ms)` only while there is something to do.
- **Memory:** a layer that draws (`draw(in:)`) holds a bitmap of its whole bounds at the
  screen's scale, 36 bytes a point² on a 3× iPhone, whether or not it shows anything.
  Draw what moves on a small layer of its own, not a second full-size one, and give a
  layer no more bounds than it draws (one app's five strips held 6.3 MB in ten).
- **Touches:** recognizers on the view. A touch its hit test declines (a `point(inside:)`
  that follows the drawn shape) goes to what is under the box, so a capsule smaller than its
  box blocks nothing; the box keeps it with `press=` or `isUserInteractionEnabled = false`.
  The agent's plain `tap` runs no recognizer: drive with `agent ios --touch platform`.
- **Size:** `intrinsicSize` is the natural size; an authored `width`, `height` wins.
- **Accessibility:** `aria-label` makes the box one element when the view says nothing to
  VoiceOver, and a `role` of `button`, `link`, `img`, `switch` or `checkbox` (`aria-checked`)
  or `slider` (`aria-valuenow`) its traits. A view with parts sets `accessibilityElements`
  (`UIAccessibilityElement`s with a label, `.button` and `accessibilityActivate`); the box is
  then a group under the label.
- **First frame:** a view attaches after first paint, so a slow one never delays it. A cheap
  one that must be in the first frame (a boot splash) says `ExactNativeFactory(firstFrame: true)`.
