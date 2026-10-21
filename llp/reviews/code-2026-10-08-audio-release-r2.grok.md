I'll start with the project instructions and the two review notes, then read the tip commits against the audio-session design.AGENTS.md points at the binding rules. I'll read those, the two round-1 reviews, and the tip commits before judging the audio-session change.The round-2 commit changes when the session is released. I'll read the holders, player lifetime, and Now Playing path against both review notes.Nothing blocks landing. The two round-1 blockers are fixed in the code. Two holes remain, neither of which sticks the session in the wrong state forever.

## Round 1

**Resolved — deactivating under a live player (Astra and Grok blockers).** Re-mute applies `isMuted` in `VideoArm.update` (`VideoArm.swift:308`) before `holdSession(false)` (`VideoModule.swift:270-272`). `setActive(false, .notifyOthersOnDeactivation)` runs only from `giveUpIfIdle` (`AudioSession.swift:66-68`), and only when `players` is empty. `playerWent` is after `destroy`, which pauses and drops the item (`VideoArm.swift:614-619`). That matches the header: deactivating with running I/O stops the player (`AVAudioSession.h` around the `setActive` comment; iOS 26 stops returning `isBusy` and still deactivates).

**Resolved — Now Playing.** `mediaTitle != nil` keeps the hold (`VideoModule.swift:266`), the same predicate as `NowPlaying.updated` (`NowPlaying.swift:74`). On the update that drops it, `module.update` clears the claim before `release`. A paused, unmuted element with a source still holds. Mute or a cleared `src` keeps `.playback` while `mediaTitle` is set.

**Resolved — a failed hold no longer sticks.** `holders.insert` is after both calls succeed (`AudioSession.swift:36-43`). On failure with nobody else holding, the category is put back to `.ambient`. `holdsSession = true` only after success (`VideoModule.swift:141`). Canvas sets `active` only after `activate()` returns; the sound arm's interruption-end path calls `activate()` again.

**Still open — the video does not actually retry that failure.** Below.

**Partly open — the test.** A new case checks the category. It does not lock "do not deactivate", a real player, `mediaTitle`, or a failed `hold`. `tearDown` still restores only the static category.

**Unchanged nit — AVKit chrome mute.** Below.

**Unchanged nit — main thread.** `holders` and `players` stay `nonisolated(unsafe)`. Callers today are on main (video update, the sound-arm observer with `queue: .main`, canvas hops to main). Left as-is.

## Should-fix

**A failed video hold is not retried for that prop set** — `host/apple/Sources/ExactKit/VideoModule.swift:260`

`last = props` is committed before `hold`, and the next `update()` returns when the built props are unchanged. `holdsSession` stays false, so the flag is retryable, and nothing calls `hold` again.

Mount a podcast (`mediaTitle`, source, unmuted) while `setActive(true)` throws (a call, `insufficientPriority`). `hold` rolls the category back to `.ambient` and does not insert. `module.update` still runs and `NowPlaying` publishes under that mixable category, which LLP 1098 D8 says is never Now Playing. The call ends. The sound arm retries `activate()` from its interruption observer; a video-only app has no such observer. Lock-screen controls stay ineligible until some other prop changes.

Compute `audible` before the dedup return. If `audible && !holdsSession`, call `holdSession(true)` even when `props == last`. On the release path, keep today's order: `module.update` (mute and `NowPlaying.updated`) before `holdSession(false)`.

**The new test still passes if release deactivates under a live player** — `host/apple/tests/ExactKitTests/AudioSessionIOSTests.swift:33`

`testWithAPlayerAliveReleaseGoesAmbientAndThePlayerLeavingGivesItUp` asserts `category == .ambient` after `release` and again after `playerWent`. `setActive(false)` inside `release` leaves that category the same, so the assertion stays green. No `AVPlayer` runs, so mute-then-category is untested (Astra asked for playback time to keep advancing). `tearDown` (`:11`) restores `AudioSession.category` only; `holders` and `players` are private, so a mid-test failure leaks an id into the process for later tests. The file is `#if os(iOS)` while the code is iOS and tvOS.

Play a short item, mute it, release, spin the run loop, and assert `timeControlStatus` stays `.playing` and `currentTime` moves; then pause, `playerWent`, and assert the session became inactive (`AVAudioSession.didBecomeInactiveNotification` on this SDK, or a package test hook around `giveUpIfIdle`). Add a package reset of both sets and the live category, and compile the test for tvOS.

## Nit

**Fullscreen AVKit mute still follows the prop** — `host/apple/Sources/ExactKit/VideoModule.swift:266`

The hold reads `props["muted"]`. AVKit's own chrome writes `player.isMuted`. `volumeChanged` (`VideoArm.swift:151`) emits `volumechange` and does not touch `AudioSession`. Mute from the fullscreen controls leaves `.playback` in place; unmute leaves `.ambient`. Drive the hold off the player's mute (the KVO the arm already has), or write the prop back before the next session decision.

## Checked

**Accounting.** `playerCame` runs only when `create` returned a handle and the process is not in agent mode (`VideoModule.swift:159`). `playerWent` runs only on that same path, after `handle` is cleared, so a second `invalidate` (deinit after `NodeView` already invalidated) returns before it (`:173`). Agent mode never inserts, and `playerWent`'s `remove` misses, so it does not deactivate. Init without a module never inserts. `holders` and `players` are separate; release removes a hold only, and a muted player stays in `players`, which is what blocks `giveUpIfIdle`.

**Category change while a muted player is running.** Safe under the published contract. `setCategory` while active is an immediate route change (`AVAudioSessionRouteChangeReasonCategoryChange`). The call that stops running I/O is `setActive(false)`, and that waits until `destroy` has paused the player. Mute is already on the `AVPlayer` before the category flip, so the route change has no audible samples. I did not run this on a device; the missing player test above is what would lock it. If `setCategory(.ambient)` itself throws, `release` swallows it and `giveUpIfIdle` does not try the category again, so a muted player would keep `.playback` until it is destroyed (deactivation then still notifies). The header does not describe that throw.

**Now Playing, the success path.** Acquire sets `.playback` before `module.update` publishes. Dropping `mediaTitle` clears the claim inside that update, then releases. Unmount releases the hold before `NowPlaying.left`; the element is leaving, and `playerWent` deactivates only after the claim is cleared and the player is paused.

Nothing blocks landing.
