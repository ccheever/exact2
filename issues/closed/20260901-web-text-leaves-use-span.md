# Web text leaves use inline spans

**Status:** Closed
**Resolution:** The web host now emits spans only for true Text-under-Text inline runs and divs for ordinary text leaves.
**Systems:** Web host, Kernel
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1007 §1, LLP 1001

LLP 1007 §1 specifies `Text` as `div`, with `span` only for an inline run. The
kernel defines that predicate precisely as a `Text` whose parent is `Text`
(`kernel/src/arena.rs:220-225`).

`host/web/src/host.rs:524-546` instead emits a `span` for every non-root Text
node that owns a `text` prop. That includes ordinary text leaves under a bare
View. The page reset does not blockify spans, so normal-flow display differs
from both the stated tag mapping and the kernel's bare-block default. The web
test merely checks that some span exists and cannot distinguish an actual
inline run from a regular leaf.

Select the tag from the kernel inline-run predicate (or the parent node type),
and add separate batch/real-DOM tests for Text-under-Text and Text-under-View.
