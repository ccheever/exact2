# Contacts by a person (LLP 1133.018 §11, stage 4)

What no tool can do here: answer the system's contacts prompt, choose in
the system's pickers, share contacts through iOS 18's access picker, and
look. The suites drive the rest: `agent.mjs` under the agent on the web,
macOS and an iOS simulator; `apple.mjs` with the Contacts framework bound on
a simulator (its six sample contacts, access set by `simctl` and the
simulator's privacy database) and only the status on macOS;
`android.mjs` builds the Android fixture's APK. Each step says what to run
and what you should see. Write down the commit, the device and its OS
version beside each result.

Two apps carry the module:

- **The Calendar example** (`examples/calendar`): the editor's Invite…
  picks guests with `selectContacts(name, email, multiple)`.
  `EXACT_APP_DIR=examples/calendar bun host/apple/build.mjs --device calendar-ios --run`
  on an iPhone; `… calendar-macos --run` on a Mac.
- **The conformance fixture** (`test/fixture`), which also reads the store
  and has Share more (the access picker). Make one with
  `bun -e "import('./packages/contacts/test/fixture.mjs').then(m => console.log(m.makeFixture('contacts').app))"`,
  then `EXACT_APP_DIR=<that path> bun host/apple/build.mjs --device contactsfixture-apple --run`.

## iPhone, iOS 18 or later

1. **The picker needs no access.** In the Calendar, open an item, tap
   Invite…. The system's contact picker shows every contact with no prompt
   first. Choose two and tap Done: the item shows "Guests: <both names>".
   Invite… again and Cancel: the guests stay.
2. **The prompt's limited choice.** Install the fixture fresh. At launch the
   store read asks: the alert names the fixture's purpose ("The conformance
   suite reads the contacts.") and offers to share all contacts or to
   choose. Choose, pick three, and continue: `status` reads
   `true granted limited`, and `names` lists only those three (`walk`
   counts their ids and pages).
3. **The access picker.** Tap Share more: iOS's contact access picker shows
   the contacts not yet shared. Add one: `names` grows by one without a
   relaunch.
4. **Full access, and a change while open.** Settings > Apps > the fixture >
   Contacts > Full Access, then return: `status` reads `true granted full`
   and `names` lists everyone. In Contacts, add a person whose name holds
   "zz": back in the fixture, type `zz` in the search: `names` is that
   person.
5. **Revoked.** Settings > Apps > the fixture > Contacts > None: on return
   `walk` reads `error NotAllowedError`. Record whether iOS ended the app
   when the setting changed (D7 observed it; Apple does not document it).
6. **Icons.** Pick (the fixture's Pick asks for names and icons) a contact
   with a photo: `chosen` names it with an `app:/tmp/picked/contacts/…`
   path.

## A Mac

1. In the Calendar, open an item and tap Invite…: a popover shows the
   picker from the window's centre (there is no anchor yet, D13 item 1).
   Choose one contact: it becomes a guest. macOS picks one at a time.
2. Install the fixture as a bundle (`… contactsfixture-apple --bundle`,
   then open the `.app`). The launch read asks; the alert shows the
   purpose. Allow: `walk` lists your contacts. Record that macOS has no
   limited choice, and that Share more answers `NotSupportedError`.

## Android 17 (API 37) phone

`bun packages/contacts/test/android.mjs` builds
`apps/contacts-calendar-fixture` and checks its APK here; neither its
`--emulator` drives nor the steps below have run, since the emulator does
not start on the Mac stage 3 was built on. Install with
`adb install -r host/android/app/build/contacts-calendar-fixture/outputs/apk/release/app-release.apk`.

1. **The system's picker.** Tap Pick: Android's contact picker (not the
   Contacts app) shows, asking for names and emails, several allowed.
   Choose two: `chosen` names both. No permission dialog shows first.
2. **The store.** At launch the read asks for contacts ("Allow … to access
   your contacts?"). Allow: `contacts-status` reads `granted full` and
   `contacts` lists the first hundred in the phone's order. Deny twice:
   `denied none`, and no dialog shows on the next launch.
3. **A change in Settings.** Settings > Apps > the fixture > Permissions >
   Contacts > Don't allow, and return: the status changes on resume
   without a relaunch.

## Android 16 or earlier phone

1. Tap Pick: the Contacts app's own picker (its look is the maker's: write
   down which), one contact through its emails. Choose one: `chosen` names
   that contact, one only, whatever `multiple` asked.
2. Pick a contact with no email: write down what the picker offers (it
   lists only contacts with the data type asked).
3. Steps 2 and 3 of Android 17 hold here too.

## Chrome on an Android phone

Open the Calendar's web build (`EXACT_APP_DIR=examples/calendar bun
host/web/dev.mjs --app calendar`, then the dev URL from the phone, `adb
reverse tcp:<port> tcp:<port>` first). In
an item, Invite… shows (Chrome on Android has the Contact Picker); tap it:
the system's picker asks for names and emails. Choose two: both become
guests.
