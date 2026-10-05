# LLP 1098: The media session

**Type:** RFC
**Status:** Draft (r1), for review. Admission is the orchestrator's under Charlie's 2026-10-04 delegation (§9 Q1).
**Systems:** Contract compiler (`metadata=` on `audio` and `video`, two compiler shapes, six events, two offsets, the test steps `tap … mediasession` and `expect mediasession`), kernel props (six), Plan (`EventKind`), Runner (the six events' payload and record, new `runner/src/runner/media_session.rs`), web output shared by both targets (`host/web/media-glue.js`), JS target (`host/web-js/media.js`), Apple (the video arm: new `host/apple/videoarm/NowPlaying.swift`; `VideoModule.swift`, `Agent.swift`, `build.mjs`; bake receipt), Linux and Windows (the record, no publication), the driver, conformance, docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-07, stage 2 on 2026-10-08, stage 3 on 2026-10-10–11 (after LLP 1096 stage 3), stage 4 on 2026-10-11 (§6)
**Amends:** LLP 1042 §3 ("remote-command policy … must not steal", made concrete in D5) and §5 (its Now Playing and remote-command half is this RFC; the offered take deletes the rest of its unimplemented design, §9 Q1); `rules/DEFERRED.md` (D13)
**Related:** LLP 1012 (the agent); LLP 1031 (sessions in one process); LLP 1038 D11 and LLP 1069.007 D4 (`tap` addressing something other than a press); LLP 1048.003 D1 (`head`, a page-wide fact declared in the tree); LLP 1056 §3 (event records); LLP 1088 D7.1 (handler arity messages); LLP 1096 D8 (`audio_session`, the one session owner). Diaries: `~/projects/x2apps/podcast/DIARY.md` (F13, Top 5 #1), `jukebox/DIARY.md` (the media-keys note under 13:18). External, read 2026-10-04: W3C Media Session (`navigator.mediaSession`: `metadata`, `MediaMetadata`, `playbackState`, `setActionHandler`, `MediaSessionActionDetails`, `setPositionState`); MediaPlayer's `MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`, `MPSkipIntervalCommand`, `MPChangePlaybackPositionCommandEvent`, `MPNowPlayingSession`; AVKit's `updatesNowPlayingInfoCenter`.

## Summary

An app that plays long audio cannot be the system's Now Playing app. The
media keys, the lock screen, Control Center, a headset's buttons and the
browser's media hub all talk to that app, and exact2 has no way to answer
them. The web has one API for it, the Media Session:

```js
navigator.mediaSession.metadata = new MediaMetadata({ title, artist, album, artwork });
navigator.mediaSession.setActionHandler("seekforward", ({ seekOffset }) => …);
navigator.mediaSession.setPositionState({ duration, position, playbackRate });
navigator.mediaSession.playbackState = "playing";
```

This RFC puts it on the media element, by the API's names:

```
audio src=ep.audio paused=paused currentTime=seekTo playbackRate=speed
  metadata=MediaMetadata(title=ep.title, artist=ep.showTitle, album=ep.showTitle, artwork=ep.image)
  seekbackwardOffset=15 seekforwardOffset=30
  seekbackward=skipBy(-1) seekforward=skipBy(1) seekto=seekAt
  previoustrack=restart nexttrack=playNext
  pause=hostPaused playing=hostPlaying timeupdate=ticked

action skipBy(sign: number, details: MediaSessionActionDetails)
  seek = max(0, pos + sign * details.seekOffset)
action seekAt(details: MediaSessionActionDetails)
  seek = details.seekTime
```

- **`metadata=`** is the API's `MediaMetadata`, a record the app builds. An
  element that carries one **claims** the media session.
- **The actions are the element's events,** named as `setActionHandler`
  names them, each offering `MediaSessionActionDetails` as its optional last
  parameter. A handler bound is an action offered; one not bound is not.
- **`play` and `pause` act on the element itself,** as the browsers' default
  actions do. The app hears the element's own `play` and `pause` events.
- **Position and playback state are the element's.** The host reads them from
  the player. The app binds nothing for them, and nothing commits at 4 Hz.
- **One claimant owns the session:** the one that most recently started
  playing (D5).

The web publishes through `navigator.mediaSession` in the media glue both
targets share. Apple publishes through `MPNowPlayingInfoCenter` and
`MPRemoteCommandCenter` in the video arm, which already owns the player.
Linux and Windows keep the record and publish nothing. The driver reads the
session in `state`, triggers an action with `tap … mediasession`, and a test
asserts it with `expect mediasession`.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `metadata=MediaMetadata(…)` on `audio` and `video` claims the session | podcast F13 | 1 |
| D2 | Six actions as the element's events; `MediaSessionActionDetails`; two offsets | podcast F13 | 1 |
| D3 | `play` and `pause` act on the element | jukebox | 2, 3 |
| D4 | Position and playback state are the element's | — | 2, 3 |
| D5 | One owner: the claimant that most recently started playing | — | 1–3 |
| D6 | The web: `media-glue.js` for both targets | podcast, jukebox | 2 |
| D7 | Apple: Now Playing in the video arm; AVKit's own publication | podcast F13 | 3 |
| D8 | iOS: a playback session and background audio, or the build refuses | podcast | 3 |
| D9 | Linux and Windows: the record, no publication; MPRIS deferred | — | 1 |
| D10 | The driver: `state.mediaSession`, `tap … mediasession`, `expect mediasession` | — | 1 |
| D11 | Compiler, plan and runner | — | 1 |
| D12 | Lean and difftest | — | 1 |
| D13 | `rules/DEFERRED.md` | — | 1 |
| D14 | Docs and adoption | podcast, jukebox | 2–4 |

## 1. Evidence

- **Podcast F13** (major, missing). The spec asked for media keys. Space and
  ←/→ work through `aria-keyshortcuts`. Media keys, lock-screen controls and
  artwork in Control Center do not. Top 5 #1: "Every audio app needs it and
  none can build it from Contract or a data module." It asks for the
  declaration on the `audio` node, with `nexttrack`/`seekforward` events.
- **Jukebox** (the 13:18 note). "Media keys / OS Now Playing: nothing in
  exact2 to bind." It also notes that Chrome's default action may pause the
  element directly, where the app would not see it.
- **A key handler cannot stand in.** `aria-keyshortcuts` admits printable
  keys, the named editing and arrow keys, and F1–F35
  (`host/web/input-glue.js:7`, `:54`); `MediaPlayPause` is none of them. A
  `key` handler hears a key only while the app has focus, and the system
  sends media keys to the Now Playing app, not the focused window.
- **LLP 1042 lists it as unbuilt.** §5: "lock-screen metadata and
  MPRemoteCommandCenter". §5's extension design: "One selected session owns
  Now Playing metadata (title, artist, artwork, duration, elapsed time, rate)
  and remote play/pause/seek commands. A removed session releases that
  ownership." §3: remote-command policy is a shared process capability; "a
  leaf must not steal [it] from another Exact session." `QUEUE.md` repeats
  it twice ("Video (LLP 1042) … complete the designed track/controller and
  app audio-session ownership APIs").
- **Today's code.**
  - `audio` and `video` are one node type, 12, `Video`
    (`kernel/tables/schema.json:54–58`); its props are kernel props (`volume`
    … `playbackVisibilityThreshold`, ids 94–110).
  - Its events are a contiguous run of `EventKind`, `loadedmetadata` through
    `canplay` (`plan/tables/format.json:623–636`). The runner decodes them
    from host kind 19 as `name\npayload` and refuses a name outside that run
    (`runner/src/runner/event.rs:512–524`). `event.rs` is 1,461 lines.
  - Both web targets share `host/web/media-glue.js` (83 lines). The wasm glue
    hands it every prop and handler (`glue.js:24–30`, `:499`); the JS target
    through `media.js` (`rt.js:627`, `:826`). Its event set is
    `media-glue.js:10`; the JS target's is `media.js:14`.
  - Apple's player is the video arm, `libexact_video.dylib`, loaded on demand
    (`VideoModule.swift:25–40`). ExactKit passes the node's props as JSON and
    the handled events as `exactListeners` (`VideoModule.swift:196–198`,
    `VideoArm.swift:220`); the arm's reports come back through
    `session.runtime.media` (`VideoModule.swift:268–273`). Its event set is
    `VideoModule.swift:96`.
  - **AVKit already publishes.** `AVPlayerView` (macOS, always the arm's
    presentation, `VideoArm.swift:62`) and `AVPlayerViewController` (iOS,
    with `controls`) default `updatesNowPlayingInfoCenter` to YES
    (`AVPlayerView.h:91–94`; iOS `AVPlayerViewController.h:136–139`,
    unavailable on tvOS). What a Mac's Now Playing shows today for an Exact
    `audio` is not measured; stage 3 measures it first.
  - Linux has no decoder or audio output. Its `state.media` reports each
    media node unavailable (`host/linux/src/agent.rs:197–209`).
- **The Media Session API, measured for r1** (Playwright 1.63; Chrome
  154.0.8037.98 through `channel: 'chrome'`, Firefox 155.0 and WebKit 26.6
  through Playwright's builds; a fresh context, headless, a page at
  `http://localhost:…/show/x`):
  - `navigator.mediaSession` exists in all three, `playbackState` starts
    `"none"`, `metadata` starts `null`.
  - `setActionHandler` accepts `play`, `pause`, `seekbackward`,
    `seekforward`, `seekto`, `previoustrack`, `nexttrack`, `stop` and
    `skipad` in all three.
  - **A relative artwork `src` resolves against the page.**
    `{src: "assets/art.png"}` read back as `http://localhost:…/show/assets/art.png`
    in all three: on a route, the wrong file. The glue must resolve it as the
    host resolves `src` (D6).
  - **`setPositionState`** throws `TypeError` in all three for a position
    greater than the duration, a `playbackRate` of 0, a `NaN` duration, or no
    duration. `duration: Infinity` is accepted by Chrome and refused by
    Firefox and WebKit. `setPositionState()` with no argument clears it in all
    three.

## 2. What the web and the platforms offer

- **Media Session** (W3C).
  - `metadata` is a `MediaMetadata` (`title`, `artist`, `album`, each a
    string, default `""`; `artwork`, a list of `MediaImage` `{src, sizes,
    type}`), or `null`.
  - `setActionHandler(action, handler)`. The handler receives
    `MediaSessionActionDetails`: `action`, and for the seeks `seekOffset`
    (seconds, may be absent: "the page should choose a sensible default"),
    `seekTime` and `fastSeek` (seekto).
  - `setPositionState({duration, playbackRate, position})`. The browser
    extrapolates the position by the rate while `playbackState` is
    `"playing"`.
  - `playbackState`: `"none"`, `"paused"` or `"playing"`. Unset, the browser
    infers it from its media elements.
  - The browser decides where it shows the session (Chrome's media hub,
    macOS's Now Playing for Safari and Chrome, Android's notification) and
    routes the hardware media keys to it.
- **MediaPlayer** (Apple; macOS 10.12.2+, iOS, tvOS).
  - `MPNowPlayingInfoCenter.default().nowPlayingInfo`: a dictionary (title,
    artist, album title, `MPMediaItemArtwork`, duration, elapsed time,
    playback rate, default rate, media type, live stream). The system
    extrapolates the elapsed time by the rate.
    `playbackState` "only applies on macOS, where playback state cannot be
    determined by the application's audio session", and "must be set every
    time the app begins or halts playback, otherwise remote control
    functionality may not work as expected" (`MPNowPlayingInfoCenter.h:64–70`).
    On iOS the system reads the audio session and the info's rate.
  - `MPRemoteCommandCenter.shared()`: `play`, `pause`, `togglePlayPause`
    (a Mac's play key), `stop`, `previousTrack`, `nextTrack`, `skipBackward`
    and `skipForward` (`MPSkipIntervalCommand`, whose `preferredIntervals`
    the lock screen shows as "15"), `changePlaybackPosition` (a scrubber).
    A command is offered while it is enabled and has a target. Its handler
    returns a status.
  - `MPNowPlayingSession` publishes from an `AVPlayer` by itself, but only
    on iOS 16+ and tvOS 14+; it is unavailable on macOS
    (`MPNowPlayingSession.h:19`, `:34`).
  - On iOS the lock screen shows the app only under a non-mixable
    `AVAudioSession` category (`.playback`), and the audio stops when the
    phone locks unless `UIBackgroundModes` includes `audio`.
- **Linux.** MPRIS is the desktop's media-session protocol, over the session
  D-Bus. The Linux host has no player behind it.

## 3. Decisions

### D1 — `metadata=MediaMetadata(…)` claims the session

`MediaMetadata` is a compiler shape, declared beside `ScrollEvent`
(`contract/types/src/selection.rs`):

```
MediaMetadata { title: string, artist: string, album: string, artwork: string }
```

- **It is the API's constructor, by its name and fields.** The app builds it
  as it builds any record (LLP 1035.005.000 D3), so every field is named once
  (`album=""` when there is none). `is_record_call` admits it as the one
  compiler shape an app may build (`contract/types/src/records.rs:11–13`
  today admits only declared shapes). An app's own `shape MediaMetadata` is
  `type-duplicate-shape` (`lib.rs:1105–1110`), as for `ScrollEvent`. A `fn`
  may return one.
- **`artwork` is one image's source,** not a list (D6, §9 Q8): a Contract
  list literal can only be empty, and one image serves every consumer. It is
  resolved as `src` is: an `https:` URL or an app asset path. An `app:/`
  artwork is not published, and `state` says so (`artworkError`, D10),
  until the media element takes an `app:/` source (podcast F19, §7).
- **`metadata=` is admitted on `audio` and `video` only**
  (`lower-attr-tag` elsewhere). It lowers to four kernel string props,
  `mediaTitle`, `mediaArtist`, `mediaAlbum` and `mediaArtwork`, each the
  field of the bound expression, as `head`'s `title` lowers to `headTitle`
  (LLP 1048.003 D1) and a bound `controls` lowers to two rows
  (`contract/lower/src/media.rs:90–127`). `contract vocab metadata` names it
  with its shape.
- **An element with `metadata=` is a claimant** for as long as it is
  mounted. It is a static fact of the element, not of a value: `""` fields
  still claim. An app that wants no session for an episode leaves the
  element unmounted, as podcast does when nothing is current.

Why the element, and not a component section or a command:

- The session is about a player, and the element is the player. Its position,
  rate and paused state are already the host's (D4), so the session needs
  nothing more from the app.
- A prop and an event use the paths every host already has: the props reach
  the media glue and the arm as they are; the events come back through host
  kind 19. A section would need a parser arm, a plan table and an evaluator
  of its own.
- It is what podcast asked for (Top 5 #1).

No tag is added. HTML has no element for the media session; this is an API,
and its shape here is props and events on HTML's own media element, as
`preservesPitch` and `playbackVisibilityThreshold` are (LLP 1042 §3).

### D2 — Six actions as the element's events

| Event | Platform action | Its `MediaSessionActionDetails` |
|---|---|---|
| `seekbackward` | web `seekbackward`; Apple `skipBackwardCommand` | `seekOffset`: the platform's, else `seekbackwardOffset` |
| `seekforward` | web `seekforward`; Apple `skipForwardCommand` | `seekOffset`: the platform's, else `seekforwardOffset` |
| `seekto` | web `seekto`; Apple `changePlaybackPositionCommand` | `seekTime`; `fastSeek` (always false on Apple) |
| `previoustrack` | web `previoustrack`; Apple `previousTrackCommand` | — |
| `nexttrack` | web `nexttrack`; Apple `nextTrackCommand` | — |
| `stop` | web `stop`; Apple `stopCommand` | — |

- **The names are `setActionHandler`'s.** None collides with an element event
  today (`docs/contract-grammar.md`'s table of 50). `play` and `pause` do
  collide, and are D3's.
- **The record.** `MediaSessionActionDetails` is a second compiler shape:

  ```
  MediaSessionActionDetails { action: string, seekOffset: number, seekTime: number, fastSeek: bool }
  ```

  `event_record` offers it for the six (`selection.rs:14–28`), so the action
  takes it as its last parameter or leaves it (LLP 1056 §3). An absent
  field is the default the API leaves to the page: `seekOffset` is the
  element's offset for that direction, `seekTime` is 0 outside `seekto`,
  `fastSeek` is false. An action that ignores the offset may bind
  `seekbackward=skip(-15)` and leave the record; the lock screen still shows
  the declared offset (below), so the two should agree. The guide says so
  (D14).
- **No positional payload.** `handler_payload` gives the six none
  (`contract/analyze/src/lib.rs:385–393`), so `handler_arity` counts only the
  record (`:400–412`). A wrong arity is refused with the declaration that
  fits (LLP 1088 D7.1, `payload.rs`).
- **The offsets.** `seekbackwardOffset` and `seekforwardOffset` are numbers,
  in seconds, default 10, admitted on `audio` and `video`. They are not the
  API's names: the web has no way to declare an offset, and Apple's lock
  screen shows `preferredIntervals` only if it is told one. They are named
  after the action and the details field they fill. A literal that is not
  finite and greater than 0 is `lower-attr-value` (`media.rs:27–42`). A
  computed one that is not is reported as the media's `error` with
  `invalid-value`, as a computed `volume` is (`media-glue.js:52`), and the
  default stands.
- **A handler offered is an action offered.** A host offers an action to the
  platform only while the owner (D5) binds its handler: the web sets that
  handler and leaves the others `null`, and Apple enables that command and
  disables the rest. So the lock screen shows a skip button only for an app
  that handles it.
- **Only a claimant may bind them.** One of the six, or an offset, on an
  element without `metadata=` is `lower-media-session`: "`seekforward=`
  belongs to a media session: give this `audio` a `metadata=`". The check is
  `media.rs`'s.
- **Not admitted:** `skipad`, `previousslide`/`nextslide`, the call actions
  (`togglemicrophone`, `togglecamera`, `hangup`), `enterpictureinpicture`,
  `voiceactivity`; Apple's `seekBackward`/`seekForward` (press-and-hold
  scanning, which the web does not have), rate, rating, like and bookmark
  commands (§7).

### D3 — `play` and `pause` act on the element

The platform's play and pause (and Apple's toggle, a Mac's play key) call the
owner's own `play()` and `pause()`. The app hears them as the element's
`play`, `playing` and `pause` events, as it hears native controls.

- **Why.** `play` and `pause` are already the element's events, reporting
  what the player did. A second meaning, "the platform asks", would be a
  second event under one name. A browser's default play and pause, for a
  page that sets no handler, act on the playing element (jukebox saw
  Chrome's; the default is the browser's, so it is not measured here), and
  this is that default made explicit: the glue sets `play` and `pause`
  handlers that call the element's methods, so every browser and Apple
  behave alike, and the driver can call the same handler (D10).
- **A bound `paused` is the app's to mirror.** With `paused=paused` bound, a
  remote pause leaves the app's `paused` false until its `pause` handler
  sets it, as with native controls today (LLP 1042 §3: "An app observing
  play/pause may mirror those events into `paused`"). Podcast already does
  (`pause=hostPaused playing=hostPlaying`). The pitfall is documented (D14).
- **A remote play or pause is a person's,** like native controls. It ends
  Chrome's off-screen autoplay rule for that element (LLP 1042 §3), and it
  wins over `playbackVisibilityThreshold` until the app's next change to
  `paused`.
- **A refused play** (the browser's autoplay policy) is the element's
  `error` with `not-allowed`, as for any play (`media-glue.js:20–22`).

### D4 — Position and playback state are the element's

The host publishes the owner's position and state from the player itself.
The app binds neither.

- **Playback state:** `"playing"` while the owner's player is not paused,
  `"paused"` while it is, `"none"` with no owner. The web sets
  `navigator.mediaSession.playbackState`; Apple sets the info's playback
  rate (0 while paused) everywhere, and `playbackState` where it applies
  (macOS, §2).
- **Position:** `{duration, position, playbackRate}` from the player's
  `duration`, `currentTime` and `playbackRate`. It is published at
  `loadedmetadata`, `durationchange`, `play`, `pause`, `ratechange`,
  `seeked` and `ended`, and when the owner changes; never at `timeupdate`,
  since the platform extrapolates.
  - The position is clamped to `[0, duration]`, which all three engines
    require (§1).
  - The rate published is the element's `playbackRate`, never 0 (all three
    refuse 0); a pause is the playback state's.
  - **An unknown or infinite duration publishes no position**
    (`setPositionState()`; Apple removes the elapsed time and sets
    `MPNowPlayingInfoPropertyIsLiveStream` for an infinite one). Chrome alone
    accepts `Infinity` (§1), so one rule serves all three.
- **Why not bind it.** A bound position would commit at `timeupdate`'s 4 Hz
  and drift between commits; the player's own clock is exact and is already
  what `state.media` reports (LLP 1042 §3). The web's API lets a page set
  `playbackState` for playback with no element (Web Audio); Exact's only
  such playback is LLP 1096's short sounds, which want no session (§7).

### D5 — One owner: the claimant that most recently started playing

A page has one media session, and so does an app process on Apple. Of the
mounted claimants (D1), the **owner** is:

1. the one whose player most recently reported `play` (HTML's: `paused`
   became false);
2. if none has played, the most recently mounted, and of those mounted in
   one commit, the later in document order.

When the owner unmounts, the next by the same order takes the session. With
no claimant, the session is cleared: the web sets `metadata` to `null`,
every handler to `null`, `playbackState` to `"none"` and clears the
position; Apple sets `nowPlayingInfo` to `nil`, disables every command it
enabled, and on macOS sets `playbackState` to `.stopped`.

- **Why play recency.** It is the platforms' own rule: iOS gives Now Playing
  to the app that most recently played, and a browser's active session
  follows the media that most recently started. A podcast app with a trailer
  `video` that also declares metadata hands the lock screen to the trailer
  while it plays, and the episode keeps it otherwise if it played last.
  Document order (`head`'s rule, LLP 1048.003 D1) would leave Now Playing on
  whichever element is later in the tree, even paused.
- **Across Exact sessions in one process** (LLP 1031's sample host), the
  same rule runs over every session's claimants, because the coordinator is
  the arm's, and the arm is one image per process (D7). This is LLP 1042
  §3's "must not steal": a session takes the system's session only by
  playing.
- **An embedder's own player** (LLP 1031) is outside Exact. An embedded app
  with a claimant takes Now Playing when it plays, as any player in the
  process would. An embedder that does not want that embeds an app without
  one.
- **A non-claimant** is never the owner, playing or not. On the web the
  browser may still give a playing non-claimant its default session while no
  claimant is mounted; that is the browser's own (§2) and unchanged. On
  Apple, AVKit's (D7).

### D6 — The web: `media-glue.js` for both targets

The session lives in `host/web/media-glue.js`, the file both targets already
load after first paint for any media element. Neither boot path changes
(`scripts/boot.mjs`), and a page without media loads nothing.

- **Claims.** `update(el)` (`media-glue.js:42–61`) reads `mediaTitle` …
  `mediaArtwork` and the offsets from `el.exactMedia.props`, and the
  handlers from `el.exactMedia.handlers`. An element with any of the four
  metadata props is a claimant. `installMedia` records its mount; a `play`
  event its play time; `removeMedia` drops it. Each change re-runs the owner
  rule (D5).
- **Publishing the owner** (only the owner's values reach
  `navigator.mediaSession`):
  - `metadata = new MediaMetadata({title, artist, album, artwork: [{src}]})`,
    with `src` resolved as the element's own `src` is: the wasm target's
    `localAssetURL` (`glue.js:185`, `:443`) and the JS target's `rel`
    (`rt.js:551`). Each target gives the glue `mediaArtwork` already
    resolved, by adding the name to the line that resolves `src` and
    `poster` (`glue.js:443`; `rt.js:551`). A relative `src` handed to
    `MediaMetadata` would resolve against the route (§1).
  - `setActionHandler` for `play` and `pause` (D3), and for each of the six
    the owner handles; `null` for the others. A handler emits the event
    through the element's own `send`, with the payload `seekOffset seekTime
    fastSeek` (`"15 0 0"`), filled per D2.
  - `playbackState` and `setPositionState` per D4, from the media events the
    glue already listens to (`media-glue.js:71–75`).
  - Only when the session's values change; a `timeupdate` sets nothing.
- **A retired element publishes nothing more** (`state.retired`, jukebox F6,
  F20). A handler that fires for an element retired since is dropped.
- **Under the driver** the glue publishes as it does outside it: the browser
  is the driver's own, so nothing outside the drive sees it, and the
  readback in `state` is the API's (D10).
- **The JS target** adds the six names to `MEDIA_EVENTS` (`media.js:14`) and
  hands their handlers the record as a trailing argument, as `rt.js:847`
  hands `scroll`'s: `[action, seekOffset, seekTime, fastSeek]`
  (`media.js:65`).
- **`glue.js` is 1,499 lines.** Its two touches ride existing lines: the
  `mediaArtwork` name on `:443`, and the `state` overlay's session on the
  `st.media` line (`:1109`). The driver's trigger does not pass through
  `glue.js` (D10). If a reviewer finds those lines unreadable, a helper
  moves out of `glue.js` first.

### D7 — Apple: Now Playing in the video arm

**Where.** The new `host/apple/videoarm/NowPlaying.swift` is compiled into
`libexact_video.dylib` beside `VideoArm.swift`, and imports MediaPlayer.
`build.mjs` builds the arm by rewriting the web arm's argument list
(`build.mjs:1008`); it gains the second source file and `-framework
MediaPlayer`. Nothing enters ExactKit's link graph, and an app with no media
node carries none of it (the bake's `loads`, `receipt.rs:541–549`).

**One coordinator per process.** `NowPlaying` is a static in the arm. Every
`VideoArm` of every session registers with it when its props carry
`mediaTitle` (a claimant), and leaves at `invalidate` (`VideoArm.swift:458`).
It runs D5 over them, from the arm's own `play` reports
(`timeStatusChanged`, `VideoArm.swift:205–216`).

**What it publishes for the owner.**
- `nowPlayingInfo`: `MPMediaItemPropertyTitle`, `Artist`, `AlbumTitle`;
  `MPMediaItemPropertyArtwork` once the image has loaded (the arm's poster
  loader, `loadPoster`, `VideoArm.swift:362`, from the source `VideoModule`
  resolved as it resolves `src` and `poster`, `VideoModule.swift:185–195`);
  `MPMediaItemPropertyPlaybackDuration`;
  `MPNowPlayingInfoPropertyElapsedPlaybackTime`; `PlaybackRate` (0 while
  paused); `DefaultPlaybackRate` (the element's `playbackRate`);
  `MediaType` (`.audio` for an `audio`, `.video` for a `video`);
  `IsLiveStream` for an infinite duration (D4). A newer artwork source
  supersedes one still loading; a failed one leaves none and is reported in
  `state` (D10).
- On macOS, `playbackState` at every play and pause (§2's header note).
- **Commands.** `play`, `pause` and `togglePlayPause` call the owner's
  `play()` and `pause()` (D3; `VideoArm.swift:341`). The six map as D2's
  table says; `skipBackwardCommand.preferredIntervals` and
  `skipForwardCommand.preferredIntervals` are the element's offsets. Each
  is enabled with a target only while the owner handles it. A handler
  returns `.success` once it has emitted the event, `.commandFailed` for an
  action the owner no longer handles, and `.noActionableNowPlayingItem` with
  no owner. `MPSkipIntervalCommandEvent.interval` is the `seekOffset`;
  `MPChangePlaybackPositionCommandEvent.positionTime` is the `seekTime`.
- The events reach the app as the arm's other reports do: `emit(name,
  payload:)`, then `session.runtime.media` (`VideoModule.swift:231–236`).
  `VideoView.events` (`VideoModule.swift:96`) gains the six, so they cross
  as `exactListeners`.

**AVKit's own publication.** While a claimant owns the session, every arm's
`AVPlayerView` (macOS) and `AVPlayerViewController` (iOS) has
`updatesNowPlayingInfoCenter = false`, so AVKit does not overwrite it. With
no claimant, AVKit's default stands: what an Exact `video` or `audio` does
today, unchanged, and measured in stage 3 (§1).

**Why not `MPNowPlayingSession`.** It publishes from the `AVPlayer` by
itself, but it is iOS 16+ and tvOS 14+ only, unavailable on macOS (§2). One
`MPNowPlayingInfoCenter` path serves macOS, iOS and tvOS.

**macOS.** No audio session: the app becomes the Now Playing app by
playing with commands enabled. The play key sends `togglePlayPause`; the
previous and next keys send the track commands, so an app that binds only
the seeks gets nothing from them. The guide says to bind both (D14).

**tvOS** builds the same file (the hourly tier 2). Its Siri Remote behaviour
is not claimed by this RFC.

### D8 — iOS: a playback session and background audio, or the build refuses

The lock screen and Control Center show an app only under a non-mixable
playback category, and its audio stops at the lock without the `audio`
background mode (§2). LLP 1096 D8 makes ExactKit's `AudioSession.swift` the
one category owner, with `app.json`'s `audio_session` choosing `.ambient`
(default) or `.playback`. This RFC adds:

- **The bake receipt says whether the plan has a claimant:**
  `receipt.rs` gains `"mediaSession": true` beside `loads` when a plan node
  binds `mediaTitle`.
- **`host/apple/build.mjs --ios` refuses** such a plan unless the manifest
  has `"audio_session": "playback"` and `host.ios.backgroundModes` includes
  `"audio"` (the key already exists: `build.mjs:404`,
  `scripts/app.schema.json:290–295`). The message names both lines to add:
  "a media session needs `\"audio_session\": \"playback\"` and
  `\"backgroundModes\": [\"audio\"]` in app.json on iOS: the lock screen shows
  only a playback session's media". macOS and the web refuse nothing.
- **LLP 1096 D8's `.playback` stays non-mixable** (no `.mixWithOthers`); a
  mixable session is never Now Playing.
- **Interruptions.** A call or a Siri request pauses the `AVPlayer`; the app
  hears `pause`. Resuming at the interruption's end is not automatic (§7).
  A route change that loses the headphones pauses it the same way.

The refusal, not an inferred default, because the background mode is an
App Store–reviewed declaration and changes the compatibility id
(`bake/src/compat.rs:334`); an app should state it (§9 Q5).

### D9 — Linux and Windows: the record, no publication

- `state.mediaSession` is reported (D10), computed by
  `host/linux/src/agent.rs` from the kernel rows it already walks for
  `state.media` (`:197–209`): the claimants' four props, their offsets and
  their handlers. With no player nothing plays, so the owner is the most
  recently mounted claimant (D5's second rule), `playbackState` is
  `"paused"`, there is no position, and `published` is `"none"`.
- The driver's trigger dispatches the six to the owner (D10). `play` and
  `pause` are refused `unavailable`, as the media node has no player.
- Windows runs through the Linux presenter and does the same.

**MPRIS is deferred, not decided against.** A Linux media session needs a
Linux player first: an MPRIS player that never plays would advertise
playback that does not happen. MPRIS also needs a session D-Bus, which the
DRM/KMS host's usual kiosk lacks. When a Linux player and a consumer exist,
a pure-Rust D-Bus client (zbus) keeps the host's "no system library" line,
as a separate artifact loaded on demand, never a feature on `exact-linux`
(§7). Windows's System Media Transport Controls wait for a Windows player.

### D10 — The driver: observe, trigger, assert

**Observe: `state.mediaSession`**, on every host:

```json
{
  "owner": 12, "testId": "audio",
  "claimants": [12],
  "metadata": { "title": "…", "artist": "…", "album": "…", "artwork": "assets/art.png" },
  "actions": ["nexttrack", "pause", "play", "previoustrack", "seekbackward", "seekforward", "seekto"],
  "seekOffsets": { "seekbackward": 15, "seekforward": 30 },
  "playbackState": "playing",
  "position": { "duration": 3180, "position": 600, "playbackRate": 1 },
  "published": "navigator.mediaSession",
  "readback": { "title": "…", "artist": "…", "album": "…", "artwork": ["https://…/assets/art.png"], "playbackState": "playing" }
}
```

- `owner` is `null` and the rest empty with no claimant. `metadata.artwork`
  is the authored source, so hosts agree; `readback` has what the platform
  holds. `artworkError` says why an artwork was not published (an `app:/`
  source, a failed load), when one was not.
- `published` is `"navigator.mediaSession"` on the web, inside the driver or
  out (D6); `"MPNowPlayingInfoCenter"` on Apple outside the driver;
  `"agent"` on Apple under the driver; `"none"` on Linux and Windows.
- **Apple publishes nothing under the driver.** A drive on a developer's
  Mac would otherwise take the Mac's Now Playing and its media keys from
  whatever they were listening to. ExactKit passes the arm a prop,
  `exactPublish: "false"`, under the driver (props, so the arm's five-symbol
  ABI does not change, `VideoModule.swift:34`). The coordinator then builds
  the same `nowPlayingInfo` and command set and reports them as `readback`
  without assigning them. An XCTest covers the assignment (§5).
- Where it is filled: the web glue's state overlays (`glue.js:1109`;
  `agent.js:374–380`, from `exact.mediaSession.state()` that
  `media-glue.js` exposes); `Agent.swift`'s `nativeSections` beside
  `"media"` (`:219`), from the coordinator's report through the arm's state
  message; Linux's `agent.rs` (D9).

**Trigger: `tap <target> mediasession <action> [<seconds>]`.** The target is
the media element, by its testId or id. The wire form is
`{"op":"tap","id":12,"mediaSession":"seekto","seconds":120}`.

- `<action>` is `play`, `pause` or one of the six. `seconds` is the
  `seekOffset` for a seek (absent: the element's own) and the `seekTime` for
  `seekto`, where it is required.
- **It calls the handler the platform would call:** the glue's registered
  function on the web (through `exact.mediaSession.act`, which the web
  carrier in `scripts/agent.mjs` evaluates in the page, as it evaluates
  `exact.agentSettled`, `agent.mjs:288`), the coordinator's command target
  on Apple, the presenter's dispatch on Linux. What follows is the real path.
- **It is refused,** with nothing dispatched, when the target is not the
  owner (`mediasession: "trailer" does not own the media session; "audio"
  does`), or when the owner does not offer the action (`mediasession:
  "audio" has no seekto`): the platform would show no such control.
- The reply carries `delivery: "substituted"`, as an answered device request
  does (LLP 1069.007 D4): the platform did not press it.
- This is `tap` addressing a target's other face, as `tap <root> history -1`
  does (LLP 1038 D11): a target form, no new operation. The driver's ten
  stay ten.

**Assert, in a test file:**

```ebnf
step = … | "tap" STRING "mediasession" IDENT [ NUMBER ] NL
         | "expect" "mediasession" ( FIELD "==" test-value
                                   | ( "has" | "missing" ) STRING ) NL ;
```

- `expect mediasession title == "Episode 12"` reads a field of
  `state.mediaSession`: `owner` (compared with the owner's testId), `title`,
  `artist`, `album`, `artwork`, `playbackState`.
- `expect mediasession has "seekto"` passes when the owner offers it;
  `missing` when it does not.

```contract-test
test "the lock screen's controls drive the player"
  tap "row-ep1"
  expect mediasession owner == "audio"
  expect mediasession title == "Episode 1"
  expect mediasession has "nexttrack"
  tap "audio" mediasession "seekforward"
  expect state seek == 30
  tap "audio" mediasession "seekto" 600
  expect state seek == 600
  tap "audio" mediasession "nexttrack"
  expect mediasession title == "Episode 2"
```

The steps reach four places: the parser (`contract/syntax/src/parser/steps.rs`,
574 lines: a `TapForm::MediaSession { action, seconds }` beside `into`,
`:250–258`, and the `expect` arm), the step's JSON encoding
(`contract/cli/src/lib.rs`, `Step::Tap`, `:483`), the driver
(`scripts/agent-test.mjs:123–125`), and difftest (D12).

### D11 — Compiler, plan and runner

- **Kernel** (`kernel/tables/schema.json`, the one declaration authority):
  props `mediaTitle`, `mediaArtist`, `mediaAlbum`, `mediaArtwork` (`str`),
  `seekbackwardOffset`, `seekforwardOffset` (`float`), appended after the
  last id.
- **Plan** (`plan/tables/format.json`): `EventKind` gains the six, appended
  after `drop`. `FORMAT_DIGEST` changes; every in-repo plan rebuilds.
- **Types:** the two compiler shapes and `event_record` (`selection.rs`, 165
  lines); `is_record_call` (`records.rs`); the six handlers typed as the
  other media handlers are (`component.rs:506`, `:569`).
- **Analysis:** the six join the handler list (`contract/analyze/src/lib.rs`,
  after `canplay`, `:370`); no positional payload (D2).
- **Lowering:** `tags.rs` (1,368 lines) gains six handler arms
  (`:360–373`'s run), the `metadata` target that lowers to four props, and
  the two offsets; `media.rs` gains `lower-media-session` and the offsets'
  ranges.
- **Runner.** The new `runner/src/runner/media_session.rs` decodes the six
  payloads (`seekOffset seekTime fastSeek`: three finite numbers, the first
  two ≥ 0, the last 0 or 1) and builds the record. `event.rs` (1,461 lines)
  gains only call sites: `media_payload`'s range check admits the six
  (`:515`), and `Event::record` offers their record (`:433`). No runner
  state: ownership, publication and the readback are the hosts'.
- **The web wasm host** accepts the six through host kind 19 unchanged.

### D12 — Lean and difftest

- Lean models no media prop or event today (`semantics/` has no
  `timeupdate`), and gains none: `metadata=` and the six are host-facing.
  Stage 1 confirms `contract lean` passes podcast's program through, as it
  passes its `timeupdate` handler.
- difftest delivers a `tap` only as a press (`semantics/difftest/src/script.rs:131–137`).
  A test with `tap … mediasession` therefore reaches its `other` arm and is
  skipped by name (`verify.rs:80`), which is right: the run cannot deliver
  the step, and the state after it would differ.
- `expect mediasession` is an assertion on host state, which the oracle
  lacks. It gets a `continue` arm, as LLP 1096 D10 gave `expect sound`, so a
  test that only asserts the session still runs.
- The corpus gains nothing: a corpus case must be fully delivered.

### D13 — `rules/DEFERRED.md`

Proposed for the orchestrator, under **Components** beside LLP 1096's entry:

> **Expanded (LLP 1098; admitted by the orchestrator under Charlie's
> 2026-10-04 delegation, "make decisions without me"):** the media session,
> by the Media Session API's names, on HTML's media element: `metadata=
> MediaMetadata(title=, artist=, album=, artwork=)` on an `audio` or `video`
> claims it; `seekbackward`, `seekforward`, `seekto`, `previoustrack`,
> `nexttrack` and `stop` are that element's events, with
> `MediaSessionActionDetails`; the platform's play and pause act on the
> element; position and playback state are the player's. The web publishes
> through `navigator.mediaSession`, Apple through `MPNowPlayingInfoCenter`
> and `MPRemoteCommandCenter` in the video arm; Linux and Windows keep the
> record. On iOS it needs `audio_session: "playback"` and the `audio`
> background mode.
>
> - **Consumers:** podcast (F13, Top 5 #1), jukebox (media keys).
> - **Unblocks:** media keys, Now Playing, the lock screen and Control
>   Center, a headset's buttons, the browser's media hub.
> - **Take (offered in LLP 1098 §9 Q1):** LLP 1042 §5's "Complete-player
>   extension design (unimplemented)" is deleted, a spec with no implementer
>   or date; its Now Playing and remote-command half is this RFC, and the
>   rest (source and track children, the media controller, PiP and
>   fullscreen results, DRM, downloads) returns with a consumer. `QUEUE.md`'s
>   two "Video (LLP 1042)" lines lose "complete the designed
>   track/controller and app audio-session ownership APIs".
> - **Still out:** the other actions (`skipad`, slides, calls, picture in
>   picture), Apple's scanning, rate, rating and like commands, an
>   app-set `playbackState` or position, more than one artwork size,
>   `app:/` artwork, resuming after an interruption, MPRIS on Linux and
>   Windows's transport controls (LLP 1098 §7).

No tag is added, and the **Components** count is unchanged.

### D14 — Docs and adoption

**Docs.**
- `contract-for-agents.md`: a "Now Playing and media keys" recipe from the
  Summary: `metadata=`, the offsets beside their seeks, `previoustrack` and
  `nexttrack` for a Mac's keys, mirroring `play` and `pause` into `paused`.
- `contract-grammar.md`: `metadata=` and the two shapes; the six events in
  the events table (the record row); the two test steps.
- `reference.md`: a host row (web and Apple publish, Linux and Windows
  record), and the iOS manifest lines of D8.
- `agent-pitfalls.md`:
  - "a remote pause leaves a bound `paused` false unless the app mirrors the
    element's `pause` event";
  - "the lock screen shows `seekbackwardOffset`, not the number in your
    action: keep them equal, or take the details' `seekOffset`";
  - "on iOS a media session needs `audio_session: \"playback\"` and the
    `audio` background mode".
- LLP 1042 §5 gains a pointer here (and loses its design, if Q1's take is
  accepted).

**Adoption** (outside the repo, `EXACT_APP_DIR`).
- **Podcast** (stage 2 on the web, stage 3 on macOS and the iOS simulator):
  the Summary's lines on its one `audio` (`app.contract:458–461`), with
  `previoustrack` restarting the episode and `nexttrack` playing Up Next;
  the iOS manifest lines. Its tests gain the Summary's
  `expect mediasession` test. F13 is closed by pressing a Mac's media keys
  and the iPhone simulator's Control Center by hand.
- **Jukebox** (stage 2, 3): `metadata=` from the current song (title,
  artist, album, its art), `previoustrack` and `nexttrack` from its queue,
  `seekto` from its scrubber.

## 4. Effect on each implementation

Every file stays under 1,500 lines: new behaviour goes in new files, and
files near the cap gain only call sites.

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| syntax | `TapForm::MediaSession` and `expect mediasession` in `parser/steps.rs` (574) | — | — |
| types, analyze, lower | two compiler shapes, `event_record`, `is_record_call`; six handlers (`lib.rs`, `component.rs`); `metadata` and the offsets in `tags.rs` (1,368); `lower-media-session` in `media.rs` | — | — |
| kernel, plan | six props; six `EventKind`s | — | — |
| runner | `media_session.rs`; two call sites in `event.rs` (1,461) | — | — |
| web, both targets | — | the session in `media-glue.js` (83); `mediaArtwork` on `glue.js:443` and `rt.js:551`; the state overlay on `glue.js:1109` (1,499) and `agent.js` (390); six names and the record in `media.js` | — |
| Apple | — | — | `videoarm/NowPlaying.swift`; `VideoArm.swift` (495) registration, play reports, `updatesNowPlayingInfoCenter`, `exactPublish`; `VideoModule.swift` (313) events and `mediaArtwork`; `Agent.swift` (641) state and the trigger; `build.mjs` (1,441) the arm's second file and framework, the iOS refusal; `receipt.rs` `mediaSession` |
| Linux, Windows | `state.mediaSession` and the trigger in `agent.rs` (891) | — | — |
| driver, tests | `scripts/agent.mjs` (the `tap` form); `agent-test.mjs`; `contract/cli/src/lib.rs` (997); difftest's `continue` arm | the web carrier's `exact.mediaSession.act` | Apple's trigger |

## 5. Tests

- **Compiler** (`contract/cli/tests/it/media_session.rs`,
  `contract/corpus/rejects.txt`):
  - `metadata=MediaMetadata(…)` with literals, with state, and from a `fn`
    returning `MediaMetadata`, on `audio` and `video`;
  - refused: `metadata=` on a `box` (`lower-attr-tag`); a field missing
    from the record; an app `shape MediaMetadata` (`type-duplicate-shape`);
    `seekforward=` or `seekforwardOffset=` without `metadata=`
    (`lower-media-session`); `seekbackwardOffset=0` and `=-5`
    (`lower-attr-value`);
  - each of the six with an action of no parameters, of the record, and with
    a captured argument before the record; a wrong arity refused with the
    declaration that fits;
  - `tap "audio" mediasession "seekto" 600` and both `expect mediasession`
    forms parse and encode; `seekto` without seconds and an unknown action
    are refused by the parser.
- **Plans.** Every in-repo plan decodes with the changed digest.
- **Runner** (`runner/src/runner/media_session.rs` tests): each payload
  decodes to its record; a malformed, negative or non-finite one is refused
  as `invalid media event`; an action leaving the record runs; the six
  outside the media run are not admitted elsewhere.
- **Conformance** (`host/web-js/conformance/media-session.contract`,
  `.steps`): two claimants and a non-claimant, compared across the wasm and
  JS targets in Chrome (the oracle), Firefox and WebKit: the owner by mount,
  then by play; the readback of metadata, an artwork resolved from a route,
  the action list; `seekforward` with and without seconds, `seekto`,
  `nexttrack`, `stop` landing in state; a non-owner's and an unhandled
  action's refusals; unmounting the owner hands the session on; the last
  unmount clears it (`metadata` `null`, `playbackState` `"none"`).
- **Web glue** (`host/web/tests/media-session.test.mjs`, three engines, the
  async lane): `setPositionState` is never called with a position past the
  duration, a rate of 0, or a non-finite duration (§1's throws), and is
  cleared for a live source; `playbackState` follows `play` and `pause`; a
  retired owner publishes nothing.
- **Linux.** Podcast's media-session test under `agent.mjs linux`, its
  remote `play` refused `unavailable`.
- **Apple.**
  - An XCTest (`build.mjs --test`, macOS; `--ios` for the simulator) drives
    the coordinator with publication on: `MPNowPlayingInfoCenter.default()
    .nowPlayingInfo` reads back the title, duration, elapsed time and rate;
    the handled commands are enabled with the declared `preferredIntervals`
    and the rest disabled; the owner moves on play and on removal; the last
    removal clears the info and disables every command.
  - Under the driver, `published: "agent"`, and the same readback.
  - `build.mjs --ios` refuses podcast without D8's two manifest lines, with
    the message, and builds it with them.
  - **By hand, stage 3's exit:** on a Mac, the play, previous and next keys
    and the menu bar's Now Playing (title, artwork, scrubber, skip
    intervals); on the iOS simulator, Control Center; on an iPhone, the lock
    screen and a headset's button. Before any of it, what AVKit publishes
    today for an Exact `audio` on macOS, recorded in §1.
- **Driven.** D14's adoptions, and the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks.

1. **2026-10-07, the model** (D1, D2, D9–D12, D13's record): kernel, plan,
   compiler, runner, Linux and Windows, the driver's forms and steps.
   - **Exit:** §5's compiler, runner and Linux tests; the plans decode.
2. **2026-10-08, the web** (D3–D6) and the web adoptions (D14).
   - **Exit:** conformance and the glue test in three engines; podcast's and
     jukebox's tests on the web; Chrome's media hub and macOS's Now Playing
     for Chrome showing podcast's episode, its artwork and its skips, by
     hand.
3. **2026-10-10–11, Apple** (D3–D5, D7, D8), after LLP 1096 stage 3 lands
   `AudioSession.swift` and `audio_session`.
   - **Exit:** the XCTest, the iOS refusal, podcast on macOS and the iOS
     simulator by hand; podcast's tests under `test macos`.
   - If the iOS refusal waits on LLP 1096, macOS lands first and the iOS
     piece gets a `QUEUE.md` line.
4. **2026-10-11, docs** (D14), after stage 2's adoptions run.

## 7. Deferred, with preconditions

- **MPRIS (Linux).** Needs a Linux player and a consumer that ships audio on
  Linux; a separate artifact over a pure-Rust D-Bus client.
- **Windows's System Media Transport Controls.** Needs a Windows player.
- **Other actions** (`skipad`, `previousslide`/`nextslide`, the call
  actions, `enterpictureinpicture`; Apple's scanning, rate, rating, like and
  bookmark commands). Needs a consumer: an ad-supported player, a slide
  deck presented to a TV, a call app.
- **An app-set `playbackState` or position.** Needs playback with no media
  element that wants a session (LLP 1096's sounds do not).
- **Several artwork sizes** (`MediaImage` lists). Needs a list literal or a
  consumer whose one image looks wrong somewhere.
- **`app:/` artwork,** with `app:/` media (podcast F19). Needs the media
  element to take an `app:/` source first.
- **Resuming after an interruption** (`AVAudioSession`'s `.shouldResume`).
  Needs a consumer that wants it; the app can already resume from `pause`.
- **Chapters, queue position and count** (`MPNowPlayingInfoPropertyChapter…`,
  `PlaybackQueueIndex`). Needs a consumer.

## 8. Considered, not taken

- **A component section** (`mediaSession` with `metadata` and handler lines,
  mirroring `navigator.mediaSession` as a page-wide object). It needs a
  parser arm, a plan table and an evaluator, and still has to name a player
  for the position. The element is the player (D1).
- **A new `mediasession` tag in the tree,** as `head` is. Not HTML, so not
  admitted (DEFERRED's Components rule).
- **Imperative commands** (`setMediaMetadata(…)`, `setPositionState(…)`).
  The app would have to reissue them at every change; a binding already does
  that, and the position would be the app's guess.
- **Flat props** (`mediaTitle=`, `mediaArtist=` …) in place of
  `metadata=MediaMetadata(…)`. Simpler for the compiler and each optional,
  but names the web does not have; `title` itself is HTML's global
  tooltip and cannot be reused (§9 Q2).
- **`play` and `pause` as app requests** under other names. A second
  meaning beside the element's own events, and an app that forgets the
  handler gets a dead play button on the lock screen (D3).
- **Media keys as `aria-keyshortcuts`** (`MediaPlayPause`). A key handler
  hears only the focused window; the system sends media keys to the Now
  Playing app (§1).
- **`MPNowPlayingSession`** (D7): iOS and tvOS only.
- **Document order for the owner** (`head`'s rule). It ignores which player
  the person last started (D5).
- **Publishing on Apple under the driver.** It would take a developer's
  media keys during every drive (D10).
- **Inferring the iOS session and background mode from a claimant** (D8).
  It would add an App Store–reviewed declaration the manifest never states.

## 9. Open questions, with the author's recommendations

1. **The admission and its take.** Recommended take: delete LLP 1042 §5's
   "Complete-player extension design (unimplemented)" — about 40 lines of
   design with no implementer or date, which RULES.md §Scope says should not
   be written — keeping a pointer here for its Now Playing half, and drop
   the matching clause from `QUEUE.md`'s two "Video (LLP 1042)" lines. As
   with LLP 1096's take, this may not count as doing-list work; the fallback
   is the orchestrator's waiver under the same delegation.
2. **`metadata=MediaMetadata(…)` or flat props.** Recommended the record:
   the API's names exactly, one attribute that makes the claim, a `fn` can
   build it. Its costs are one compiler shape an app may construct and
   `album=""` when there is no album. Flat props (`mediaTitle=` …) are the
   alternative if the orchestrator wants no constructible compiler shape.
3. **The owner.** Recommended play recency, then mount order (D5), as the
   platforms do. The alternative is `head`'s document order.
4. **`play` and `pause`.** Recommended: they act on the element, and the app
   hears the element's events (D3). The alternative is app handlers under
   new names.
5. **iOS.** Recommended: the build refuses a claimant without
   `audio_session: "playback"` and the `audio` background mode (D8). The
   alternative is to infer both.
6. **The offsets' default.** Recommended 10 s, with podcast setting 15 and
   30. The alternative is to require them on a claimant that binds a seek.
7. **Publication under the driver.** Recommended: the web publishes (its
   browser is the driver's), Apple does not (D10).
8. **`artwork`.** Recommended one source string, resolved as `src`; a list
   of sizes deferred (§7).
9. **Linux.** Recommended the record only, MPRIS deferred behind a Linux
   player (D9).
10. **The trigger's spelling.** Recommended `tap <element> mediasession
    <action> [<seconds>]` (D10), which also tests ownership by refusing a
    non-owner. The alternative is a target-free `tap @mediasession <action>`,
    which would share `@`'s namespace with held requests (LLP 1069.007 D4).

## 10. Revisions

- **r1** (2026-10-04): first draft.
