# Recipe: native views

A view needing a platform API (video, a map, the camera) is a Swift view: a hyphenated tag in
`app.json` `modules`, made by `modules/apple/*.swift`. Drawing is not: a Skia or Reanimated
screen ports to boxes, SVG, keyframes and transitions, on every platform. Contract places it:

```text
component Clip
  state last = ""
  action seeked(at: string)
    last = at
  view
    column
      video-view src="intro.mp4" message=seeked aria-label="Intro" height=180
      text last testId="last"
```

```swift
import UIKit // AppKit on macOS, under #if os(macOS)

final class Clip: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["video-view": ExactNativeFactory(for: Clip.self) { clip, props, events in Video(clip, props, events) }]
    }
}
let exactModule: ExactModule.Type = Clip.self

final class Video: ExactNativeInstance {
    private let strip = VideoView() // an AVPlayerLayer and a tap recognizer
    private let pad: Clip
    private var ticket: ExactTicket?
    init(_ pad: Clip, _ props: [String: String], _ events: ExactNativeEvents) {
        self.pad = pad
        super.init(events: events)
        try? setProps(props)
        strip.tapped = { [weak self] at in events.message(String(at)); self?.start() }
        events.intrinsicSize(CGSize(width: 320, height: 180))
    }
    // A live ticket costs CPU
    func start() {
        guard ticket == nil else { return }
        ticket = pad.context.frames { [weak self] f in
            guard let self, !self.strip.advance(to: f.now) else { return }
            self.ticket?.stop(); self.ticket = nil
        }
    }
    override var view: ExactNativeView { strip }
    override func setProps(_ props: [String: String]) throws { strip.src = props["src"] ?? "" }
    override func destroy() { ticket?.stop() }
}
```

- **Props are strings**, all of them on each change. A name Contract knows (`value`,
  `title`) is the box's: `contract vocab <name>`.
- **Events:** `events.message(text)` runs `message=`; `press()`, `change(v)`, `load()` likewise.
- **Time is the session's:** `context.frames`, `context.after(ms)`, never a
  `CADisplayLink` or timer, so `clock +N` moves it; timed audio is a `sequence`.
- **Touches:** recognizers on the view; a point `point(inside:)` declines goes under the box
  unless it has `press=`. Drive them with `agent ios --touch platform`.
- **Size:** `intrinsicSize` is the natural size; an authored size wins.
- **Accessibility:** [native views](../contract-for-agents.md#native-views-accessibility).
- **Data:** `app.ts` asks: `native!.watch('state'); return native!.call({op: 'state'});`. The
  build has no module, so the resource shows its `else` row until launch: write no bake code.
- **A crash:** `agent ios logs` shows the newest crash report.
- **First frame:** views attach after first paint. Only a cheap one says
  `ExactNativeFactory(firstFrame: true)`, never an engine's; the module is made with it, on
  main: make an `AVAudioEngine` on first use. A splash is Contract: native slows launch.
