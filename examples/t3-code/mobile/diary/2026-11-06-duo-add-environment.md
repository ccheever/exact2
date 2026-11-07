# 2026-11-06 — Duo Add Environment rail clearance

## Rough

The Add Environment form used the ordinary trailing safe-area padding. On the
iPhone Duo, the native vertical rail is a route-local occlusion, so the form
could extend underneath the rail even though Home and the thread feed already
reserved it.

## Lean in

Pairing routes now add the measured trailing occlusion to their existing
safe-area padding. The compact layout keeps the same width because the
reservation is zero when no Duo rail is present.

## Needed

A fresh TestFlight drive must open Add Environment on the Duo and confirm the
form card, fields and submit button remain clear of the rail while the sheet
keeps the Expo 55% and 70% resting heights.

## Checkpoints

- Focused mobile Contract tests, including content geometry and pairing routes.
- Contract build and signed TestFlight archive from this checkout.
