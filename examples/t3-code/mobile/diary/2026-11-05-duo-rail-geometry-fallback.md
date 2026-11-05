surfaces: ios
agent: Codex
task: preserve responsive route geometry through transient native transitions

# Duo rail geometry fallback

## Rough

The first focused test run failed because the root action needed the geometry
parser in its Contract fixture, but imported source functions are not exposed to
that action evaluator. The action now recognizes the native ready packet marker
and the TypeScript source function validates the stored packet before using it.
The app-local checkout has no `exact.mjs`, so the documented feedback command
could not run from this example; the command failure is recorded here instead.

## Lean in

Keeping the fallback in the same route generation makes the transition rule
small: a transient unavailable packet keeps the last ready reservation, while a
new generation or route gets no stale geometry.

## Needed

- fold and keyboard geometry: provided by the native content observer; the app
  retains a validated same-generation reservation during transient unavailability
- native simulator: provided

## Checkpoints

- editing app.contract / app.ts: smooth after the fixture adjustment
- iOS build: smooth; the signed simulator app built and opened
- driving the app: smooth launch and accessibility tree on the iPhone Duo; a
  connected thread was unavailable for the keyboard-specific drive
