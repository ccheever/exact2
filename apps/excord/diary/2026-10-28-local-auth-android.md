surfaces: android
agent: Claude Code (Claude Opus 5.5)
task: Excord's lock on an Android emulator: LLP 1133.010's drives 1 to 8 and the Keystore suite

### Rough

- Every lock drive started the fixture on 4790 while the APK called the gateway at
  `ws://127.0.0.1:5709`: since 2026-10-26 a development build bakes a fixture port of the
  checkout's own (4790 plus a hash of the app's directory). The session answered, the
  socket was refused, and the drives read it as the lock failing. The drives now ask the
  bake for the port (`excordFixture`, `packages/local-auth/test/fixture.mjs`).
  `packages/age-range/test/drives.mjs` still assumes 4790 (QUEUE.md).
- `--bind local-auth` with `EXACT_STORE=real` kept the token in the agent's own store on
  Android: the reader carrier passed only `EXACT_AGENT_*` to the app. It passes
  `EXACT_STORE` now (`scripts/agent-android.mjs`).
- Under load the emulator stopped the activity seconds after Home, so a drive that
  pressed Home and started the app again 1.5 s later found no return to lock: the
  activity had never left. The suite waits for Android's `STOPPED` first. A person
  cannot return faster than the activity stops, so this is the drive's, not Excord's.
- A fingerprint touch that arrives before BiometricPrompt has animated in is dropped
  (`FingerprintHal: fingerprint event listener at unexpected state`); the suite touches
  again until the call answers.

### Lean in

- `bun packages/local-auth/test/drives.mjs android` and `bun
  packages/local-auth/test/android.mjs` on an API 31 Google APIs emulator: `adb root`
  lists the app's files, `adb emu finger touch 1` answers BiometricPrompt and `adb shell
  input text <pin>` its PIN pad. Enrolling the emulator's finger is Settings' own flow,
  driven with `uiautomator dump` and `adb emu finger touch`.

### Needed

- Nothing new in exact2 for the lock; the prompt and Keystore on a phone with real
  hardware are `packages/local-auth/test/device-checklist.md`'s Android part.

### Checkpoints

- android: drives 1 to 8 under the agent pass; the conformance suite with the token in the
  Keystore passes (the switch's prompt, the lock on a real return from Home, the launch's
  prompt, an unenrolled finger refused, Back, the PIN, `unprotect`, and "Sign in again"
  once the screen lock is removed).
