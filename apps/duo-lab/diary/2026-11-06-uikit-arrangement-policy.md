surfaces: ios, macos, web
agent: Codex
task: let UIKit own arrangement presentation

### Rough

- Arrangement adoption disabled navigation-bar safe-area changes, including an app's explicit native setting. Removing that override leaves the navigation item with UIKit's default or the route hatch's configuration.
- Peer-position checks must allow UIKit's safe-area changes. They now measure content position relative to the native inset.

### Lean in

- UIKit owns arrangement allocation, bar minimization, scroll-edge effects and navigation transitions. The hatch retains authored pane content and supplies its native child controllers.
- Other hosts keep the authored responsive CSS layout.

### Needed

- Native navigation configuration: provided through the route hatch
- Independent pane scrolling and retained content: provided

### Checkpoints

- UIKit: native policy and peer-inset tests pass on an open iOS 27.1 Duo simulator (2 tests, no skips). Existing containment and focus cases pass too.
- Duo Lab: the arrangement smoke passes through open, folded and closed poses in Split, Overlay and Authored modes, with retained state, independent scrolling, confirmation retention, and cancelled and completed native Back gestures.
- Repository: build, Clippy, format, caps, boot and workspace checks pass. The full Cargo suite hit SVG and Tailwind time limits under load. Tailwind passes alone; SVG still exceeds its five-second limit at 5.2 seconds.
