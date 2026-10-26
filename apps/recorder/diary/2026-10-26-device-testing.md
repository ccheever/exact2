surfaces: ios, web
agent: Codex, GPT-6
task: verify recording and playback

## Rough

An interrupted AAC writer discarded audio recorded after Resume. A PCM backup now recovers the completed take. Immediate playback could also open an unfinished asset; the app now waits for Stop. Speech model download was hidden when a local engine lacked its language model. Debugging time was not measured.

## Lean in

The same authored app tests exercise completed takes and language availability on the web before device checks.

## Needed

- audio: provided, rough (see Rough)
- speech recognition: provided, rough (see Rough)

## Checkpoints

- iOS build: smooth
- editing app.contract: smooth
- driving the app: rough; interruption and Silent mode needed phone checks
