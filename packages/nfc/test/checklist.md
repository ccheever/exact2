# NFC by a person (LLP 1133.035 §11 items 11 to 17)

What no simulator or emulator can do: bring a tag to a radio. Each step says
what to run and what you should see. Record what you see beside the item's
number; where an item says "record", the answer goes into the RFC's as-built
notes.

## What you need

- Five NTAG215 stickers (NFC Forum Type 2, 504 bytes, 492 of them for
  NDEF). Write nothing to them beforehand except where a step says so.
- A contactless bank or transit card. Never write down its number or any
  other data it shows.
- An iPhone XS or later, and an Android phone with NFC and Chrome on it.
  Use test phones: never record a personal phone's own cards.
- Shelf's first-run library: The Dispossessed carries its Open Library
  work key (`/works/OL59863W`), so its tag holds the page's URL. Piranesi
  carries none, as a book added by hand does, so its tag holds Shelf's own
  record only.

## Before the iPhone

The App ID `com.exact.shelf` needs the **NFC Tag Reading** capability
(Certificates, Identifiers & Profiles, then a fresh development profile).
That is an account step for the team's admin. Without it, the build signs,
but iOS ends the app at its first scan. Then:

```
bun host/apple/build.mjs --device shelf-apple --run
```

The first Scan or Tag This Book shows no permission dialog: iOS asks
nothing for NFC.

## Before Android

```
bun scripts/hermes-android.mjs build        # once
bun scripts/agent-android.mjs build shelf --apk
adb install -r host/android/app/build/shelf/outputs/apk/release/app-release.apk
```

For Chrome on the phone: `bun host/web/dev.mjs --app shelf`, then
`adb reverse tcp:<port> tcp:<port>` and open `http://localhost:<port>` in
Chrome. Web NFC needs a secure context, and `localhost` is one, but the
Mac's LAN address is not.

## Lock one sticker

Permanently lock one sticker beforehand. A tag app such as NXP TagWriter
can lock it, and so can the fixture's Lock (item 17's app). Mark it, because
it can never be written again.

## The items

11. **Tag and scan.** On each platform, tag The Dispossessed onto one
    sticker and Piranesi onto another (open the book, Tag This Book, hold
    the sticker to the phone). Then Scan each: the right book opens.
    - iPhone: the system's sheet shows "Hold your iPhone near the sticker in
      the book." and closes with "Tagged". No alert follows.
    - Android and Chrome: Shelf's own sheet says "Hold your phone near the
      sticker in the book" with Cancel, then "Tagged".
    - Cancel while waiting: nothing is written, and Scan or Tag This Book
      shows again.
12. **Shelf closed.** Close Shelf and touch The Dispossessed's sticker to
    the phone.
    - iPhone: the background-read notification; tapping it opens Open
      Library's page for the book in Safari.
    - Android 16 opens the page in the browser. Android 17 first shows its
      "open link" notification. Record which you got.
    - Close Shelf and touch Piranesi's sticker (Shelf's own record only):
      the iPhone does nothing, and Android says no app supports it.
    - Delete The Dispossessed from Shelf and Scan its sticker: "Not on Your
      Shelf", with the link, and Open Link opens it.
13. **Failures.**
    - Tag a book onto the locked sticker: "This tag is locked".
    - Pull the sticker away mid-write: "Hold the phone still until it's
      done". It is easiest with the sticker at the edge of the reader's
      range.
    - "This tag is too small" needs a tag smaller than Shelf's two records
      (about 80 bytes), such as an NTAG210 (48 bytes). An NTAG215 never
      gives it. If you have one, record Chrome's error name for it (D3).
    - On the iPhone, each failure is the sheet's last line and no alert
      follows. On Android and in Chrome, it is an alert.
14. **A card that holds no NDEF.** Hold the bank card to the phone during a
    Scan.
    - Android and Chrome: "This tag can't be read", and the scan goes on.
      A sticker after it is read.
    - iPhone: nothing happens and the sheet stays (D11).
15. **The radio (Android).** Turn NFC off in Settings and return to Shelf:
    Scan and Tag This Book read "Turn On NFC", which opens the NFC switch.
    Turning it on brings Scan back.
16. **Two stickers at once.** Hold two stickers together to the phone during
    a Scan.
    - iPhone: neither is read. Take one away, and the one left is read.
    - Android and Chrome: record which one is read (D5).
17. **Tier 2, from the fixture app.** Make it as the suite does
    (`packages/contacts/test/fixture.mjs`'s `makeFixture('nfc')`; keep its
    directory), then build it for the iPhone with
    `EXACT_APP_DIR=<its directory> bun host/apple/build.mjs --device <name>-apple --run`.
    For Android, build it with `agent-android.mjs build <name> --apk`.
    - iPhone, with `apple.polling`: a reading carries a `serialNumber` and
      `apple.tag`. Command sends an ISO 7816 SELECT. To an NTAG it fails,
      and to a payment card it answers status bytes; record the two bytes,
      never the data.
    - Android: `android.tag.id` matches Chrome's `serialNumber` for the
      same sticker.
    - From a `reading`'s handler, `sendMiFareCommand` (iPhone) and
      `transceive` (Android) send `GET_VERSION` (`60`) to an NTAG215: the
      answer ends `00 11 03` (NTAG215's storage size and protocol).
    - On the iPhone, a second sticker is read only after the first is taken
      away.
    - Command after the scan ended fails `InvalidStateError`.
- **Chrome, a data-code write (D15).** In Chrome on the phone, Tag This
  Book's write runs from data code after the press. Record whether Chrome
  still allows it as the press's activation, or fails it
  `NotAllowedError`.
- **Android, an activity remade.** Turn on Developer options > "Don't keep
  activities". Start a Scan, switch to another app and back: the scan still
  waits, and a sticker is read.
