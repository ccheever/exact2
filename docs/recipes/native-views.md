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
import UIKit // iOS only; for macOS too, import AppKit under #if os(macOS)

final class Pad: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["step-strip": ExactNativeFactory(for: Pad.self) { pad, props, events in Strip(pad, props, events) }]
    }
}
let exactModule: ExactModule.Type = Pad.self

final class Strip: ExactNativeInstance {
    private let strip = StripView() // a UIView that draws and has a tap recognizer
    private var ticket: ExactTicket?
    init(_ pad: Pad, _ props: [String: String], _ events: ExactNativeEvents) {
        super.init(events: events)
        try? setProps(props)
        strip.tapped = { events.message(String($0)) }
        events.intrinsicSize(CGSize(width: 320, height: 48))
        ticket = pad.context.frames { [strip] frame in strip.advance(to: frame.now) }
    }
    override var view: ExactNativeView { strip }
    override func setProps(_ props: [String: String]) throws { strip.steps = Int(props["steps"] ?? "") ?? 16 }
    override func destroy() { ticket?.stop() }
}
```

- **Props are strings**, the whole object again on each change. A name Contract knows
  (`value`, `size`, `label`, `name`) is the box's, and the compiler says to rename it.
- **Events:** `events.message(text)` runs `message=`; `press()`, `change(v)`, `load()` likewise.
- **Time is the session's:** `context.frames` and `context.after(ms)`, never a
  `CADisplayLink` or a `DispatchQueue` timer, so the agent's `clock +N` moves the animation
  and a drive repeats.
- **Touches:** recognizers on the view. A touch its hit test declines (a `point(inside:)`
  that follows the drawn shape) goes to what is under the box, so a capsule smaller than its
  box blocks nothing; the box keeps it with `press=` or `isUserInteractionEnabled = false`.
  The agent's plain `tap` runs no recognizer: drive with `agent ios --touch platform`.
- **Size:** `intrinsicSize` is the natural size; an authored `width`, `height` wins.
- **Accessibility:** `aria-label` (and `role`) make the box one element when the view says
  nothing to VoiceOver. A view with parts sets `accessibilityElements`
  (`UIAccessibilityElement`s with a label, `.button` and `accessibilityActivate`); the box is
  then a group under the label.
- **First frame:** a view attaches after first paint, so a slow one never delays it. A cheap
  one that must be in the first frame (a boot splash) says `ExactNativeFactory(firstFrame: true)`.
