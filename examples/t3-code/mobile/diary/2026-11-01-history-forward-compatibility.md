# Mobile history forward compatibility

surfaces: ios
agent: Codex gpt-6.1-sol
task: keep older-page scrolling usable when the server adds a turn-item type

### Rough

The scroll drive reached older history, then stopped with `Invalid T3 data` when
one page contained a turn-item tag this build did not know. The desktop decoder
already treats that union as extensible and keeps the page boundary moving.

### Lean in

The mobile decoder now drops only unknown turn-item rows from snapshots and older
pages, and advances the stream sequence for an unknown turn-item update. Known
rows still use the existing validation, so malformed data remains visible as an
error instead of being silently accepted. The cursor and `hasMore` values are
retained when a page contains a mixture of known and future rows.

### Needed

A rebuilt Duo drive confirmed that older-history scrolling continues from turn
12 through turn 01 without the decoder error. The table/code row stayed intact,
and the existing Home and transcript separators remained between rows only.
Frame pacing is still unqualified because the simulator does not provide FPS
sampling; no framework issue is claimed.

### Checkpoints

The regression is red before the decoder change and green after it. The full
mobile suite passes 4,241 tests with two native bridge skips, strict TypeScript,
Contract compilation, caps and boot pass, and the normal no-install iOS build
passes strict codesign. The Duo drive is recorded under
`target/t3-scroll-evidence/history-compat-receipt.json`.
