# LLP 1042: Video — browser semantics, native playback

**Type:** Spec
**Status:** Draft; §7 A and B accepted and built, C declined (Charlie, 2026-09-28)
**Systems:** Kernel replaced elements; Contract media attributes/events; web video; Apple AVKit; keyboard viewport; Video Player app
**Author:** Codex for Charlie Cheever
**Implementer:** Codex, 2026-09-18
**Date:** 2026-09-18
**Related:** LLP 1001 (CSS defaults), 1008 §9 (keyboard viewport), 1011 (intrinsic size), 1012 (agent operations), 1020 (optional native artifact), 1031 (session lifetime), 1039 (viewport facts)

## 1. Consumer and scope trade

Charlie requested a web-based video component with iOS/macOS options, then a
simple player that shrinks for the software keyboard. `apps/video-player` is
that consumer. Video comes off DEFERRED; router/viewport-fact integration
(1038/1039, previously not started) follows this consumer. The 1039 current
link gives this spec its working-set slot; neither proposal is deleted.

This document describes the implementation and separately records the remaining
product requirements. It does not claim that every AVFoundation capability is
implemented by admitting `video`.

## 2. Element and ownership

`video "assets/movie.mp4"` is a replaced leaf. `src` can also be named.
The web arm is an actual `HTMLVideoElement`; Apple is AVPlayer with
AVPlayerViewController or an inline AVPlayerLayer on iOS, and AVPlayerView on macOS. Decoding, HLS,
native controls, track menus, presentation and the media clock belong to those
engines. Codecs are engine capabilities, not promises by Exact. MP4/H.264 is the
bundled consumer. Native HLS and alternate tracks are delegated to AVKit;
the browser's supported formats remain the web oracle.

The kernel owns only the CSS box. Metadata reports the display size through the
existing intrinsic-size seam. Before metadata the default box is 300×150;
authored dimensions, aspect-ratio and flex constraints win. Video shares the
image's documented block-flow deviation (1011 §1). `object-fit` uses CSS's
fill/contain/cover/none/scale-down vocabulary; the video UA default is contain
(verified in Chrome), unlike CSS's general initial fill. Media never sizes its ancestor
from a playback frame. The sample explicitly chooses contain.

Apple imports AVKit only in `libexact_video.dylib`, loaded at first video
creation. ExactKit carries its opaque ABI; no Cargo feature or alternate core
build exists. The web helper is loaded on demand after the first animation
frame. It sets DOM properties and reports events; it does not render frames.

For iOS inline videos that request no controls, fullscreen behavior, PiP,
linear-playback restriction or video-frame analysis, the optional artifact uses
an AVPlayerLayer-backed view (Shop, 2026-09-19). PiP and frame analysis select
AVKit only when set to true by name (Charlie, 2026-09-28, §7 A): Chrome's
`<video>` without `controls` draws no UI, and without controls or automatic
start PiP cannot be reached. So `allowsPictureInPicturePlayback` and
`allowsVideoFrameAnalysis` are unset-means-no on a video without `controls`
(Live Text on a paused frame is the app's to ask for); with `controls`, AVKit
is used as before. The first presentation waits for source props instead of constructing a controller before
they arrive. Enabling any controller feature later promotes the same AVPlayer
without replacing its item or seeking. Once installed, that controller remains
until the node is destroyed, including when controls are subsequently hidden.
This does not replace AVKit's gestures or controls. `state.media.renderer`
distinguishes AVPlayerLayer from AVKit. Both remain in the existing optional
video artifact; this change does not remove its AVKit link dependency.

One node incarnation owns one player. Changing style, typing, window resizing
and keyboard notifications retain player identity and playback position. A new
source replaces the item, cancels old observers, and resets metadata; callbacks
carry an item generation. Unmount, session destruction and plan replacement
invalidate observers, periodic callbacks and playback. A stale node callback
cannot reach the replacement node with the same numerical id.

## 3. Authoring surface

HTML attribute spelling stays HTML spelling; DOM property spelling stays DOM
spelling. Boolean false removes an HTML boolean attribute, never serializes
`controls="false"` as if it disabled controls.

| Property | Meaning/default and projection |
|---|---|
| src, poster | HTTP(S) or bundled relative asset; resolved through the existing asset owner on Apple |
| controls, autoplay, loop, muted | false by default; native controls, playback request, item loop and audio mute |
| preload | none/metadata/auto; a hint, browser-owned on web. AVKit may prepare an item even under none so its Play control can work |
| playsinline | false by default; true keeps iPhone playback inline |
| crossorigin | browser CORS mode; not a native credential/header escape hatch |
| controlslist | browser tokens; macOS projects nofullscreen; Apple projects noremoteplayback. Other tokens remain browser hints |
| disablepictureinpicture, disableremoteplayback | false; disable native PiP or external playback as available |
| volume | 0–1, default 1; iPhone hardware volume remains system-owned |
| playbackRate | positive playback speed; this implementation supports 0.25–4 on both hosts |
| preservesPitch | true; native spectral/varispeed audio time pitch |
| currentTime | seconds; seek on a changed assignment, not on every unrelated commit; pending seek waits for metadata |
| paused | optional Exact writable projection of DOM's read-only observation; changed true pauses, changed false requests play; unbound lets native controls own it |

Autoplay remains a request. Browsers can reject it. The sample is muted and
inline so it can start without surprise audio; rejected play reaches `error`.
An app observing play/pause may mirror those events into `paused`; equality
prevents a feedback seek or repeated play call. A loop is a seek: `seeking`,
`waiting`, `seeked`, `canplay`, `playing`, never `pause`, `play` or `ended`,
and `paused` stays false (Chrome 154).

**Off screen (Charlie, 2026-09-28, §7 B).** Chrome's rule for a `muted` video
its `autoplay` attribute started, with `paused` unbound: it plays only while
some of it (any positive area) is in the viewport, pausing out of view
(`pause`, `paused` true, the time held) and resuming on return (`play`,
`playing`); one that loads out of view starts when first seen. A pause or play
the rule did not ask for (native controls, the end of a video without `loop`)
ends it, as HTML's `pause()` and `play()` clear the element's can-autoplay
flag; so does binding `paused`, since the web glue then calls `play()`. The
web is Chrome's own; Apple checks view geometry through the same host as
`playbackVisibilityThreshold` below (`OffscreenAutoplay`,
`VideoModule.swift`). With `paused` bound, a video plays on out of view on
every host unless `playbackVisibilityThreshold` says otherwise.

### Visibility-controlled inline playback (Shop, 2026-09-19)

Shop product media needs to suspend a playing inline video as its gallery leaves
view, then resume the same item on return. `playbackVisibilityThreshold` is an
explicit Exact media policy, not an HTML attribute or a global video default.
With an authored `paused` binding, a finite value in 0–1 temporarily overrides
that request to paused when the positive rectangular intersection area divided
by the video box area falls below the threshold. Zero admits any positive area.
Removing the property restores the authored request. A manual `paused=true`
always wins, and the item and playback position remain retained. Without an
explicit `paused` binding the policy is inactive, so native controls remain the
playback owner. Literal out-of-range values are refused by the compiler.

The browser uses IntersectionObserver with an implicit viewport root and
threshold notifications. Apple checks actual view coordinates against the window
and clipping ancestors, including nested scrollports. Native scroll, layout,
mount and completed-batch notifications coalesce on the main queue; the host
tracks only opted-in videos and sends no app scroll action or polling frame loop.
Only a changed effective pause request reaches the player. Retirement clears the
native weak registration and disconnects the browser observer. Existing play and
pause events report actual engine state so the app can label its custom control.
This is rectangular intersection, not sibling occlusion or pixel visibility;
PiP/background playback must use a different app policy.

Shop authors 0.5 based on source controls observed paused at low partial visibility
and resumed after returning; the exact source threshold is inferred, not extracted.
Web/iOS media-clock probes cover partial vertical clipping, frozen time, automatic
return, retained generation on Apple, manual pause, horizontal paging, covered
routes and unmount. The Swift clipping test exercises two axes, hide/show and
removal. Final direct-device evidence and broader check results belong in the
consumer checkpoint. Linux has no decoder, so it has no playback policy to apply.

### Native presentation and tuning

These are explicitly named host preferences, not invented HTML attributes.
The browser may ignore a preference it cannot express. Availability and user
policy still decide whether a requested feature is available.

| Property | Apple projection | Web projection |
|---|---|---|
| allowsPictureInPicturePlayback | AVKit, true unless disabled | inverse disablePictureInPicture |
| canStartPictureInPictureAutomaticallyFromInline | iOS AVKit, false | browser-owned |
| entersFullScreenWhenPlaybackBegins | iOS AVKit, otherwise derived from playsinline | browser-owned |
| exitsFullScreenWhenPlaybackEnds | iOS AVKit, false | browser-owned |
| requiresLinearPlayback | iOS AVKit, false | no equivalent |
| showsTimecodes | macOS AVPlayerView, false | no equivalent |
| allowsVideoFrameAnalysis | AVKit, true | browser-owned |
| preferredPeakBitRate | AVPlayerItem, 0 means automatic | browser-owned |
| preferredForwardBufferDuration | AVPlayerItem seconds, 0 means automatic | browser-owned |
| automaticallyWaitsToMinimizeStalling | AVPlayer, true | browser-owned |
| preventsDisplaySleepDuringVideoPlayback | AVPlayer, true | browser-owned |

Native AVKit controls supply fullscreen, scrubber, AirPlay and available
subtitle/audio-track choices. PiP and AirPlay need real-device verification;
setting the property is not evidence of a completed session. App audio-session,
background and remote-command policy are shared process capabilities; a leaf
must not steal them from another Exact session.

### Events and observations

Standard events: loadedmetadata, canplay, play, playing, pause, ended, waiting,
seeking, seeked, ratechange and volumechange (no action payload).
`timeupdate(seconds)` and `durationchange(seconds)` carry finite seconds;
unknown/indefinite duration is null in agent state and has no numeric action
payload; after a seek `currentTime` and the next `timeupdate` are the seek's
target, as HTML's official playback position is (AVPlayer reports its old time
until the seek lands; jukebox F20). `error(code)` carries a stable code, never
the engine's text (jukebox F6, 2026-10-04): MediaError's `aborted`, `network`,
`decode` and `src-not-supported` (any failure before metadata, as HTML's
dedicated media source failure), `not-allowed` for a `play()` the browser
refused, `invalid-value` for a number out of range. A play interrupted by a
pause or a new load (AbortError) is not an error. The text is in `state.media`.
A node the tree removed reports nothing more on any host, a late rejected play
or `timeupdate` included. Native events are useful playback
observations, not an assertion that AVFoundation reproduces HTML's complete
network-state/event ordering algorithm.

`state.media` reports each mounted video id, currentTime, duration, paused,
muted, readyState, videoWidth, videoHeight and renderer. Native generation
identifies source replacement. This is an observation from the engine, never a
second player model. The agent's eight operations do not change. `clock settle`
settles layout; it never seeks a real video. Playback checks observe the media
clock and use explicit pause/seek assignments when a stable frame is needed.

## 4. Keyboard consumer

The root declares `interactive-widget="resizes-content"`. A flex column contains
fixed-height heading and note composer, with `video flex=1 min-height=0` between
them. iOS's existing keyboardLayoutGuide path resizes the layout viewport inside
the keyboard transaction; the video receives a smaller box while the same
AVPlayer continues. Done moves focus to the Done button. Hiding the keyboard
restores the available box. No keyboard-height constant or timer sizes the video.

macOS has no software keyboard in this path; resizing the window exercises the
same flex allocation. Chrome uses its viewport policy. Safari's visualViewport is projected into
the root's available height under resizes-content, excluding pinch zoom. This
lives in shared viewport handling, not a special video keyboard callback.

## 5. Remaining complete-player requirements

These capabilities need a subsequent concrete consumer and are not silently
represented as working booleans:

- source/track child elements, external WebVTT and programmatic audio/subtitle
  selection; native embedded/HLS selections already belong to AVKit controls;
- explicit play/fullscreen/PiP command results and capability/error objects,
  including restoring the owning route after PiP;
- app-scoped AVAudioSession arbitration, interruption/route policy, background
  entitlement, lock-screen metadata and MPRemoteCommandCenter;
- authenticated media requests and cookies, FairPlay/EME licenses, offline
  downloads and cache budgets, live-edge/latency controls and diagnostics;
- adaptive source/quality selection, thumbnails, chapters and playlists;
- Linux decoder/presenter (the node has geometry but no playback there).

These are part of the complete design inventory, not part of the simple player's
implementation claim. DRM and background behavior cannot honestly be made
portable by copying an iOS property onto a DOM node.

### Complete-player extension design (unimplemented)

Keep three owners instead of putting every control on the leaf:

1. **Element:** HTML `source` and `track` children supply ordered MIME-typed
   alternatives and WebVTT captions/subtitles/descriptions/chapters. `default`,
   `kind`, `label` and `srclang` keep their HTML meanings. App-authored source
   alternatives are distinct from the quality variants inside an HLS manifest.
   Read-only track inventories carry stable engine ids; selection asks the
   owning engine and reports the actual selection. Caption accessibility follows
   the user's system preferences unless explicitly overridden.
2. **Media controller:** play, pause, load, seek, fullscreen and PiP requests
   address the mounted node id plus incarnation and return accepted/refused
   outcomes. DOM user-activation requirements and native presentation owners
   remain authoritative. A controller is invalid after unmount. `buffered`,
   `seekable`, `played`, `networkState`, `readyState`, `ended`, `seeking`,
   `currentSrc`, dimensions and error are observations. Ranges carry seconds;
   live duration is represented explicitly as indefinite, never serialized NaN.
   Fullscreen/PiP entry and exit are observations too. AVKit's restoration
   delegate returns to the owning session/route, with no reparenting of another
   session's view. A playlist changes sources only after an observed end.
3. **Application media service:** one app-scoped audio-session arbiter owns
   category/mode/mixing/ducking, interruption handling and route changes.
   `backgroundPlayback` is an intent requiring the manifest's existing audio
   background mode, not a view-level entitlement switch. One selected session
   owns Now Playing metadata (title, artist, artwork, duration, elapsed time,
   rate) and remote play/pause/seek commands. A removed session releases that
   ownership. Download/cache and DRM are separate optional artifacts, loaded
   only for a source requiring them, never a core Cargo feature.

Authenticated source requests reference an app-owned request policy; they do
not put access tokens in Contract, agent state or asset URLs. FairPlay/EME
references a license provider at the data seam; native persistable content keys
and browser MediaKeySession remain separate capabilities. Offline playback
references a completed download with an explicit disk budget and expiry.
These are not arbitrary `headers` on the shared video prop table.

Native preferences additionally cover preferred maximum resolution,
peak bitrate, forward buffer, live offset, waiting policy, external playback
and display sleep. The existing properties above are the proven subset. A
capability record reports platform support for PiP/fullscreen/remote playback,
DRM type, offline storage and rate range; unsupported requests fail explicitly.
Telemetry reports stalls, dropped frames, observed bitrate and errors only when
requested. No per-frame delivery to Contract is introduced.

## 6. Verification

The standing five checks, plus a browser playback/resizing drive and native
macOS/iOS builds. The simulator drive must observe advancing currentTime,
keyboard visible and smaller video height, editor above the keyboard, unchanged
source generation, and restored height after Done. A malformed source must
produce an observable error. Unmount must stop the player. Evidence and exact
results are recorded in the app README after running.

Shop startup validation (2026-09-19): four alternating normal-timing simulator
launches per build reduce same-feed median exec-to-first-node-draw from 352.8
to 307.85 ms with the opted-out layer presentation. Home/product playback and
retained-player promotion probes pass; 216 Apple host tests pass. This is not
loaded-content readiness or physical-device/source-app performance evidence.
Direct Mac input could not reveal native transport controls in either the old
or new signed fixture; native controller selection alone is not a gesture pass.
Workspace build/test/Clippy were incomplete in filesystem-helper fixture bakes;
formatting, caps, boot and Apple platform builds pass. Detailed evidence lives
in the Shop consumer's `.evidence/video-startup-checkpoint.json`.

## 7. The video row's cost, and Charlie's rulings (2026-09-28)

A–C were proposed with these measurements and ruled on 2026-09-28: A and B
yes, C no (no pool). A and B are built (§2, §3); §7.4 was built before the
rulings. The Extra Heavy
feed (`~/bench/xheavy`, heavybench probe) made only of its video row (`video
… autoplay muted loop playsinline controls=false paused=…`), base origin/main
`5d177f3f`, "mine" = `perf/video-row` (§7.4), "layer" = the same build
launched with `allowsPictureInPicturePlayback=false allowsVideoFrameAnalysis=false`
on the video (A's default, before it was built), medians of two rounds of `fling` and `ladder` (2026-09-28):

| M1 iPad Pro 12.9" | fps | 24k pt/s | ladder 48k | late/s | CPU ms/s | main ms/s | peak MB |
|---|---|---|---|---|---|---|---|
| SwiftUI (AVPlayerLayer, AVPlayerLooper) | 104.7 | 42.4 | 19.1 | 4.3 | 437 | 242 | 34 |
| base | 96.1 | 12.7 | 13.4 | 6.9 | 846 | 442 | 88 |
| base, layer | 110.9 | 66.5 | 111 (one run) | 2.6 | 379 | 199 | 57 |
| mine | 100.1 | 14.7 | 14.6 | 3.9 | 772 | 430 | 98 |
| mine, layer | 112.8 | 77.4 | 117.0 | 1.9 | 335 | 178 | 56 |

| iPhone 13 Pro Max | fps | 24k pt/s | ladder 48k | late/s | CPU ms/s | main ms/s | peak MB |
|---|---|---|---|---|---|---|---|
| SwiftUI | 96.6 | 34.3 | 21.8 | 11.3 | 433 | 238 | 37 |
| base | 85.0 | 12.0 | 11.0 | 7.3 | 794 | 411 | 73 |
| base, layer | 107.9 | 59.3 | 53.9 | 4.8 | 420 | 233 | 75 |
| mine | 101.8 | 40.7 | 19.7 | 6.6 | 635 | 357 | 71 |
| mine, layer | 119.4 | 119.6 | 113.6 | 0.5 | 353 | 201 | 75 |

The 17-kind feed (one round, iPad): base 103.0 fps at 969 ms/s CPU, mine
102.2 at 959, mine with the layer 107.7 at 943, SwiftUI 107.7 at 622 (the
other kinds carry that difference). exact2's peak memory on the video feed
stays above SwiftUI's (56 against 34 MB on the iPad) on every variant.

As built (A and B on `perf/video-rulings`, launched as the app authors it, no
PiP or frame-analysis property; two rounds of `fling` and `ladder`,
2026-09-28, base and SwiftUI over all four rounds):

| | fps | 24k pt/s | ladder 48k | late/s | CPU ms/s | main ms/s | peak MB |
|---|---|---|---|---|---|---|---|
| iPad SwiftUI | 104.6 | 40.9 | 19.4 | 4.0 | 433 | 239 | 35 |
| iPad base | 96.6 | 12.4 | 12.4 | 6.4 | 839 | 443 | 89 |
| iPad as built | 111.1 | 66.8 | 115.9 | 2.2 | 330 | 175 | 55 |
| iPhone SwiftUI | 96.2 | 34.3 | 18.3 | 11.9 | 438 | 242 | 37 |
| iPhone base | 85.0 | 10.8 | 11.0 | 6.9 | 789 | 406 | 73 |
| iPhone as built | 119.5 | 120.0 | 116.6 | 0.5 | 351 | 196 | 75 |

The 17-kind feed on the iPad (one round): as built 105.9 fps at 947 ms/s
CPU, base 104.2 at 977, SwiftUI 107.8 at 625. The peak-memory gap is not the
row's: the video row grows the process as much as SwiftUI's does over a fling
(+13 against +12 MB on the iPad); exact2 starts 13–20 MB higher on every kind
of the feed (QUEUE).

### A. No controller for a video without `controls` — accepted

§2 made AVKit the default because PiP and frame analysis defaulted to true,
and "authors must opt out to use the layer". Accepted: on iOS a video without
`controls` presents the AVPlayerLayer unless the node sets a controller
feature explicitly: `allowsPictureInPicturePlayback=true`,
`allowsVideoFrameAnalysis=true`, `canStartPictureInPictureAutomaticallyFromInline`,
`entersFullScreenWhenPlaybackBegins` (or, as now, no `playsinline`),
`exitsFullScreenWhenPlaybackEnds` or `requiresLinearPlayback`. With
`controls`, nothing changes. Promotion to a controller when a feature is
turned on later, and "once installed, it remains", stay as written.

Why: Chrome's `<video>` without `controls` draws no UI, and without the
controls' button (or automatic start, off by default) PiP cannot be reached
anyway; the unset preferences buy a view controller, an `AVPlayerView` and
AVKit's interstitial coordinator per row. Profiled (Time Profiler, iPad,
fling): the controller path spends 374 ms/s of the main thread against the
layer's 151, 134 of it in `AVPlayerViewController` (68 in `AVPlayerView`'s
`layoutSubviews` as each row enters the window). Its CoreMedia threads run 129
ms/s against the layer's 26; 92 ms/s is `fpic_*` sync XPC (AVKit's
interstitial coordinator asking for the current item) that only the
controller brings.

What an author loses: on iOS a paused video without `controls` no longer
offers Live Text or subject lifting on its frame unless it sets
`allowsVideoFrameAnalysis=true`; Chrome's `<video>` offers neither.
`state.media.renderer` reads `AVPlayerLayer`. macOS keeps `AVPlayerView`
(its cost is not measured here).

### B. Chrome pauses an autoplaying muted video off screen — accepted

Measured in Chrome 154 (headless and headed, 2026-09-28): a `muted` video
started by its `autoplay` attribute pauses when scrolled wholly out of the
viewport (`pause`, `paused` true, the time held) and resumes on return
(`play`, `playing`). One started or restarted by `play()` — which is what the
web glue does whenever `paused` is bound — plays on. Native hosts play on in
both cases, so with `paused` unbound they differed from the oracle. Accepted:
Apple follows Chrome's rule for an unbound `paused` (§3). A later probe added:
any visible area counts as in view, and one loaded out of view does not start
until seen. Checked with one fixture (an autoplaying video pushed out of view
and back, a bound one beside it, one loaded out of view then revealed) on
Chrome, macOS and the iOS simulator: the same four phases on all three.
The feed binds `paused`, so the rule does not touch its numbers.

### C. Player reuse (LLP 1068 Q5): no pool, as recommended

On the layer path the part of a row's video a pool would keep (the
`AVPlayer`, its layer view, their teardown; LLP 1068 §4.6 keeps the item per
row) is about 10 ms/s (player creation 3.0, the layer view 4.6, detaching
both 2.6) of the layer path's 151 ms/s of main thread in the fling, and that
path already beats SwiftUI's. "One node incarnation owns one player" stands.

### 7.4 Built before the rulings (`perf/video-row`)

Parity fixes and cuts that change nothing an author sees in Chrome: a loop
is a seek (Chrome's seeking, waiting, seeked, canplay, playing; never pause,
play or ended), where the player used to pause and play, and an app mirroring
play/pause into `paused` (apps/video-player) stopped at the first loop;
`ratechange` only for a playback-rate change, not on every play and pause; a
setter only for a changed value (volumechange no longer fires on every
update); only handled events cross the ABI and the 4 Hz timeupdate runs only
while handled, as the web glue does; one parsed `AVURLAsset` per unchanged
local file; `src`/`poster` resolved only when they change.

Sources: [HTML media](https://html.spec.whatwg.org/multipage/media.html),
[AVPlayerViewController](https://developer.apple.com/documentation/avkit/avplayerviewcontroller),
[AVPlayerView](https://developer.apple.com/documentation/avkit/avplayerview).
