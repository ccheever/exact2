# History scroll steady-state writes

## Rough

The thread scroll action still entered its end and empty-key cleanup branches on every native scroll event. Those branches rebuilt the saved-anchor array even when the current route already had no anchor to clear.

## Lean in

The cleanup branches now require an anchor key, offset, pending restore, or stale revision before publishing. Repeated live-edge and empty-key events therefore leave the Contract state untouched.

## Needed

This is a source and Contract optimization. A populated native thread drive with frame tracing is still needed for a device-level scroll measurement.

## Checkpoints

- Added regressions for repeated live-edge and empty-key events.
- The full mobile suite remains the validation gate.
