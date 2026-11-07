# 2026-11-07 — Cross-host tab icon sources

## Rough

The Duo fix restored the iOS SF Symbols, but the generator still warned that
those symbols would be blank on the app's web and Linux deployments.

## Lean in

The tab bar now names app icon tokens. Each token keeps its SF Symbol for Apple
and supplies a monochrome SVG fallback for the other hosts, so the same tab
structure remains visible across the shipped targets.

## Checkpoint

- Contract generation and the mobile tab/navigation tests.
- The signed iOS build continues to use the same native SF Symbols.
