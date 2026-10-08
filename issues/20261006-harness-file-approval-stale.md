# Bind harness file approval to the file contents shown in its preview

**Status:** Open
**Systems:** Harness data, Harness approvals, Filesystem tools
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** apps/harness/data/src/agent.rs:375, apps/harness/data/src/tools.rs:137 and 252

The harness computes a file diff for approval, waits for the person, then recomputes `change(name, input)` during execution. It retains no expected preimage from the approved preview. An intervening file change can therefore be overwritten without appearing in the approved diff.

Verified using the actual tools implementation: preview writing `approved-before\n` to `approved-after\n`; replace the file with `concurrent-edit-never-shown\n` while approval is pending; run the approved call. It reports success and overwrites the intervening text. The approved preview never showed that deletion.

This matters with a human editor, formatter, another agent, or a changing symlink while an approval is on screen. For `edit_file`, the one-occurrence check validates the new text but does not verify that the reviewed preimage is still current.

Prepare a concrete edit containing the target identity, expected preimage and resulting bytes. After approval, check that the target still matches before applying that edit. A changed target should return a conflict or request a fresh preview. Prefer a safe atomic replacement where appropriate, preserving the file's intended metadata.

Acceptance: the reproduction refuses or asks again and preserves the concurrent edit. Unchanged approved writes and edits still succeed; new-file previews detect a file appearing before execution; changed link targets do not silently redirect an approved operation.
