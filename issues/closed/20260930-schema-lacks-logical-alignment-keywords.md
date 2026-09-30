# The schema's alignment enums lack `start`, `end`, `self-start` and `self-end`, which Taffy has

**Status:** Closed
**Resolution:** Schema exposes start/end and item/self self-start/self-end; Contract exposes all five alignment rows; literal Chrome LTR/RTL fixtures and compiler vocabulary tests pass.
**Systems:** kernel (schema), Contract compiler, web host
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 §5, LLP 1053 (direction), kernel/tables/schema.json `AlignItems`/`JustifyContent`/`AlignSelf`, vendor/taffy/src/style/alignment.rs

Taffy 0.14 resolves `start`, `end`, `self-start` and `self-end` (writing-mode and direction relative), and `safe`/`unsafe`. The schema's `AlignItems`, `AlignSelf`, `JustifyContent`, `JustifyItems` and `AlignContent` offer `flex-start`/`flex-end`/`center`/`stretch`/`baseline`/`normal` and the space keywords only, so an RTL-aware app cannot say `align-items: start`, and the LLP 1074 fixtures had to write `flex-start` in grids. Exposure work, not layout work: enum values, Contract validation, CSS emission (the names are CSS's own), and a differential case per keyword under `direction: rtl`. `safe center` is proposed separately in LLP 1054.000.001. The other unexposed Taffy capabilities are listed in LLP 1074 §5: `display: flow-root`, the preferred-size keywords (`min-content`, `max-content`, `fit-content`, `stretch`), `overflow: clip`, `justify-self`, named grid lines and areas.
