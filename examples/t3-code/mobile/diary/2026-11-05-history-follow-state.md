# History follow-state steady scrolls

## Rough

The thread scroll action already avoided rebuilding a settled anchor, but it still assigned the same live-edge follow flag and owner on every native scroll event.

## Lean in

Follow-state assignments now publish only when the route owner or end-follow value changes. This keeps the scroll action's steady path free of scalar state writes as well as anchor-array writes.

## Needed

The source regression proves the no-write path. A populated native feed with frame tracing is still needed for a device-level measurement.

## Checkpoints

- Added a Proxy-backed Contract regression for repeated live-edge events.
- The full mobile suite remains the validation gate.
