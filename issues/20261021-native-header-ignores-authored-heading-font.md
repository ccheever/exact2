# A native header or sheet title ignores the heading's authored font

**Status:** Open
**Systems:** Apple host navigation bar projection, docs
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

A sheet's heading written `font-family="Filson" font-weight=800` draws in UIKit's regular navigation-title font. Brooks' source uses its brand face for every bar title.

## Cause

The native bar's title is projected from the heading's text only; its authored typography is not carried to `UINavigationBarAppearance`. The port sets the font from its own `navigation(_:)` hook.

## Fix

Project an authored `font-family`, `font-weight` and `font-size` on the header's heading into the bar's `titleTextAttributes` and `largeTitleTextAttributes`, as CLAUDE.md's precedence asks (what the author writes wins). Until then, document the restriction beside native headers.
