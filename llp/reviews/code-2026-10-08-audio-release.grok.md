I'll review the tip commit read-only, starting with the repo instructions and the audio-session change.The review stays read-only. I'll read the tip commit, the audio-session sources, and the LLP 1096 amendment.The diff is small. Next I’ll read the session and video holders, plus every caller the amendment depends on.The holder count itself is consistent. The session yield on re-mute is not, and that is the change's purpose, so this should not land as-is.

## Blocker

**Re-mute, clearing `src`, and teardown never successfully give the session back** — `host/apple/Sources/ExactKit/VideoModule.swift:259` and `:167`, `host/apple/Sources/ExactKit/AudioSession.swift:43`.

`update` calls `holdSession` before `module.update`, and `invalidate` calls it before `destroy`. On re-mute the `AVPlayer` is still playing audible audio when `release` runs. `release` then does `try? setCategory(.ambient)` and `try? setActive(false, options: .notifyOthersOnDeactivation)`, and it has already dropped the holder.

Apple's `AVAudioSession.ErrorCode.isBusy` is deactivation while the session is still playing. That call fails here, the error is discarded, and `notifyOthersOnDeactivation` is the API that tells the interrupted app it may resume. An ambient category does not send that. A podcast another app had paused stays paused — the case this commit names. The same order hits a cleared `src` and a removed view: the item is torn down only after the yield has already failed, and nothing retries once I/O is actually idle. If `setCategory(.ambient)` throws as well (category change while active and playing), the session stays `.playback` and active with zero holders.

Fix: stop I/O first (apply mute, or `destroy`, which pauses and clears the item), then yield. Treat a failed `setActive(false)` as not released and retry once the player is idle. Keep the holder until that call succeeds.

## Should-fix

**A failed `hold` is sticky** — `AudioSession.swift:33`, `VideoModule.swift:140`.

`holders.insert` happens before `setCategory` / `setActive`, and `holdsSession = true` happens before the `try`. Scenario: unmute during a call, `setActive(true)` throws (`cannotInterruptOthers` or `isBusy`). The view will not try again (`audible == holdsSession`), and that id stays in the set, so a later last `release` will not yield either. The sound arm retries on interruption-end; a video does not.

Fix: insert only after both calls succeed. On throw, remove the id and leave `holdsSession` false so the next `update` retries.

**The test does not cover the bug** — `host/apple/tests/ExactKitTests/AudioSessionIOSTests.swift:23` (file is `#if os(iOS)`, `tearDown` at `:11`).

It drives `hold` / `release` with no `AVPlayer`, then reads `category`. `setActive(false)` can be removed, or fail with `isBusy`, and the test still passes. `tearDown` restores the static `category` and not the live session or `holders`, so a failure mid-test leaks an id into later tests in the same process and leaves `AVAudioSession` on `.ambient`. tvOS compiles the same `AudioSession` and runs none of this.

Fix: assert the yield only after a playing player has been muted, reset `holders` and the live category in `tearDown`, and compile the test for tvOS.

## Nit

**Fullscreen AVKit mute does not move the hold** — `VideoModule.swift:259`. The fullscreen controller is the same player and adds no second holder, so accounting is fine. The hold reads the `muted` prop, not `player.isMuted`. Mute or unmute from AVKit's own chrome (the fullscreen controls) leaves the session where the prop put it.

**Callers are on the main thread today; `AudioSession` does not require it** — `AudioSession.swift:17`. Video `update` / `forget`, the sound-arm interruption observer (`queue: .main`), and canvas audio (it hops to main) all run there. `holders` is an unsynchronized `nonisolated(unsafe)` set. A later notification-thread caller races it and `AVAudioSession`.

## Checked, not filed

Paused-but-unmuted keeps `.playback` (`muted` and `src` only). That is the right state for a media-session podcast: `.ambient` is mixable, and LLP 1098 D8 says a mixable session is never Now Playing. `backgroundModes: ["audio"]` keeps working across that pause. Agent mode still skips video, sound, and canvas. `deinit` and a second `invalidate` release once. Route-change pause and a phone-call interruption do not re-activate a video-only holder; LLP 1098 already says resume is not automatic, and this commit does not make that worse. The sound arm and canvas audio still hold forever, as the amendment says, so an app that has started either one never returns to ambient.

This blocks landing. The counter is fine; the re-mute yield does not resume the other app.
