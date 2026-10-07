---
name: 20261007-let-go-banner
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-let-go-banner
pr_url: https://github.com/ccheever/exact2/pull/190
verified_commit: null
---

# A superseded request no longer shows an error

## Outcome

In normal use the clone showed a red banner, "the answer was let go before this reply; the
request may already have been sent". T3 Code shows nothing when a request it replaced is
cancelled. Now a request Exact let go is not a failure anywhere in the clone: no banner, no
toast, no failure field. A real failure (network down, server error, timeout) still shows the
reference's error.

## Scope and exclusions

In scope: every place the clone turns a failed native call into user-visible error state (the
transcript banner `client.error`, toasts, cached `error` / `failure` fields), and one rule for
"let go" with tests. Excluded: framework code (`js/src/prelude.js` is the cause and stays as
is); view answers that only return an error in their value (a let-go answer's value is dropped).

## Context and guidance

Parent specification: [spec](../../spec.md). Brief: wave 4, ADDENDUM 9–11. Port range 16440–16459.

Cause: main `a19523a57` (merged into the feature branch in `4cdb8aa63`) made the native runtime
reject a let-go answer's in-flight fetches with `FetchError { kind: "Aborted" }` so its
`catch`/`finally` run (`js/src/prelude.js` `__exact_forget`; LLP 1016 D5; docs/agent-pitfalls.md
"A superseded send's fetch rejects natively"). Before, the continuation vanished. The clone's
native calls go through `native.later`, a `fetch("exact-native:")`, so they now reject. The
`snapshot` resource asks again on every `t3.status`/`t3.events` change; the older answer's
`client.refresh` is let go mid-drain, its `call()` wrapped the rejection as a `transport`
ClientError, and `refresh`'s catch wrote it to `client.error` (its epoch was still current: the
newer refresh had not had its status reply yet). The banner reads `client.error`
(`requests.ts`). After the first rejection the continuation's next native calls run outside any
answer and are refused with a plain `Error("fetch() called outside an answer …")`.

Reference: T3 Code `1e2ecbd975` `settings/ThemeSearchSection.tsx` and `OpenSourceLicenses.tsx`
set an error only `if (!controller.signal.aborted)`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| main merge | feature branch | `4cdb8aa63` | `a19523a57` in the feature branch | `git merge-base --is-ancestor` |

## Implementation notes

- `let-go.ts`: `letGo(error)` is true for the runtime's `FetchError` kind `Aborted` and the
  clone's ClientError kind `superseded`; `letGoAware(native)` turns the rejection into a
  `superseded` ClientError and refuses that answer's later calls the same way.
- `app.ts` `answer()` wraps `native` once per answer. The existing `superseded` paths
  (`refresh`, `command`, `write`) are then silent.
- Every catch that writes an error, toast or failure field asks `letGo` first and rethrows (or
  skips the write and keeps its cleanup: a `saving`/`acting` flag, a loading toast dismissed).
  Caches (`palette` search and browse, PR list/detail, usage, image search, diagnostics, device
  hosts, media URLs, timeline item detail, lazy diff files) are not written, so the next ask
  reads again.
- `r6-pr-actions.ts` showed an error toast for a `superseded` prepare; it now dismisses the
  loading toast instead.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| No banner on a let-go refresh | lane server 16440, fixture repo project | pair, onboarding, settle | no `error-banner`; state `error` "" | macOS | before/after drive |
| Real failures still shown | Bun fakes | `let-go.test.ts` | Network refresh/send failure shows; refused copy toasts | — | tests |
| Rule in one helper | — | `let-go.test.ts` | Aborted / superseded let go; Network, Timeout, transport not | — | tests |
| Regression gates | — | clone checks, five checks | green | macOS | numbers below |

## Progress

Implemented 2026-10-07 on the feature branch `0a7ca50ad`. Verification: unverified.

Not run: oracle and trace-diff rows (not run); a let-go send in a live drive (the drive shows
the snapshot and branch reads; sends are covered by tests).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | `86ee667eb` on `0a7ca50ad` | `bun test examples/t3-code` 2198 pass / 1 skip / 0 fail (base 2190; +8 in `let-go.test.ts`, the reproduction among them); strict tsc (`--target ES2023 --lib ES2023,DOM`) clean; contract build 2535 slots, 45 resources; `cargo test -p t3-code-macos --lib` 11/0; no AppKit binary touched; caps pass; five checks pass (cargo build, cargo test 3310 pass / 0 fail / 32 ignored, clippy, fmt, caps, boot); macOS bundle builds | drive records below; PR screenshot | none |

Drives: one BEFORE (`t3-code-evidence-base` at `0a7ca50ad`) and one AFTER, each one
`agent.mjs macos --size 1280x840` call: pair with a fresh link, Continue, Continue, "Do not
import projects", settle, `clock +2000`, settle, screenshot, `tree`, `state`, `logs`. Lane
server 16440 with the fixture repo added as a project (`target/lane-letgo`, not committed).

```
BEFORE  logs: query data: snapshot / console: fetch() called outside an answer … / forget request 72 (data)
        tree: View#500 [error-banner] > Text#504 "the answer was let go before this reply; the request may already have been sent"
        state: "status": "Connected", "error": "the answer was let go …", "clientError": "the answer was let go …"
        ref picker: "Select ref", no checkout strip (the branches read let go was cached as a failure)
AFTER   logs: forget request 48, 51, 72 (data), 62 (branches), 86 (details); no "fetch() called outside an answer"
        tree: no error-banner
        state: "status": "Connected", "error": "", "clientError": ""
        ref picker: "main", checkout strip "Current checkout · main"
```

## Next action

Review the PR.
