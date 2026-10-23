# The agent cannot drag to dismiss the keyboard or film under real timing

**Status:** Open
**Systems:** `scripts/agent.mjs` on iOS (LLP 1012, LLP 1079)
**Severity:** P3
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

Two things Excord's §3.7 and §3.11 need a person and a phone for:

- A drag that ends over the software keyboard arrives as a synthetic scroll, not a
  touch, so UIKit's interactive keyboard dismissal (`keyboardDismissMode`
  `.interactive`, which the list has) cannot be driven.
- `"screenshot … over 600 every 50"` films on the agent's clock, which skips a
  scroll's motion; under real UIKit timing it cannot film, and `layout` during a
  running scroll reports where it is headed, not where it is.
