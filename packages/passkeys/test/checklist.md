# Passkeys on a device (LLP 1133.011 §10, drive 9)

What the agent cannot drive: the platform's own sheet against a relying party at
a public origin. Each step names what to see; a step that shows something else is
a finding.

## What the origin must have first

- The relying party (`apps/auth-fixture/relying-party.mjs`'s routes) answering at
  `https://<domain>`, its origin list holding `https://<domain>` and each Android
  build's `android:apk-key-hash:…` (`EXACT_ANDROID_ORIGINS`).
- `https://<domain>/.well-known/apple-app-site-association` naming
  `<team id>.com.exact.authfixture` under `webcredentials.apps`, and
  `SIMULATORX.com.exact.authfixture` for a simulator build.
- `https://<domain>/.well-known/assetlinks.json` with
  `delegate_permission/common.get_login_creds` for the APK's package and the
  SHA-256 of the certificate that signed it.
- The fixture pointed at it: `app.ts`'s `ISSUER` (`http://127.0.0.1:4331`) and
  its `net.fetch` and `auth.session` grants to `https://<domain>`, and its
  `auth.passkey` grant and `rpId()` from `auth-fixture.exact.test` to `<domain>`.

`bun host/web/build.mjs auth-fixture-web --bake` prints both files' entries when
the RP ID is not the app's own origin (`passkeyNote`), and writes them when it is.

## iPhone (iOS 17.4 or newer)

1. `bun host/apple/build.mjs --ios --device auth-fixture-apple --run`, signed with
   a team whose id the association file names. A development build signs
   `webcredentials:<domain>?mode=developer`: turn on Settings › Developer ›
   Associated Domains Development, or the device reads the file through Apple's
   CDN, which can lag a deploy by hours.
2. Type a username, tap "Create a passkey": the system sheet names the domain and
   the username; confirm with Face ID. The app shows "Signed in as …".
3. Tap "Sign out". Tap the username field: the QuickType bar offers the passkey.
   Choose it: signed in, with no second sheet.
4. Sign out, tap "Sign in with a passkey": the sheet lists the passkey; choose it:
   signed in.
5. Sign out, tap "Sign in with a passkey", cancel the sheet: no error line, and the
   field's suggestion comes back.
6. Signed in, tap "Create a passkey" again: "You already have a passkey for this
   account".
7. Have the server forget the credential (`POST /passkey/forget`), sign in with a
   second passkey: Settings › Passwords no longer lists the forgotten one
   (`signalAllAcceptedCredentials`, iOS and macOS 26.2 or newer; on an older
   OS nothing changes, which is D13's "nothing to tell").

## Mac (macOS 14 or newer)

A Mac build signs the Associated Domains entitlement only with a provisioning
profile (D6), which no build here makes yet: an unprofiled build reads
`available: false` and shows neither button (drive 6, which the agent runs).
With a profile: steps 2, 4, 5, 6 and 7 above; step 3 does not apply (no
conditional request on macOS), and the sheet offers the iPhone by QR code as well
as iCloud Keychain.

## Android phone (Android 10 or newer, the reader's floor; a Google account and a screen lock)

1. Build the APK with the certificate whose SHA-256 is in `app.json`'s
   `host.android.certificates` and in `assetlinks.json`, and install it.
2. Steps 2 to 6 above, with Google Password Manager's sheet instead of Apple's.
   Step 3 (suggestions in the keyboard) needs Android 15 (API 35); on an older
   phone the field offers nothing and step 4 is the way in.
3. Step 7 needs API 35 and a provider that implements the signal; Google Password
   Manager's support is the provider's to state.
