# Working capsule uses the source glass surface

The combined working/status capsule above the composer already matched the source's segment sizing and measured width reservations, but it still painted a flat fallback fill. The source uses its regular glass surface for this capsule.

The Contract now mounts the existing `t3-composer-material` leaf behind the capsule and passes the current appearance's glass tint without widening `ThreadColors`. The existing fallback fill and border remain as the material's surface and fallback path. Focused coverage checks the native leaf, radius, tint, surface and border declarations.

## Checkpoints

- focused working-control tests: pending
- mobile Contract build: pending
- iOS simulator build: pending
