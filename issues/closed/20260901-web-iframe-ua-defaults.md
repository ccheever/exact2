# Web iframe keeps browser default geometry

**Status:** Closed
**Resolution:** Web iframes now explicitly match the intended block, borderless, content-box 300 by 150 default.
**Systems:** Web host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1007 §1, LLP 1020

The web reset covers images and controls but not iframes
(`host/web/index.html:5-20`). A Chrome computed-style probe for a kernel 300×150
WebView produced a 304×154 inline element with `2px inset` border and
`box-sizing: content-box`. The kernel and native wrappers describe 300×150
block-like geometry.

Batch tests inspect emitted JSON and therefore miss the browser's UA border
and display defaults.

Reset iframe border/margin/display/box sizing to the declared web-standard
kernel mapping, then add a real-browser computed-rect and hit-test assertion
for a bare WebView.
