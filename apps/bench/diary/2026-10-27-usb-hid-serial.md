surfaces: web, macos
agent: Claude Code (Claude Opus 5.5)
task: Bench, LLP 1133.037's serial monitor and DFU updater, over the substitute

### Rough

- A Contract resource of a module name that fails at launch (`attachedPorts` on the web,
  `NotSupportedError`) stops the web app's boot. The choice between the attached list and
  the remembered one is made in `app.ts` by the status's `attachedList` instead.
- `after(ms, …)` takes a literal only, and a mutation's `then` may not send the same
  mutation again, so DFU's `bwPollTimeout` waits are an `every(10, step)` task comparing
  `performanceNow()` with the time the board asked for.
- A record literal names every field, the options too (`USBDeviceFilter(vendorId=none, …)`).
- `accept=".bin"` on a file input is refused unless the app declares a file handler for it.
- Data code has no clock (`Date.now()` is refused in a data source), so the log's lines
  carry no timestamps yet.

### Lean in

- The substitute's `prefer extension serial.ports=[…]` with timed `input` events and the
  `chooser` ticket (`tap @chooser s3`) drove every serial case without a board.
- `dfu.ts` as a generator of waits made its state machine testable in Bun against a fake
  board and drivable step by step from Contract.

### Needed

- The `bytes` declaration type (LLP 1133.006 D8 item 1): bytes travel as base64 strings.
- A data reader's own cursor (LLP 1133.022 D19 item 5).

### Checkpoints

- web: `bun apps/bench/bench.drives.mjs web` passes drives 1, 2, 3, 5, 6, 7 and 8.
- `bun test apps/bench/dfu.test.ts`: four cases, DFU 1.1, DfuSe, errVERIFY, a board left
  in error.
