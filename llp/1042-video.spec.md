# LLP 1042: Video — browser semantics, native playback

**Type:** Spec
**Status:** Draft
**Systems:** Kernel replaced elements; Contract media attributes/events; web video; Apple AVKit; keyboard viewport; Video Player app
**Author:** Codex for Charlie Cheever
**Implementer:** Codex, 2026-09-18
**Date:** 2026-09-18
**Related:** LLP 1001 (CSS defaults), 1008 §9 (keyboard viewport), 1011 (intrinsic size), 1012 (agent operations), 1020 (optional native artifact), 1031 (session lifetime), 1039 (viewport facts)

## 1. Consumer and scope trade

Charlie requested a web-based video component with iOS/macOS options, then a
simple player that shrinks for the software keyboard. `apps/video-player` is
that consumer. Video comes off NOT-DOING; router/viewport-fact integration
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
an AVPlayerLayer-backed view (Shop, 2026-09-19). Default PiP and frame-analysis
preferences still select AVKit; authors must opt out to use the layer. The first
presentation waits for source props instead of constructing a controller before
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
prevents a feedback seek or repeated play call.

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
payload. `error(message)` carries a string. Native events are useful playback
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

Sources: [HTML media](https://html.spec.whatwg.org/multipage/media.html),
[AVPlayerViewController](https://developer.apple.com/documentation/avkit/avplayerviewcontroller),
[AVPlayerView](https://developer.apple.com/documentation/avkit/avplayerview).
