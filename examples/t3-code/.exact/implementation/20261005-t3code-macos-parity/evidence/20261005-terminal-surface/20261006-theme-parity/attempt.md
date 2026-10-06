# Terminal theme and renderer verification

2026-10-06. Implementation `b01d97a0fab9d8622dd5ec0db43ecde4bcb5ae65`, following merge
`61b7a428c`. Base `eb259ef281b224520b35faad61929f8be9cf40ed`.
macOS 26.6.2, Xcode 27.0, Bun 1.4.2; Chromium 154.0.8037.98; Retina scale 2.

## Result

**The renderer checks pass. Full original T3 Code terminal parity does not.**
This branch supplies Ghostty in WKWebView and a development harness. It does not yet
supply the original drawer, terminal tabs, splits, PTY sessions, menus, Add to chat,
sign-in terminals, or persistence. Those remain the separate drawer/layout/integration
and sign-in tickets. The harness toolbar is deliberately labelled development and
is not the original app's terminal toolbar.

The original applies theme colors and font preferences. Before this repair, the
bridge passed only system light/dark and the harness used the emulator's fallback
colors and 12 px default font. Now the app's selected appearance, stock/preset/custom
terminal background, foreground, cursor and selection, and simple/advanced font
preferences reach the native view. Simple typography follows the code font/size;
advanced typography uses its independent terminal values. Changes during page loading
are reapplied when the page finishes loading.

The black image was not valid visual evidence. In this run the default layer capture
was entirely transparent: every RGBA channel was zero. The window capture shows the
app and its terminal. [The failed capture](failed-layer-capture.png) is preserved as
a failure, not proof of an empty terminal. Use `screenshot … window` for this app.
No image here has been retouched, recolored or generated.

## Inspect the pictures

| State | Native WKWebView | Original renderer in Chrome |
| --- | --- | --- |
| Stock light | [Native](theme-light.png) | [Original](oracle-light.png) |
| Stock dark | [Native](theme-dark.png) | [Original](oracle-dark.png) |
| Grove light | [Native](theme-grove.png) | [Original](oracle-grove.png) |
| Custom dark | [Native](theme-custom.png) | [Original](oracle-custom.png) |

All four pairs are 720×420 CSS/point boxes, 1440×840 pixels, font size 13,
90 columns × 22 rows. Both show scrollback through line 2000, bold/italic/underline/
strike, 16 ANSI colors, truecolor, CJK, Korean, emoji, Nerd Font symbols, a link and
file path, and the prompt. An alternate-screen round trip precedes this final state.
The same native instance changes through all four themes without losing output.
[Increasing the font to Menlo 18](theme-custom-font18.png) changes 90 columns to 65.

These are **original renderer comparisons, not full Electron screenshots**. The oracle
bundles the original `GhosttyTerminalSurface` and dependencies from the pinned source,
mounts them with the reference scrollbar styles, and feeds [the same bytes](fixture.txt).
Its app-independent wrapper supplies the theme and font, callbacks and asset URLs.
It does not run React, the drawer, or a server. The native shots run the actual app's
`T3TerminalView` inside an AppKit test window.

The images are not pixel-identical. Mean absolute RGB differences on the 0–255 scale
are 1.22 light, 1.58 dark, 1.22 Grove and 1.03 custom. About 3% of pixels differ.
Visual inspection shows matching grid placement and content, with browser rasterization
and glyph-edge differences. These numbers are descriptive, not a blanket parity threshold.
See [pixel measurements](pixel-comparison.json) and [original grid/text readback](oracle-report.json).

Integrated app captures at 1280×840:

- [Dark with ANSI output](app-dark-window.png)
- [Light with ANSI output](app-light-window.png)
- [Real key delivery and visible loopback output](app-loopback-window.png)

The app is in a fresh isolated store, disconnected, with onboarding behind the harness.
`echo hello` is echoed by the labelled loopback fixture; it is **not a shell command
executed against a PTY**. [Drive transcript](app-window-drive.jsonl).

## Behavior and remaining differences

| Check | Observation |
| --- | --- |
| Stock/preset/custom theme | Pass. Native canvas background sampled after every change; output retained. Custom foreground/cursor/selection payload and all 10 built-in palette halves checked. |
| Font settings | Pass. Snapshot preferences covered by Bun tests; native font change resizes the grid. |
| Rendering | Pass for the pictured fixture, including wide glyphs and alternate-screen restoration. |
| Keys | Native text, Return, Up, Backspace, Tab, Ctrl+C and Escape encode correctly. Declined Cmd+K returns to the application. Original Chrome `ls -la`, Return and Up produce the same bytes. |
| Paste | Native host-supplied paste and bracketed paste pass. System Cmd+V clipboard path not rechecked. |
| IME | `NSTextInputClient` marked-text/commit test commits Korean once. Physical Korean 2-Set and candidate-window position unverified. |
| Hidden output | Output continues parsing while hidden and paints when shown. |
| Flood | Native 5 MB numbered-output test passes without loss. See timings in the native log. |
| Mouse, selection and links | Ported pure tests pass; real pointer selection, scrollbar drag, link activation and right-click unverified in this attempt. |
| Scrollbar and cursor | Visible in native captures. Physical scrollbar drag and reduced-motion cursor behavior not certified by these still images. |
| Full terminal UI/session | Not implemented here. No drawer/session parity claim. |
| Scaling | The 1/4/11/44-view benchmark was skipped; its prior proposed budget is still not accepted by this attempt. |

## Source and validation

Reference: [T3 Code 1e2ecbd9758830669684b494d4398f626b0576e0](https://github.com/pingdotgg/t3code/tree/1e2ecbd9758830669684b494d4398f626b0576e0).
Eleven reference source files, including the renderer, theme palette and drawer,
were retrieved through GitHub's contents API and compared byte-for-byte against the
local archive. All matched. [Source hashes](reference-source.json). Binary integrity
also passes: [vendor log](vendor.log).

| Validation | Result / log |
| --- | --- |
| Bun app tests | 1496 pass, 0 fail. [Log](bun-tests-pinned.log) |
| Native terminal XCTest | 8 pass, 1 scale test skipped, 0 failures. [Log](native-tests.log) |
| App Rust tests | 10 pass. [Log](app-rust-tests-pinned.log) |
| Strict TypeScript | App and terminal entry clean. [App](tsc.log), [entry](terminal-tsc.log) |
| Contract build | Pass, 48693 nodes. [Log](contract.log) |
| macOS app build and launch | Pass. [Build](app-build.log), [drive](app-window-drive.jsonl) |
| Repository build/test/lint/caps/boot | Pass. [Build](gate-build.log), [tests](gate-test.log), [Clippy](gate-clippy.log), [fmt](gate-fmt.log), [caps](gate-caps.log), [boot](gate-boot.log) |

The shell initially selected Bun 1.3.14. It produced two failures, including a WASM
callback-table signature failure. [That failed run](failed-unpinned-bun-tests.log) is
retained. Both pass using the repository's required Bun 1.4.2; no tests were weakened.

## Reproduce

Run from the repository root with Bun 1.4.2 first in PATH. Use a fresh named store;
no pairing credentials or server are needed for these harness checks.

```sh
export PATH="$HOME/.bun-1.4.2/bin:$PATH"
export EXACT_APP_DIR="$PWD/examples/t3-code"
bun examples/t3-code/terminal-host/build.mjs
bun examples/t3-code/terminal-host/verify-vendor.mjs
bun test examples/t3-code
bun host/apple/build.mjs t3-code-macos --bundle
mkdir -p target/terminal-review
bun scripts/agent.mjs macos --storage terminal-review-fresh --size 1280x840 --json \
  'tap terminal-harness-toggle' 'clock +1000 real' \
  'prefer prefers-color-scheme dark' 'clock +300 real' \
  'screenshot target/terminal-review/dark.png window' \
  'prefer prefers-color-scheme light' 'clock +300 real' \
  'screenshot target/terminal-review/light.png window' \
  'tap terminal-harness-loopback' 'clock +300 real' \
  'type terminal-harness-view echo hello' 'type terminal-harness-view key Enter' \
  'clock +300 real' state 'screenshot target/terminal-review/loopback.png window' logs
```

Build the native `terminal` test binary with the existing [README recipe](../../../../../../README.md),
then run it with `T3_APP_DIR="$PWD/examples/t3-code"` and
`T3_TERMINAL_TEST_DIR="$PWD/target/terminal-review"`. The new
`testThemeChangesPreserveOutputAndResizeFont` captures the four theme states and the
font-size change. Tests create their own AppKit windows and dispose their views.

For the original comparison, use the pinned reference source above. Bundle
`apps/web/src/terminal/ghostty/surface.ts` for a browser, replacing only the `?url`
asset imports with locally served URLs and `lib/utils` with its `isMacPlatform`
helper. Mount in a 720×420 box with DPR 2 and the reference's scrollbar class styles.
Supply every required callback, including `beforeKey: () => true`; missing callbacks
invalidate keyboard checks. Set font size 13, apply the shared stock/Grove palettes
and custom colors `#123456`, `#abcdef`, `#fedcba`, `#11223380`, then write `fixture.txt`.
Wait for two animation frames before capture. Read `cols`, `rows` and
`snapshot.rowData` for the grid/text assertions, as recorded in `oracle-report.json`.

The source fingerprint in [report.json](report.json) includes tracked app inputs and
hashes of the generated terminal assets used by the native build. The manifest checks
artifact integrity; it does not turn the unverified rows above into passes.
