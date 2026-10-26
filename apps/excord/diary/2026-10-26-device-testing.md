surfaces: ios
agent: Codex, GPT-6
task: verify application lock behavior

## Rough

A keychain attributes query could prompt before explicit Unlock on the tested phone. The host now probes indexed accounts without permitting authentication, and the app prevents duplicate Unlock requests. Biometric success still needs a person at the phone. Debugging time was not measured.

## Lean in

The host regression covers indexed-secret reconciliation independently of a biometric prompt.

## Needed

- local authentication: provided, rough (see Rough)

## Checkpoints

- iOS build: smooth
- editing app.contract: smooth
- driving the app: rough; biometric completion cannot be qualified through mirroring
