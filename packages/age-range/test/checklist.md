# The age range on a device (LLP 1133.028 §9 stages 2 and 3)

What the agent cannot drive: Apple's Declared Age Range sheet and Play's Age
Signals answer for a real account. Excord's age-restricted channel
(#after-hours in the fixture world) is the consumer. Each step names what to
see; a step that shows something else is a finding. A person runs them, never
an agent on a phone (LLP 1133.020).

## What the accounts must have first

- **Apple:** the Declared Age Range capability on `com.exact.excord`, and a
  development profile that carries it
  (`com.apple.developer.declared-age-range`). A sandbox Apple Account
  (Settings › Developer › Sandbox Apple Account), whose Age Assurance
  scenarios set the age and region the sheet answers for (LLP 1133.020 row
  15).
- **Google:** Excord on a Play track in Play Console, its age bands left at
  Play's defaults (0–12, 13–15, 16–17, 18+), and installed from the track. A
  sideloaded build gets `APP_NOT_OWNED`.
- **Excord's fixture server** reachable from the phone
  (`bun apps/excord/fixture/server.ts --world transcript --host <LAN address>`,
  `EXACT_BACKEND_ORIGIN=http://<LAN address>:4790` for the build).

## iPhone (iOS 26 or newer)

1. `bun host/apple/build.mjs --ios --device excord-apple --run` with the
   profile. Open #after-hours: Apple's sheet asks to share the age range with
   Excord, over the channel's spinner.
2. An adult scenario, Share: "Age-Restricted Channel" with Continue; Continue
   opens the channel. Go back and open it again: no sheet, no gate (once a
   launch).
3. Relaunch, open it, Don't Share: the confirmation alone, the same screen as
   step 2.
4. A scenario under 18, Share: "This Channel Isn't Available", no
   Continue; Go Back.
5. A scenario in a region that requires the signal (Texas): the sheet has no
   way to decline.
6. iOS 18 or older: the confirmation alone, no sheet (NotSupportedError).

## Mac (macOS 26 or newer)

7. A build with a profile: steps 1 to 3 in a window; the sheet is attached to
   Excord's key window. Without a profile the build signs no entitlement and
   the confirmation shows alone.

## Android phone (Android 10 or newer, installed from Play)

8. Open #after-hours: Play may show its own prompt, or none in the US states
   with these laws. An adult account: the confirmation, then the channel.
9. A supervised account under 18: "This Channel Isn't Available".
10. An account Play asks to verify: "This Channel Isn't Available" with
    "Google Play needs you to finish verifying your age …".
11. A build installed by `adb install`: the confirmation alone (`APP_NOT_OWNED`
    is NotAllowedError, which Excord treats as no signal).

## Windows (later, D17)

12. On the NucBox (build 26200): whether the Windows age APIs exist at all,
    recorded; the cell is built with package identity (LLP 1115.004 D15).
