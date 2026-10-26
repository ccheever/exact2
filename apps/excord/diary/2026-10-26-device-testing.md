surfaces: ios
agent: Codex, GPT-6
task: verify application lock behavior

## Rough

A keychain attributes query could prompt before explicit Unlock on the tested phone. The host now probes indexed accounts without permitting authentication, and the app prevents duplicate Unlock requests. The tester confirms that one Face ID authentication opens Home, cancellation keeps the app locked, and Unlock then succeeds. Passcode fallback and the other unverified lifecycle cases are listed in LLP 1133.020. Debugging time was not measured.

## Lean in

The host regression covers indexed-secret reconciliation independently of a biometric prompt.

## Needed

- local authentication: provided, rough (see Rough)

## Checkpoints

- iOS build: smooth
- editing app.contract: smooth
- driving the app: rough; the tester completed the basic biometric checks on the phone
