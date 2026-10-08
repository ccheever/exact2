# LLP 1114: What the Bluesky clone asks of Exact2

**Type:** RFC (a decision brief: each sub-LLP proposes, Charlie decides)
**Status:** Draft, 2026-10-08. Not reviewed.
**Systems:** the capabilities its sub-LLPs name: `exactBodyFrom` and a video transcode (1114.000); `showNotification` and a scheduled wake (1114.001); the file input's `capture` (1114.002); `compressImage` (1114.003); `openURL` (1114.004); `video` captions and the picker's `accept` (1114.005); the manifest's `share_target` and Apple's first extension target (1114.006); `textarea` highlights (1114.007); `aria-actions` (1114.008); the `net.fetch` grant's grammar (1114.009)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-08
**Implementer:** unassigned until ruled; each admitted sub-LLP gets its own lane
**Related:** LLP 1054 and LLP 1054.000 (the port's first round, 2026-09-26: its field report and its asks; 1054.000 R5 admitted `https://*.host.bsky.network`); LLP 1109 (the app farm's brief, whose shape this copies); LLP 1069 and its sub-LLPs (the Expo survey and the device capabilities these extend: 1069.002 the picker, 1069.003 share, 1069.006 the auth session, 1069.007 and 1069.009 device inputs in the agent's hands, 1069.008 grants); LLP 1038 D8 (an incoming URL as a location); LLP 1010 §6.2 (list anchoring); `rules/DEFERRED.md`; the clone builder's request, `exact2-asks-for-charlie.md`, and its `gap-audit-2026-10-08.md` (group 10), in the `bluesky-clone-builds` Tuft project ([preview](https://router.tuft.dev/preview/3011de219bcaa9bb/f62dc3d800ee4e17b288aa1737a9f974))

## 0. Summary

The Bluesky clone (`dev.tuft.blueskyclone`) is an exact2 app built by an agent,
bsky4, against Bluesky's own `social-app`. Its gap audit of 2026-10-08 sorted
what the clone lacks into groups. Items 1–9 need nothing from exact2, and bsky4
is doing them. Group 10 is what exact2 lacks. bsky4 wrote that group up as eight
asks and eight "worth a quick check" gaps, ordered by user-visible value per effort.

This is the port's second round. The first was LLP 1054, the field report from the
same app's overnight build on 2026-09-26, and LLP 1054.000, its asks. Those were
about the language, layout and the data seam. These are about device and platform
capability, and only one overlaps: 1114.009 widens the PDS reach 1054.000 R5 admitted.

Each ask was checked against exact2 at `origin/main` (`d81ea3013`) and against
Bluesky's sources. Two needed no new capability. Ten became the sub-LLPs below,
each a self-contained RFC with its own questions. Four were triaged here (§4).
Several of bsky4's claims were wrong in ways that change the design (§5). The
biggest is that Bluesky now uploads video in parts, so the "bodies over 64 MiB"
ask goes away.

| LLP | Ask (gap-audit item) | exact2 | Clone | Earlier decision it moves |
| --- | --- | --- | --- | --- |
| [1114.000](1114.000-video-upload.rfc.md) | Video upload (57): a file's byte range as a body; progress; `compressVideo` | stage 1 **S**; stage 3 **L** | ~1 day | 1069.002 A1.8 (stage 3 only, as Amendment A3) |
| [1114.001](1114.001-background-refresh-and-notification-taps.rfc.md) | Push (90): a notification tap opens a route; a scheduled background wake | stage 1 **S**; stage 2 **M–L** | ½ day | LLP 1069 §5 "Not now" (background tasks; a DEFERRED admission) |
| [1114.002](1114.002-camera-capture.rfc.md) | Camera (67): HTML's `capture` on the file input | **S–M** | S | `DEFERRED.md:175` "no `capture` on the picker" (Charlie, 2026-10-02) |
| [1114.003](1114.003-image-crop.rfc.md) | Crop (66, 72): a `crop` rectangle on `compressImage` | **S–M** | M | 1069.002's "D6 reopened" (Charlie, 2026-10-08) keeps cropping out; Amendment A2 |
| [1114.004](1114.004-in-app-browser.rfc.md) | In-app browser (49): `openURL(url, popup=true)` | **S** | S | recommendations in LLP 1069 §2.7, §5 and 1069.006; none ruled |
| [1114.005](1114.005-video-captions.rfc.md) | Captions (55): the stream's own captions observed; `.vtt` in the picker; later `<track>` | stage 1 **S**; `<track>` **M** | S | `<track>` only: the LLP 1098 line on DEFERRED (recommended: not yet) |
| [1114.006](1114.006-receiving-shares.rfc.md) | Share extension (89): the manifest's `share_target`, arriving as a location | **L** | S | none; the first extension target |
| [1114.007](1114.007-highlights-in-a-text-field.rfc.md) | Styled runs in the composer (60): `textarea` highlights, as CSS Custom Highlights | **M** | S | none |
| [1114.008](1114.008-accessibility-actions.rfc.md) | Accessibility custom actions (96): ARIA's `aria-actions` | **S–M** | S | updates LLP 1057.000 §10.4 ("ARIA has no custom actions") |
| [1114.009](1114.009-origins-learned-at-run-time.rfc.md) | Arbitrary PDS hosts (86): an open grant, `net.fetch https://*` | **S** | S–M | the one-domain limit, LLP 1054.000 R5 and LLP 1016 D6's amendment (Charlie, 2026-09-26) |

## 1. Asked, but already possible

**The sign-up captcha (item 84).** bsky4 proposed iframe navigation events so
the app could read hCaptcha's redirect. That can't work and isn't needed. It
can't work because the web doesn't let a page read a cross-origin iframe's
location, `iframe` reports only `load` (with no URL) and `message`, and
watching an iframe's navigation is ruled out (`DEFERRED.md:452`, LLP 1020 §5).
Bluesky's captcha page also redirects to `bsky.app`, which the clone can't
claim. It isn't needed because AT Protocol OAuth has a sign-up mode: the
authorization request carries `prompt=create`, and the provider's own pages
take the handle, the password, the hCaptcha and consent, then return a code
for the new account through the ordinary callback. That is `openAuthSession`
(LLP 1069.006), built and tested on the web, macOS and iOS. A test request to
bsky.social's authorize endpoint returned `promptMode: "create"` and the same
hCaptcha site key Bluesky's own captcha page uses. No account was made.
The clone's work is S: a "Create account" button that adds `prompt=create`.
Not verified: a whole sign-up, which would create a real account, and whether
one fits the web popup's ten minutes. The agent reaches only fixtures unless
`EXACT_DEVICE=real` (LLP 1069.006 D7), so the clone's fixture provider has to
honour `prompt=create`.

**Prepend anchoring (item 2).** Rows added above the viewport keep the first
visible row where it was. `capture_anchor` and `restore_anchor`
(`runner/src/instance/collection/index.rs:569-639`) hold its key and offset
across a prepend or a measure, and the host applies the correction in the same
frame. The JS target has its own copy (`host/web-js/list.js:142-153`), and
`host/web-js/conformance/lists.contract:41` covers rows added by `reachstart`
(LLP 1010 §6.2). The clone's chat can use `reachstart` with
`scroll-start="end"` today. Inverted lists stay refused (`DEFERRED.md:542`), and
`overflow-anchor: none` isn't built (`QUEUE.md:47`); neither is asked for.

## 2. Order

By what the clone gains per day of exact2 work, with each sub-LLP's stages
split where they land separately:

1. **1114.000 stage 1**, a file's byte range as a body (S). This alone lets the
   clone post any video Bluesky accepts, with a progress bar from counted parts.
2. **1114.001 stage 1**, `showNotification(navigate=…)` (S). This is a gap
   today: Apple's delegate handles only `willPresent` (`Notify.swift:45`,
   `:69-75`), and nothing in `host/` or `runner/` answers a tap.
3. **1114.002** camera and **1114.003** crop (S–M each). They share the picker
   path, and the avatar flow uses both.
4. **1114.004** in-app browser and **1114.009** the open grant (S each).
5. **1114.005 stage 1**: confirm the stream's captions show, put them in
   `state.media`, and admit `text/vtt` in `accept` (S).
6. **1114.008** accessibility actions (S–M), then **1114.007** composer
   highlights (M).
7. **1114.001 stage 2**, the scheduled wake (M–L).
8. **1114.000 stage 3**, `compressVideo` (L), and **1114.006** receiving shares
   (L). Both are large and gated on devices. Neither blocks a feature the clone
   can't approximate: without stage 3 the clone uploads the original, and
   without 1114.006 a person pastes a link.

## 3. Rulings asked, in one place

Each is argued in its sub-LLP; the recommendation is the sub-LLP's.

- **Admit** each of the ten, with the Bluesky clone as consumer. Recommended:
  yes for 1114.000 stages 1–2, 1114.001 stage 1, 1114.002, 1114.003, 1114.004,
  1114.005 stage 1 and D5, 1114.007, 1114.008 and 1114.009. Yes, but in
  their place in §2, for 1114.000 stage 3, 1114.001 stage 2 and 1114.006.
  Not yet for 1114.005's `<track>`.
- **`rules/DEFERRED.md`.** 1114.002 strikes the 2026-10-02 sentence "no
  `capture` on the picker (the camera is a native module)". 1114.001 stage 2
  admits background tasks; its offered take is that delivery's owed
  `BGAppRefreshTask` (LLP 1030.003) rides the same mechanism. Push stays
  refused (`:435`), and so do badges (`:436`), which bsky4 had counted on.
- **LLP 1069.002's amendments.** A2 is 1114.003's `crop` on `compressImage`.
  A3 is 1114.000's `compressVideo`. Each reopens part of D6.
- **The one-domain limit** on grants (Charlie, 2026-09-26) gives way to one
  open grant spelling, `https://*`, which admits any public HTTPS origin on 443
  and is printed first in the reach table (1114.009).
- **Specs not yet merged.** 1114.008 adopts ARIA's `aria-actions` ahead of its
  spec PR (w3c/aria#1805), as Firefox and Chrome 151 have. 1114.007's web path
  needs OpaqueRange (`textarea.createValueRange()`), which only Chrome has
  shipped. Elsewhere on the web it paints nothing. These browser facts are the
  drafting agents' and postdate this author's training; check them before
  ruling.
- **New apparatus**, which RULES.md has an agent ask for: a development-only
  `EXACT_WAKE` and the driver's `--wake` to test a cold wake (1114.001), and
  `host/apple/extension.mjs`, because `host/apple/build.mjs` is at 1,499 of its
  1,500 lines (1114.006).

## 4. The smaller gaps, triaged

None has a sub-LLP. Each gets one when the clone actually asks for it.

- **Alternate app icon (item 78).** Not there: no manifest key, no
  `CFBundleAlternateIcons`, no command. LLP 1030 D10 already chose the shape:
  a value the Contract sets, chosen from icons shipped in the binary, with
  iOS's own alert. The web has no standard; an installed web app's icon is
  fixed. S–M, and it touches `build.mjs`'s asset catalog. Low value.
- **Contacts (item 83).** Not there; the Expo survey leaves it to app modules
  (LLP 1069 §5 #8). The web's Contact Picker API (`navigator.contacts.select`,
  Chrome on Android only) maps to iOS's `CNContactPickerViewController`, which
  needs no permission (M). But Bluesky's "find friends" reads the whole address
  book and matches it on a server. No web standard covers that. It is an app
  Swift module (LLP 1067.000) with `NSContactsUsageDescription` and a privacy
  decision, and the picker doesn't do it.
- **Pausing an animated GIF or WebP (item 50).** Not there:
  `AnimatedRasters.swift` plays (LLP 1011.000) and has no pause, nor does the
  web's `<img>`. The web's usual answer is to play the GIF as a `<video>`, which
  Bluesky's web client does, but AVPlayer can't play WebM. A pause on an
  animated `image` is S on Apple and M on the web, which would reuse the frame
  decoder that today runs only under the agent (`image-glue.js`).
- **Apple's Translation framework (item 93).** "An app Swift module could do it
  today" is only partly true. Before iOS 26, `TranslationSession` comes only
  through SwiftUI's `.translationTask`, which a view module can host and a data
  module can't. The web has a standard here, the WICG Translator API
  (`Translator.create({sourceLanguage, targetLanguage}).translate()`), so a
  first-party capability would take its names. M.

The "already there" list was spot-checked and holds: `share`, `copyText`,
`openURL`, `setRootFontSize`, `haptic` (`contract/syntax/src/lib.rs:83-123`),
popover menus, swipe actions, reorder, video events, animated GIF/WebP, `iframe`,
the picker with video, `compressImage`, and `exactBodyFrom` (64 MiB,
`runner/src/request.rs:120`). The exceptions: a local notification posts, but
a tap routes nowhere (1114.001 stage 1). It doesn't post on Linux or Windows,
which refuse it as `unavailable`, or in the terminal, which drops it silently
(`host/terminal/src/host.rs:237-251`). Animated rasters are Apple and the web only.
Swipe actions are UIKit's own only on iOS; elsewhere they are a scroll-snap row.

## 5. Corrections to the request

- **Video upload.** Bluesky uploads in parts (`startUpload`, `uploadPart`,
  `finishUpload`, `getUploadStatus`), four at a time, so no body passes 64 MiB.
  Its limits are 300 MB and 10 minutes, not 100 MB and 3 minutes. The
  compressor is `@bsky.app/video-compressor` (3 Mb/s, at most 1920 px, and a
  file under 25 MiB passes through), and the web uses Mediabunny over WebCodecs.
- **Captions** reach the player inside the HLS playlist, written there by
  Bluesky's video service, not as a sidecar `<track>`. Attaching a `.vtt` is
  not only clone work: the picker refuses `text/vtt` today.
- **Camera.** Bluesky's camera button is native only (`OpenCameraBtn.web.tsx`
  returns null). On native it crops with its own crop sheet, not
  `EditImageDialog`, which is web only and crops to a PNG before compressing.
- **In-app browser.** `inApp` isn't a web name. Bluesky never uses a dismissal
  event. `docs/contract-for-humans.md:1036` wrongly says the JS target has no
  `openURL` (`host/web-js/commands.js:20` has one).
- **Push.** "Works while running" holds on Apple and the web only. Badges are
  refused, so the half-day estimate is short. `backgroundModes` is copied into
  `UIBackgroundModes` with nothing behind it.
- **Accessibility actions.** Bluesky has none on a feed post. It uses them for
  a chat message's options, the chat list's conversation options, and a
  notification's expand-authors and view-profile.
- **Composer styling** is colour only, never font or size, so a paint-only
  design covers it.
- **PDS hosts.** Grants take `scheme://*.domain` since 2026-09-26, and
  `https://*.host.bsky.network` already admits every Bluesky-hosted account.
  Only PDSes outside Bluesky's domains are unreachable, through `auth.session`
  as well as `net.fetch`.
- **Share extension.** The App Group is needed only for files, and the web half
  is a manifest key plus, for files, a service worker.

## 6. Not verified

- Every exact2 claim above cites code at `d81ea3013`. Claims about the clone come
  from a local copy of 2026-09-27 (`~/projects/bluesky-deploy/bluesky-exact2`)
  and should be rechecked against the current clone. That copy plays its HLS
  `playlist` in `video controls=true`, which Firefox can't play. That is a gap
  of its own, separate from captions.
- Nothing here was built or run, except the one sign-up probe in §1.
