# The composers by a person (LLP 1133.024 D14–D16, stage 5)

What no tool can do here: look at the system's mail and message composers,
see what they were given, send or cancel, and see the answer. The Calendar
example's `tests/compose.mjs` drives the rest under the agent: the `compose`
ticket's options and its answers. Write down the commit, the device and its
OS version beside each result.

In the Calendar example, open an item, Invite… two contacts with an email
and a phone number, then use the editor's two buttons.

## iPhone, with a Mail account

`bun host/apple/build.mjs --ios calendar-ios --device --run`.

1. **Email the Invitation…** Mail's composer shows over the app with both
   guests in To, the item's title as the subject, its date in the body and
   `invite-<id>.ics` attached. Send it to yourself: the attachment opens in
   Calendar as the item. The editor's answer is `sent`; Cancel and Delete
   Draft answer `cancelled`, Save Draft `saved`.
2. **Text the Guests…** Messages' composer shows with both phone numbers
   and the body. Cancel answers `cancelled`.

## iOS simulator (no Mail account)

`bun host/apple/build.mjs --ios calendar-ios --run`.

3. **No account.** `composeStatus().emailAttachments` is false, so Email
   the Invitation… shares the `.ics` through the share sheet instead.
   A mail with no attachment (`composeEmail` with none) opens a `mailto:`
   URL: the simulator says no app opens it, and the answer is `failed`.

## Mac

`bun host/apple/build.mjs calendar-macos --run`.

4. **Email the Invitation…** Mail opens a compose window with the guests,
   the subject and the `.ics` attached; the answer is `opened`. With a `cc`
   beside an attachment the send refuses with `NotSupportedError`.
5. **Text the Guests…** Messages opens a compose window with the numbers.

## Android phone

Once the example has an Android crate (it has none yet), its APK installed.

6. **Email the Invitation…** Only mail apps are offered (Gmail, say),
   with the guests, the subject, the body and the `.ics` attached through
   the app's file provider; the answer is `opened`.
7. **Text the Guests…** The default SMS app opens with the numbers and the
   body.

## Web

`bun host/web/dev.mjs --app calendar` in Chrome.

8. **Email the Invitation…** The `.ics` goes to the share sheet (the web's
   mail carries no attachment); a mail with no attachment opens the
   default mail app through `mailto:` with the fields filled.
