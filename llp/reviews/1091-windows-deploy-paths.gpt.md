# LLP 1091 §11 Windows deploy paths — review

2026-10-04, Codex. Scope: three inherited deploy path predicates, not a new
resolver or publisher protocol.

The parent independently read §11 and the retained `before.json` reproduction
before implementation. Approved the native path decisions and file scope;
requested the local canonical reconstruction equality to close case-distinct
NTFS sibling escapes without changing general `inside()` callers. That
condition is now explicit in §11 and its real-filesystem qualification.

The Windows host agent independently reviewed the proposal and found no
blocker: skip only graph builtins, refuse nonabsolute physical paths, check
consulted manifests, use native parsed roots including drive-relative paths,
and preserve exact native `node_modules` components. Its review did not run
tests or edit this worktree. The original guard probe is not evidence of a
successful end-to-end deployment escape.

The parent then independently reviewed the production helpers/call sites and
real-filesystem tests. No blocker found. Requested explicit missing physical
source and missing consulted-manifest assertions for the intentionally strict
realpath behavior; both are added. This source review is separate from tests
executed by the implementer.

The Windows host agent independently reviewed the final production/test diff.
Approved all three call sites, builtin-only skipping, absolute physical source
and manifest checks, native canonicalization plus reconstruction, and retained
registry trailing-component semantics. No blocker found. It did not rerun the
tests or repository gates. Neither review claims hostile-replacement safety or
linked native JavaScript execution.

The implementer ran the focused suite: eight tests, 42 assertions, no failures
or skips. Real junction, extended native paths, ordinary case aliases and
enabled NTFS case-sensitive sibling cases passed. Repository caps and boot
checks passed. Full private build, 2,326 default tests (54 existing ignores)
and strict all-target Clippy passed on base `39018dfa4`. The single all-workspace
format command hit Windows OS 206; the same 379 Cargo-selected targets passed
in 25 rustfmt batches with each target's edition, with the full inventory and
outcomes retained. The existing publisher test stops at its intentional Windows
durable stream-head refusal before completing cases. That limit is recorded,
not disabled; root accepted the focused production-path coverage for this
bounded correction. No GUI/browser, publication or linked-JS claim is made.

The final fetch added upstream `b79156175`, with no overlap in these four
owned paths. The implementer inspected its typed-inline/Lean expansion split,
rebased, and independently repeated build, 2,326 tests/54 ignores, strict lint,
caps, boot, focused eight/42 path checks and the exact 379-target formatting
inventory (25 passing batches). The actual old qualified compiler and new
compiler produce byte-identical 20,033-byte Skirmish plans and source-graph
JSON on the same frozen app and native Windows path. Those executed results
are recorded separately from the source reviews above; neither peer reviewed
or reran the whole upstream semantics change. No frozen consumer was changed.
The advisory difftest quick command reports nothing changed in its tracked
language directories, so its successful exit is not Lean execution evidence.
