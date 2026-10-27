# The bake-off on a device

The bake-off grades scrolling on a 60 Hz simulator. `device.sh` measures the same apps on a real iPhone or iPad,
at the screen's own rate (120 Hz on ProMotion), with the scroll probe of `../heavy-list/probe` (its README has
every field): the probe drives the app's tallest scroll view programmatically at constant speeds and records every
frame. It needs no touches, so it runs on any device that takes a development build.

## Running it

```sh
R=~/bakeoff/rounds/r26/apps OUT=/tmp/bakeoff-device
for a in contacts-exact contacts-swiftui chat-exact chat-swiftui; do bench/bakeoff/device.sh build $R/$a $OUT; done
export BENCH_DEVICE=<udid> BENCH_SIGN_IDENTITY=<sha1> BENCH_PROFILE=<profile.mobileprovision>
bench/bakeoff/device.sh run $OUT contacts-exact contacts-swiftui chat-exact chat-swiftui
```

- `build` needs no device and no keychain. A native app is compiled as the skeleton's `build.sh` compiles it, but
  for `iphoneos` with `-O -whole-module-optimization`, as it would ship (the simulator build is `-Onone`). An Exact
  app is copied to `$OUT/src/`, pointed at this checkout (`exact.mjs update`) and built as an unsigned release
  archive (`host/apple/build.mjs --device --archive --unsigned`). Exact builds share `$OUT/target`: the first
  takes about 4 minutes on an M-series Mac, the rest reuse the kernel.
- Both arms declare iPhone and iPad, so on an iPad both run full screen at the iPad's size. `--iphone` builds an
  iPhone-only app, which an iPad runs in compatibility mode; the result's `idiom` (0 phone, 1 pad), `listPt` and
  `maxFps` say which ran.
- `run` signs each app with the probe as `dev.bakeoff.device.<name>` (`resign.sh`; a wildcard development profile
  covers it), installs all of them, runs `RUNS` rounds (default 2) of `SCENARIOS` (default `fling ladder`) with the
  rounds outermost, then one blank-sampled fling each, uninstalls everything (also on failure) and prints
  `summarize.py`'s table. Results are `$OUT/results/<name>-<scenario>-t-<round>.json` and `<name>-fling-b-1.json`;
  `python3 bench/heavy-list/probe/analyze.py <file>` prints one run per speed.
- Signing needs the login keychain unlocked: from a shell without it, `codesign` fails with
  `errSecInternalComponent`.
- A bake-off list is 12,000 to 60,000 pt tall, shorter than a 24k pt/s segment's 48,000 pt. `run` sets the probe's
  `BENCH_BOUNCE=1` (a segment that meets an end turns back at the same speed, so every segment moves its whole
  2 s) and `BENCH_MINHEIGHT=4000` (the probe's default ignores a list under 20,000 pt).

## Real touches on a device

`host/apple/feel/probe.mjs` (real touches, UIKit's physics as the baseline) runs on simulators only. Running it on
a device (LLP 1080.000 stage 4) needs:

1. **The touch runner for `iphoneos`.** `touches.mjs` `buildRunner` assumes the simulator throughout: the
   `iPhoneSimulator` platform's `XCTRunner.app` and frameworks, the simulator triple, ad hoc signing, `simctl
   install`. A device build takes the `iPhoneOS` platform's copies, the team identity and an embedded profile
   (`com.exact.touches.xctrunner` is covered by a wildcard one), `devicectl device install app`, and an
   `.xctestrun` with `UseDestinationArtifacts` and `TestHostBundleIdentifier` in place of paths (LLP 1080.000 D1).
2. **Settings › Developer › UI Automation** turned on once on the device, by a person.
3. **A way back to the Mac.** The runner connects out to `phoneBridge()`'s address, the Mac's first IPv4
   (`touches.swift` `Link` is IPv4 only). A device on USB alone reaches the Mac through CoreDevice's tunnel, which is
   IPv6 (`devicectl device info details`, "Tunnel IP Address"); the device must share a Wi-Fi network with the Mac,
   or the runner and the bridge must take IPv6. A kept runner is reached on `127.0.0.1:<port>`, which is the
   device's loopback, not the Mac's, so on a device each session starts a runner or reaches it over the bridge.
4. **The accessibility defaults** that `openTouches` reads and restores with `simctl spawn … defaults` do not apply
   on a device; it must skip them.
5. **The recorder for `iphoneos`.** `probe.mjs` `recorder()` builds `feel.m` for the simulator only. On a device
   it is built for `iphoneos`, embedded in the app and signed with it (as `resign.sh` embeds the scroll probe), and
   inserted by `devicectl device process launch --environment-variables` with `FEEL_OUT` under the app's Documents.
6. **A live channel to the recorder.** `probe.mjs` reads `trace.jsonl` while it plays (the hello, when the list has
   come to rest, each blank check's end) and starts sampling and the blank check by writing files beside it. On a
   simulator those are the Mac's files; on a device they are in the app's container, which `devicectl device copy`
   reaches in seconds per call, too slow for the 100 ms polls. The recorder would stream its lines to the Mac over a
   socket (a `FEEL_CONNECT`, as the runner's) and take the two switches on the same socket.

## Results

None yet. On 2026-10-27 all twenty r26 apps of the ten long-list pairs (contacts, podcasts, inbox, watchlist,
chat, moodboard, feed, photos, timetable, expenses) built for an iPad Pro 12.9" (M1, 120 Hz, iPadOS 27.2), and none
could be signed: the shell's login keychain was locked.
