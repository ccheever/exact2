# Video Player

A single Contract screen plays a bundled, locally generated ten-second H.264
color study. Tap the note field to open the iPhone keyboard: the video gets
smaller while playback continues. Done closes the keyboard and restores the
picture's space. Native controls and the app's Pause/Play button both work.

From the repository root:

```sh
bun host/web/dev.mjs --app video-player
bun host/apple/build.mjs video-player --ios --run
bun host/apple/build.mjs video-player --run
```

The iOS command uses a simulator; `--device --run` targets a connected iPhone.
On macOS, resize the window to exercise the same flex allocation. There is no
software keyboard overlap on the desktop path.

The screen uses `interactive-widget="resizes-content"` with fixed heading and
composer rows and `video flex=1 min-height=0`. The player keeps its identity
across layout changes. Media runs on its own clock; the agent's `clock settle`
settles layout without seeking the video.

The component is an HTML video element on the web and an AVKit view on Apple.
AVKit lives in an optional, separately loaded `libexact_video.dylib` artifact.
The property and event matrix, ownership rules, and further player design are
in [LLP 1042](../../llp/1042-video.spec.md).

## Verification — 2026-09-18

- iPhone 17 Pro simulator: advancing playback, note editing, Pause/Play, and
  keyboard dismissal passed. The video height was **553 → 252 → 553 points**;
  keyboard overlap was 301 points and source generation stayed 1.
- Chrome: real playback and seeking passed, as did false boolean attributes,
  volume changes, PiP preference retention, and observable malformed-media
  errors. Resizing the app from 420×900 to 420×580 reduced video height from
  682 to 362 pixels while retaining the same element and advancing playback.
  Reload stopped and detached the previous player.
- The static server passed eight byte-range cases, including partial, suffix,
  open-ended and unsatisfiable requests. Seeking uses these responses.
- The focused kernel and web Rust tests passed, including intrinsic sizing,
  viewport policy, media event payloads and invalid property rejection.
- macOS: advancing playback, click-to-edit, note entry, Done and Pause/Play
  passed. The drive also exposed and fixed synchronous AppKit click tracking
  in the agent: the mouse-up event must be queued before mouse-down enters
  a native text field's tracking loop.
- Web, macOS and iOS app builds passed. Workspace formatting and focused
  Clippy across kernel, runner, Contract, web and Apple passed.
- Source-size/document caps and the one-module startup check passed.
- The full workspace build/test attempts encountered concurrent list changes:
  a borrowed list geometry lifetime in the Apple host and old `list_viewport`
  call signatures in Caltrain's metrics binary. Full-workspace Clippy was
  interrupted while app bake jobs remained pending. These runs do not establish
  a passing full-workspace gate.

PiP and AirPlay need physical-device verification. Background audio, Now
Playing, DRM, offline downloads, external caption tracks and controller
commands are designed in LLP 1042 but are not implemented by this sample.
Linux has layout and an explicit unavailable-player observation, not a decoder.
