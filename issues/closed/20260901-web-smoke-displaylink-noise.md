# Web smoke flags display service diagnostics

**Status:** Closed
**Resolution:** Smoke suppresses only the exact headless CVDisplayLink diagnostic while preserving page and runtime failures.
**Systems:** Tooling, Web host
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1012

`node scripts/smoke.mjs web` passed its application, layout, motion, canvas,
and Contract assertions but exited 1 after headless Chrome repeatedly emitted
`CVDisplayLinkCreateWithCGDisplay failed` / `CVReturn -6670` diagnostics on a
machine without an active display service.

`scripts/agent.mjs:118-119` puts almost every Chrome stderr line into host
logs, and `scripts/smoke.mjs:172-173` fails any such line containing `error`.
This makes an environmental Chrome diagnostic indistinguishable from an
Exact/page error.

Classify browser-process diagnostics separately from console/runtime errors.
Either suppress this known headless display diagnostic or configure Chrome so
it is not emitted; keep page exceptions and Exact errors blocking.
