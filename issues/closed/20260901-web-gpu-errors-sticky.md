# Web GPU errors never clear

**Status:** Closed
**Resolution:** GPU module and ABI errors are now consumed once in sequence instead of remaining sticky.
**Systems:** GPU, Web host
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1009

The web GPU ABI error accessor clones the stored error without clearing it
(`gpu/src/web.rs:94-101`). After one parse/create failure, later unrelated
errors remain hidden behind the first message forever. The native accessor
takes and clears its equivalent slot.

Give both ABIs the same consume-on-read semantics and add a sequence test:
error A, read A, successful call, error B, read B, then no error.
