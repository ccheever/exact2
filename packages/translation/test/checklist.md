# @exact/translation on devices, by hand

LLP 1133.027 §12 stage 3's hand checks: the system's own download sheet and
a language pack removed in Settings, which the agent scripts but cannot
show. Each step says what to see; a step that shows something else is a
failure to report with a screenshot.

Excord's fixture world has a Japanese message to translate: in #general,
open the fixture menu ("Fixture traffic", the ⋯ button in the channel's
header, shown only against the fixture) and choose "Receive a
message in Japanese". Its text is "明日の会議は十時からです。".
Run the fixture server as `packages/local-auth/test/device-checklist.md`
says (on this Mac, with `--host <LAN address>` and
`EXACT_BACKEND_ORIGIN` for an iPhone, `adb reverse tcp:4790 tcp:4790` for
Android).

## Mac

Build: `bun host/apple/build.mjs excord-apple --run`.

1. System Settings → General → Language & Region → Translation Languages:
   remove Japanese if it is downloaded.
2. In Excord, right-click the Japanese message: the menu has Translate.
   Choose it. macOS shows its download sheet for Japanese, over Excord's
   window. Cancel: the line under the message reads "Japanese isn't
   downloaded", and no translation shows.
3. Translate again and choose Download: the sheet shows the download, then
   closes; the message shows "Tomorrow's meeting starts at ten o'clock." (or
   close to it) and "Translated from Japanese · Show Original".
4. Show Original: the translation goes.
5. An English message: its menu has no Translate.
6. Remove Japanese again (step 1) with Excord open, and translate the message
   once more: the sheet asks again (macOS 26) or the line reads "Japanese
   isn't downloaded" (macOS 15 to 25).

## iPhone, iOS 26 or later

Build: `bun host/apple/build.mjs excord-apple --ios --device --run`.

1. Settings → Apps → Translate → Downloaded Languages: delete Japanese.
2. Long-press the Japanese message: Translate. Choose it: iOS's download
   sheet. Cancel: "Japanese isn't downloaded". Translate, Download: the
   translation, as on the Mac.
3. With the translation showing, go to Settings and delete Japanese, return
   and translate another Japanese message: the line reads "Japanese isn't
   downloaded" or the sheet asks again; Excord does not crash or hang.

## iPhone or iPad, iOS 18 to 25

As iOS 26. The download sheet comes from a view Excord hosts out of sight;
check that it shows over the transcript and that cancelling it returns to
the transcript with the line.

## Chrome on a desktop

Chrome 138 or later has `Translator` and `LanguageDetector`.
`bun host/web/dev.mjs --app excord`, opened in Chrome.

1. chrome://on-device-translation-internals: remove the ja–en pack if it is
   installed.
2. Right-click the Japanese message → Translate: "Downloading Japanese…" with
   a percentage, then the translation once Chrome has the pack.
3. Firefox and Safari have no `Translator`: the menu has no Translate.

## Android phone (stage 4, not built)

The Kotlin implementation over ML Kit is not built (LLP 1133.027 §12, stage
4's as-built note). When it lands: on a Pixel, translate the message with
Wi-Fi on; the line reads "Downloading Japanese…" while ML Kit fetches the
model (about 30 MB), then the translation with "Translated with Google" and
Google's attribution graphic (D10).
