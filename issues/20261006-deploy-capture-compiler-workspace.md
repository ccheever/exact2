# Restore deploy-capture tests after Contract source discovery

**Status:** Open
**Systems:** deploy source capture, build scripts, verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** QUEUE.md four app.test.mjs deploy-capture failures; LLP 1091 D10

Four source-capture tests currently fail before reaching their intended assertions. `scripts/deploy.mjs:193` invokes `cargo run -p contract ... sources` from the supplied Exact root. The shell fixtures deliberately contain a smaller game workspace without the Contract compiler; the invocation reports `package(s) contract not found in workspace`.

Reproduced with the pinned Bun 1.4.2: `bun test ./scripts/app.test.mjs` reports 76 passed, two skipped and four failed:

- deploy excludes generated shells and regenerates them from captured game source
- R12 deploy captures initialized dependency submodules as source
- R13 capture refuses tracked files under inferred game output roots
- R14 external game capture refuses tracked output roots

QUEUE already records the regression. These tests guard reproducible delivery and rejection of generated/tracked output contamination. They are absent from both the five checks and the current async glue glob, so their continued failure is easy to miss.

Give the fixtures access to the compiler from the actual SDK workspace, or make source discovery select the authoritative compiler while retaining the capture root under test. Do not bypass package discovery or weaken the existing provenance assertions. Add this existing test file to appropriate asynchronous script verification.

Acceptance: all four tests exercise their intended positive/refusal paths and pass under a frozen install. Test temporary workspaces without a local compiler package and an external game app. The async lane detects a deliberate source-capture regression without adding a blocking check.

## Audit (2026-10-08)

The current main-based run of `bun test scripts/app.test.mjs` passed 84 tests, skipped two, and timed out after 30 seconds in R12 dependency-submodule capture; its unfinished continuation reported `contract sources did not answer`. The other three listed source-capture fixtures passed. Keep the remaining R12 timeout and async coverage question open; the original four-failure count is historical.
