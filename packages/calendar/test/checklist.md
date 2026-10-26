# Calendars by a person (LLP 1133.018 §11, stage 4)

What no tool can do here: answer the system's calendar prompt, save or
cancel in the system's event editor, raise write-only access to full, and
look. The suites drive the rest: `agent.mjs` and the Calendar's
`tests/drives.mjs` under the agent on macOS and an iOS simulator; `apple.mjs`
with EventKit bound on a simulator (its own calendars; write-only set in its
privacy database) and only the status on macOS; `web.mjs` the `.ics` download
in Chromium, Firefox and WebKit; `packages/contacts/test/android.mjs` builds
the Android fixture's APK, which carries both packages. Each step says what to
run and what you should see. Write down the commit, the device and its OS
version beside each result.

Two apps carry the module:

- **The Calendar example** (`examples/calendar`): the agenda's "Show my
  calendars" switch reads the calendars and two months of events, and the
  editor's Add to Calendar… opens the system's editor.
  `EXACT_APP_DIR=examples/calendar bun host/apple/build.mjs --device calendar-ios --run`
  on an iPhone; `… calendar-macos --run` on a Mac.
- **The conformance fixture** (`test/fixture`), which saves, removes and
  raises, and whose grants are the write-only line and the full one. Make
  one with
  `bun -e "import('./packages/contacts/test/fixture.mjs').then(m => console.log(m.makeFixture('calendar').app))"`,
  then `EXACT_APP_DIR=<that path> bun host/apple/build.mjs --device calendarfixture-apple --run`.

## iPhone, iOS 17 or later

1. **Nothing is asked before the switch.** Install the Calendar fresh: no
   prompt at launch, and the agenda shows only the app's own items.
2. **Full access.** Turn on Show my calendars: the alert asks for full
   access with the Calendar's purpose. Allow: the agenda's section lists
   today's events from your calendars and the month dots the days that have
   one.
3. **A change while open.** In Calendar, add an event today, return: it
   shows in the section without a tap. Delete it: it goes.
4. **The editor.** Open an item, tap Add to Calendar…: the system's editor
   shows the item's title, time and notes. Add: the note reads saved, and
   with the switch on the event shows in the section. Again, and Cancel:
   nothing is added.
5. **Revoked and back.** Settings > Apps > Calendar example > Calendars >
   None: on return the section shows the denied line with Open Settings.
   Full Access again: the section returns. Record whether iOS ended the app
   when the setting changed.
6. **Write-only, then the raise** (the fixture). Install it fresh and tap
   Save: the alert asks to add events only (the write-only purpose). Allow:
   `saved` reads `none none` (no id: nothing can read it back), and the
   event is in your default calendar in Calendar. `walk` reads
   `error NotAllowedError write-only`. Tap Full access: iOS asks again for
   full access. Choose Keep Add Only (or Don't Allow on versions that use
   that label): `raised` reads `write-only`. Tap it again: write down whether
   iOS asks a second time.
7. **The editor without access** (the fixture, access None in Settings):
   tap Editor, Add: `edited` reads `saved`; the event is in Calendar.

## A Mac

1. In the Calendar, open an item and tap Add to Calendar…: Calendar opens
   the `.ics` and asks where to add it; the note reads "Opened in your
   calendar app" (macOS has no editor that reports, D15).
2. Turn on Show my calendars: the alert asks for full access with the
   purpose. Allow: your events and dots show. Record the time from the
   switch to the section (D8 asks for the first answer's time).
3. Install the fixture as a bundle (`… calendarfixture-apple --bundle`) and
   tap Save: write down whether macOS offers write-only access, and what
   `saved` and `walk` read after.

## Android phone (any version from 10)

`bun packages/contacts/test/android.mjs` builds
`apps/contacts-calendar-fixture`, whose APK declares `READ_CALENDAR` and,
because it calls `saveEvent`, `WRITE_CALENDAR`. Neither its `--emulator`
drives nor the steps below have run, since the emulator does not start on the
Mac stage 3 was built on. Install with
`adb install -r host/android/app/build/contacts-calendar-fixture/outputs/apk/release/app-release.apk`.

1. **One dialog for both permissions.** At launch the reads ask: one
   dialog, "Allow … to access your calendar?". Allow: `calendar-status`
   reads `granted full`, `calendars` lists the phone's calendars, `events`
   those of September and October 2026.
2. **A save in the device's zone.** Tap Save: `saved` reads
   `Fixture event UTC`, and the phone's calendar app shows it at 08:00 UTC
   on 16 September in the primary calendar. Write down which calendar that
   was.
3. **The insert screen.** Tap Add with the editor: the phone's calendar app
   opens its new-event screen with the title, time and notes. Save there:
   write down whether `edited` reads `saved` (the app returned `RESULT_OK`)
   or `unknown`, and which calendar app it was. Back out instead: `unknown`.
4. **Denied.** Deny the dialog twice: `denied none`, no dialog on the next
   launch; Settings > Apps > the fixture > Permissions > Calendar > Allow,
   and return: the status changes on resume.
5. **An all-day event.** Add one in the calendar app today: back in the
   fixture after a relaunch, it lists under its local date, not the day
   before or after (D2: the provider keeps it at a UTC midnight).
