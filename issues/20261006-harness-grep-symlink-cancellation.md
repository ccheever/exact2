# Stop harness grep on cancellation and avoid repeated symlink traversal

**Status:** Open
**Systems:** Harness data, Harness tools
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** apps/harness/data/src/tools.rs:337, apps/harness/data/src/agent.rs:457, LLP 1101.001 P17

The grep tool follows directory symlinks with `Path::is_dir`, pushes every child, and keeps no visited-directory identity. Its 200-match limit bounds successful hits only. The `cancel` argument to `tools::run` is passed to bash but never to grep.

Verified using the actual tools.rs implementation in a temporary harness: make a folder containing `left` and `right`, both symlinks back to that folder; search for an absent substring. Even with the cancellation flag set before the call, it is still running after two seconds and must be killed by its recorded PID. The OS's link-resolution limit bounds an individual pathname, but two links create an exponential number of equivalent paths before that limit.

Interrupting a turn changes the visible harness state; this old tool thread can continue consuming CPU and retaining its session after the turn is gone. Real repositories can contain linked workspaces or generated link trees.

Do not descend directory symlinks by default, or track stable directory identities if following them is supported. Pass cancellation into filesystem traversal and check it while enumerating and reading. Bound traversal work independently of the number of matches and explain any partial result.

Acceptance: cyclic and multiply-linked folders terminate promptly without duplicates or explosive work. A pre-cancelled call performs no traversal; cancellation stops an in-progress no-match search. Normal deterministic search order remains covered.
