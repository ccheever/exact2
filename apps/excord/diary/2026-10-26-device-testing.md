surfaces: ios, web
agent: Codex, GPT-6
task: verify application lock behavior

## Rough

A keychain attributes query could prompt before explicit Unlock on the tested phone. The host now probes indexed accounts without permitting authentication, and the app prevents duplicate Unlock requests. The tester confirms one Face ID check at launch, cancellation and retry, protection after Home/return and a two-minute device lock, and passcode fallback both after failed Face ID attempts and with the app's Face ID permission disabled. The permission was restored. Deleting and reinstalling the same signed fixture creates a fresh install marker and opens its automatically signed-in channels without the old lock screen. LLDB attachment failed, so deletion of the previous protected Keychain item was not directly verified. Remaining checks are listed in LLP 1133.020. Debugging time was not measured.

The follow-up build compiles but cannot install because the app now requires Declared Age Range and the available development profile lacks that capability. The previously installed signed build remains usable for authentication checks; those results do not qualify the merged revision. A complete capability/profile prerequisite check before compilation would save the failed build's work.

Interrupted indexed writes must not select an unconfirmed Keychain generation. Reconciliation now waits for successful probes and schedules recovery while the app is idle, preserving the last durable credential. The regression fails before the repair and passes afterward; runner tests, the iOS simulator and macOS authentication suites, and independent review cover the repair. The existing phone observations do not qualify this new build.

The app-source sweep rejects `display` styling on the channel header's default Back button. `if not wide` keeps the native control on compact screens and omits it beside the wide directory.

## Lean in

An in-memory Keychain hook reproduces reconciliation failures without requiring a biometric prompt or modifying real credentials.

## Needed

- local authentication: provided, rough (see Rough)

## Checkpoints

- iOS build: rough; the follow-up requires an updated development profile
- editing app.contract: smooth
- driving the app: rough; the tester completed the basic biometric checks on the phone
