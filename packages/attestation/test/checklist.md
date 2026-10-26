# Attestation on a device (LLP 1133.028 §9 stages 2 and 3)

What the agent cannot drive: a real App Attest key in a Secure Enclave and a
Play Integrity token Google decodes. Each step names what to see; a step that
shows something else is a finding. A person runs them, never an agent on a
phone (LLP 1133.020).

## What the accounts must have first

- **Apple:** the App Attest capability on `com.exact.authfixture` in the team's
  identifiers, and a development profile that carries it. The build signs
  `com.apple.developer.devicecheck.appattest-environment` as `development` for
  a debuggable build and `production` otherwise (`host/apple/presence.mjs`).
- **Google:** the Play Integrity API enabled in LLP 1133.020 §3.2's Cloud
  project; a service account with a JSON key, kept outside the repository; its
  path in `EXACT_PLAY_INTEGRITY_KEY` where the fixture server runs; the
  project's number in `apps/auth-fixture/app.ts`'s `CLOUD_PROJECT`. For a
  `PLAY_RECOGNIZED` verdict, the auth fixture on a Play track in Play Console
  and installed from it (LLP 1133.020 §3.4).
- **The fixture server** reachable from the phone:
  `bun apps/auth-fixture/fixture.mjs --team <team id>` (no `--substitute`), on
  this Mac's LAN address, with `app.ts`'s `ISSUER` and its grants pointed at it.

## iPhone (iOS 17 or newer)

1. `bun host/apple/build.mjs --ios --device auth-fixture-apple --run`, signed
   with the team's profile. "Attestation: app-attest" shows.
2. Tap "Check this build": "Attested: App Attest, development build". The
   server's log (`GET /fixture/log`) has `attestation.attested: 1`.
3. Tap it again: "Asserted (counter 1): App Attest, development build"; again,
   counter 2.
4. `POST /fixture/reset` on the server, tap again: "Unknown key; reset.
   Attested: …", a new key.
5. Delete the app, install it again, tap: a new attestation (the key went with
   the app). Restore an iCloud backup of the phone that held the app: the
   first check attests a new key, since the key id is excluded from backup.
6. Turn on Airplane Mode, tap: "Failed: NetworkError; retried. Failed: …";
   turn it off, tap: the same key attests (Apple's `serverUnavailable` kept it).
7. Record for LLP 1133.028 stage 2: the `aaguid` the server logs
   (`appattestdevelop` or `appattest` with a development profile), and whether
   the receipt carries `apple_validation_category_01`.

## iOS Simulator

8. `--bind attestation` on a simulator build: "Attestation: unavailable"
   (`isSupported` is false there), and no button. Recorded on an iOS 27
   simulator on 2026-10-26: unavailable, and `attest` refuses
   `NotSupportedError`.

## Mac (macOS 27 or newer)

9. A development build without a profile signs no App Attest entitlement,
   and on macOS 27 it still reads available and attests a key: recorded on
   2026-10-26, with an `appattest` (production) `aaguid` and the bare bundle
   id as the app id, since an ad hoc signature has no team. With a profile
   carrying App Attest: steps 2 to 4. Record the `aaguid` and whether the Mac
   needs a capability beyond the profile (D17).

## Android phone (Android 10 or newer, with Play services)

10. Build the APK (`bun scripts/agent-android.mjs build auth-fixture --apk`)
    with `CLOUD_PROJECT` set, install it by `adb install`, tap "Check this
    build": "Play: unrecognized version, no device integrity" on an emulator, or
    "Play: unrecognized version, device integrity" on a phone. The server
    decoded a real token with the service account.
11. The same build from a Play track: "Play: recognized, device integrity".
12. A tester's account given fixed verdicts in Play Console: the line ends
    "(a test response)".
13. Turn on Airplane Mode, tap: "Failed: NetworkError; retried. Failed:
    NetworkError".
