# Passkeys on a device (LLP 1133.011 §10, drive 9)

What the agent cannot drive: the platform's own sheet against the fixture's
relying party at its public origin, `https://exact-auth-fixture.expo.app`. Each
step names what to see; a step that shows something else is a finding.

## The origin

`bun apps/auth-fixture/origin.mjs` deploys it and checks it; `--check` only
checks. It serves:

- `/.well-known/apple-app-site-association`: `webcredentials` for
  `CUW59GZ74D.com.exact.authfixture` (the team of this Mac's Apple Development
  identity, `app.json`'s `host.ios.team`) and `SIMULATORX.com.exact.authfixture`
  (a simulator build).
- `/.well-known/assetlinks.json`: `delegate_permission/common.get_login_creds`
  for `com.exact.authfixture` and each SHA-256 in `app.json`'s
  `host.android.certificates`. The one listed is this Mac's
  `~/.android/debug.keystore`, which signs every APK built here; an APK built on
  another machine needs that machine's fingerprint added and the origin deployed
  again.
- `POST /passkey/…`: the relying party (`apps/auth-fixture/relying-party.mjs`),
  accepting the origins `https://exact-auth-fixture.expo.app` and the
  `android:apk-key-hash:…` of each listed certificate.

The server keeps nothing: each passkey it registers is a signed record the app
keeps. "Forget this passkey" drops the record of the passkey the session signed
in with, which is the server forgetting it.

## Every build

Build the fixture with the public relying party selected:
`EXACT_BACKEND_ORIGIN=https://exact-auth-fixture.expo.app`. The bake then takes
the grant `auth.passkey exact-auth-fixture.expo.app` and fetches the relying
party there. Sign-in and "Check this build" still talk to the fixture server on
this Mac, which a phone cannot reach; this checklist does not use them.

## iPhone (iOS 17.4 or newer)

1. Register the App ID `com.exact.authfixture` in team CUW59GZ74D with the
   Associated Domains capability, and install a development profile for it that
   names the phone; the team's wildcard profile cannot carry Associated Domains.
   Then `EXACT_BACKEND_ORIGIN=https://exact-auth-fixture.expo.app bun
   host/apple/build.mjs --ios --device auth-fixture-apple --run`. A development
   build signs `webcredentials:exact-auth-fixture.expo.app?mode=developer`: turn
   on Settings › Developer › Associated Domains Development, or the phone reads
   the file through Apple's CDN, which can lag a deploy by hours.
2. Type a username, tap "Create a passkey": the system sheet names
   exact-auth-fixture.expo.app and the username; confirm with Face ID. The app
   shows "Signed in as …". If it shows "The server refused the passkey (origin
   …)", the origin it names is the one AuthenticationServices writes: record it
   for D6.
3. Tap "Sign out". Tap the username field: the QuickType bar offers the passkey.
   Choose it: signed in, with no second sheet.
4. Sign out, tap "Sign in with a passkey": the sheet lists the passkey; choose it:
   signed in.
5. Sign out, tap "Sign in with a passkey", cancel the sheet: no error line, and
   the field's suggestion comes back.
6. Signed in, tap "Create a passkey" again: "You already have a passkey for this
   account".
7. Signed in, tap "Forget this passkey": "The server forgot this passkey". Sign
   out, tap "Sign in with a passkey" and choose it: "That passkey is not for an
   account here", and Settings › Passwords no longer lists it
   (`signalUnknownCredential`, iOS 26.2 or newer; on an older OS it stays
   listed, which is D13's "nothing to tell").

## Mac (macOS 14 or newer)

A Mac build signs the Associated Domains entitlement only with a provisioning
profile (D6), which no build here makes yet: an unprofiled build reads
`available: false` and shows neither button (drive 6, which the agent runs).
With a profile: steps 2, 4, 5, 6 and 7 above; step 3 does not apply (no
conditional request on macOS), and the sheet offers the iPhone by QR code as well
as iCloud Keychain.

## Android phone (Android 10 or newer, the reader's floor; a Google account and a screen lock)

1. On this Mac, `EXACT_BACKEND_ORIGIN=https://exact-auth-fixture.expo.app bun
   scripts/agent-android.mjs build auth-fixture --apk`, then `adb install` the
   APK it names. It is signed with the debug key whose fingerprint the origin
   lists.
2. Steps 2 to 7 above, with Google Password Manager's sheet instead of Apple's.
   Step 3 (suggestions in the keyboard) needs Android 15 (API 35); on an older
   phone the field offers nothing and step 4 is the way in. Step 7's removal
   needs API 35 and a provider that implements the signal; Google Password
   Manager's support is the provider's to state.
