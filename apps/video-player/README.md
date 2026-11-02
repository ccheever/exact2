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

## Limits

PiP and AirPlay need physical-device verification. Background audio, Now
Playing, DRM, offline downloads, external caption tracks and controller
commands are not implemented by this sample.
Linux has layout and an explicit unavailable-player observation, not a decoder.
