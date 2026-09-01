# Rejected Linux reload installs candidate fonts

**Status:** Closed
**Resolution:** Linux now stages a candidate text engine and retains the running font catalog when reload boot refuses.
**Systems:** Linux host, Fonts
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1019, LLP 1023.001

Linux installs a candidate plan's font catalog before the candidate runner has
booted successfully (`host/linux/src/presenter.rs:206-227`,
`host/linux/src/text.rs:299-383`). If boot is refused, the last-good runner
continues but now measures and paints with the rejected plan's fonts.

Stage font resolution/catalog state beside the candidate presenter and commit
it only after runner boot succeeds. Add a failed-reload fixture whose candidate
uses different faces and assert the old app's measurements/catalog remain
unchanged.
