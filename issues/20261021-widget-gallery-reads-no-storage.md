# The widget gallery shows an empty timeline for a widget that reads storage

**Status:** Open
**Systems:** widget gallery (`WidgetGallery.swift`), scripts/agent-widget.mjs, docs
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' mileage widget reads saved totals and a photo from the app's shared data. In the development gallery it shows an empty timeline, with white margins around the coloured widget, though the real widget on the Home Screen shows the data. The lane spent its time checking the saved store before finding that the gallery reads none.

## Cause

By design, the gallery's in-process stand-in makes no storage reads or requests.

## Fix

Let the gallery read the app's saved shared data when asked (a flag, or by default under the agent), and say in its output when a timeline was built without storage. Put that limit beside the gallery commands in the widget docs.
