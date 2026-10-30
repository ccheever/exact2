surfaces: ios, android
agent: Claude Code, Claude Opus 5.5
task: keep a recording going while the app is hidden

## Rough

No Android emulator boots on this Mac, so the foreground service, its notification and the part writer's
recovery were built and checked in the APK's manifest (`aapt2 dump xmltree`) but never run.

## Lean in

One grant line, `device.microphone-background purpose.recordingInBackground`, gives both iOS's `audio`
background mode and Android's microphone foreground service; the app's code did not change beyond the line
and its string.

## Needed

- background audio capture: provided (iOS driven on a simulator; Android built, not run)
- crash-safe recording on Android: provided, not run

## Checkpoints

- iOS build: smooth
- Android build: smooth; not run (no emulator)
- driving the app: smooth on the iOS simulator
