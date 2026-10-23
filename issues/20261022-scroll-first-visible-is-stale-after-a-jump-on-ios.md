# After a keyed jump, iOS reports the row above as the first visible one

**Status:** Open
**Systems:** Apple host (iOS), the virtualized `list`'s scroll event
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

After `scrollIntoView` to key K (the agent's `tap transcript into K`), row K sits
exactly at the list's top, but the list's scroll event reports K−1 at 84.9 pt. The
report was computed while the rows above K had estimated heights; when they measure
and the list does not move, nothing reports again. The web reports K. An app that
saves its reader's place from the scroll event (Excord §3.3) saves the wrong row.

## Expected

A report after the rows around the viewport settle, when it differs from the last one.
