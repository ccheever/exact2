surfaces: web, ios
agent: Codex
task: support browser fold layouts

### Rough

- The web host ignored CSS fold signals when the JavaScript interfaces were absent. It now reads CSS posture, segment counts and segment rectangles, and observes media changes without requiring a resize.
- Wide navigation displaced panes from the fold. It now requires enough width within each horizontal segment.
- The browser diagnostics hatch inserted its readout after first paint, moving the list down 18 pixels. Removing the panel and its web module keeps the initial layout stable.

### Lean in

- Missing JavaScript posture data does not prevent CSS segment detection. The Fold screen's authored facts show posture, segment counts and viewport size.

### Needed

- CSS posture and segment data: browser APIs
- Split and Overlay: provided by the native iOS app

### Checkpoints

- `bun test host/web/tests/display-mode.test.mjs host/web-js/facts.test.mjs host/web/tests/wide.test.mjs --isolate` passes. With both JavaScript interfaces absent before boot, the browser test verifies CSS fold changes in resources, rendered text, packed facts and segment rectangles.
- A Chromium drive through the tunnel verifies pane placement in emulated book, laptop and flat poses and the native app requirement. No page errors occur.
- On a physical iPhone Duo running iOS 27.3 beta, Safari reports one segment, unavailable JavaScript posture, no matching CSS posture or split queries, and unavailable segment widths. This is observed with the inner display partially folded and Viewport Segments enabled. The cause remains unverified.
- Panel removal: the mobile browser drive reports no layout shifts, no page-module request and no page errors. All 22 pane, navigation and viewport browser tests pass.
- Native compatibility: a fresh iOS build and a drive through Split, Overlay, Authored and Back retain drafts, counters and pane identities after the manifest cleanup.
- Caps, boot and `git diff --check` pass.
