# @exact/local-auth on devices, by hand

LLP 1133.010 §9 drive 10 and what the simulator and the emulator cannot
show: the Secure Enclave or the Keystore's hardware, the Face ID permission
prompt (the iOS 27 simulator shows none), a passcode removed, sleep. Each
step says what to see; a step that shows something else is a failure to
report with a screenshot.

Build Excord for the device and sign in with a real account, or against the
fixture, which signs in by itself. A development build bakes the fixture at
`http://127.0.0.1:<port>`, a port of this checkout's own that `bun
scripts/agent-launch.mjs --fixture-port --app excord` prints (`<port>` below):

- Android: run `bun apps/excord/fixture/server.ts --world transcript --port
  <port>` on this Mac and `adb reverse tcp:<port> tcp:<port>`.
- iPhone: run the server on this Mac's LAN address (`ipconfig getifaddr en0`,
  say `10.0.0.73`) with `--host 10.0.0.73 --port <port>`, and build with
  `EXACT_BACKEND_ORIGIN=http://10.0.0.73:<port>`. A development build takes the
  fixture's loopback grants for that address; a production build refuses it.
  Allow Excord's Local Network prompt.

## iPhone with Face ID

Build: `bun host/apple/build.mjs excord-apple --ios --device --run`.

1. Open the account menu. The switch reads "Require Face ID".
2. Turn it on. iOS asks "Do you want to allow Excord to use Face ID?" with
   the text "Excord uses Face ID to unlock your Discord session." Allow.
   Face ID's sheet shows; look at the phone. The switch is on.
3. Quit Excord from the app switcher and open it. The lock screen shows,
   then Face ID's sheet, before any channel or message. Look: the channels
   load.
4. Go to the home screen and back. The lock screen covers the transcript;
   tap Unlock, look: the transcript returns, with no reconnect (no
   "connecting" banner).
5. Lock the phone, wait two minutes, unlock it, open Excord: as step 4.
6. Open Excord, cancel the sheet: the lock screen stays, with Unlock.
   Cover the camera and let Face ID fail five times: the sheet falls back
   to the passcode; enter it: the channels load.
7. Settings → Face ID & Passcode → Excord off, then open Excord: the sheet
   asks for the passcode instead (the lock does not stop). The switch's
   label still reads "Require Face ID".
8. Settings → Face ID & Passcode → Turn Passcode Off. Open Excord: "Sign in
   again" on the sign-in screen; the token is gone (iOS deleted the item
   with the passcode). Turn the passcode back on afterwards.
9. Delete Excord and install it again: it opens signed out, with nothing of
   the last install (the first-launch wipe).

## Mac with Touch ID

Needs a macOS development provisioning profile for `com.exact.excord`
granting Keychain Sharing (`keychain-access-groups`). Signing a Mac build
with it is not built yet (LLP 1133.010 §11, stage 1's as-built note): today
the build says "Locked secrets: this build keeps none", the switch does not
show, and the token stays in the login keychain. When the signing lands:

1. `bun host/apple/build.mjs excord-apple --run`: the build names the
   profile it signed with.
2. The switch reads "Require Touch ID". Turn it on: Touch ID's sheet, with
   "Unlock Excord". Touch: the switch is on.
3. Quit and open: the lock screen, then the sheet, before any channel.
4. A Mac without Touch ID, or with the lid closed on a display: the sheet
   asks for the login password.

## Android phone (stage 2)

Build the APK with the reader (`bun scripts/agent-android.mjs build excord --apk`; the NDK and JDK 17 as `host/android/README.md` says) and
install it.

1. The switch reads the platform's own label ("Use fingerprint", "Use
   face"). Turn it on: BiometricPrompt with "Unlock Excord"; touch the
   sensor. The switch is on.
2. Quit and open: the lock screen, then the prompt, before any channel.
   Use the PIN instead ("Use PIN"): the channels load.
3. Home, then Excord again: the lock screen; Unlock; the prompt.
4. Lock the phone, wait two minutes, unlock, open Excord: as step 3.
5. Settings → Security → screen lock to None: Excord shows "Sign in again"
   (the Keystore key is invalidated with the lock screen).
