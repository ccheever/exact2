# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `b85eba31b9`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two blockers remain. I reviewed the code and tests without running builds or tests.

1. **The 256-entry cap can discard an active fetch’s cancellation mapping.** [js/src/lib.rs:1400](js/src/lib.rs:1400) records streams too and evicts the oldest entry unconditionally. Ordinary replies remove their mapping, but [stream termination at line 1481](js/src/lib.rs:1481) does not.

   A client can keep one ordinary POST pending while opening and ending 256 streams sequentially. The POST’s mapping is then evicted despite its answer remaining parked. Aborting it reaches [take_aborted at line 777](js/src/lib.rs:777), which silently drops the unknown ticket: no transport cancellation or runner resumption follows. This needs only two simultaneous requests. It is a **new regression in the fix diff**; active mappings must survive the cap.

2. **The wasm page realm still misses wakes from storage let-go continuations.** [module-glue.js:135](host/web/module-glue.js:135) wakes only when `context` is absent. However, let-go storage delivery installs a [scratch context at line 216](host/web/module-glue.js:216). Its [independently scheduled chain at line 224](host/web/module-glue.js:224) need not produce any host reply.

   Concretely: retire an answer with storage outstanding alongside a held POST; its storage continuation aborts another held fetch. Door 22 records that abort with a non-null scratch context, suppressing the wake. With no unrelated completion, cancellation remains stranded. The [new page test](host/web/module-storage.test.mjs:251) exercises a null context, so it misses this case.

For **(a)**:

- **1 fixes the reported inline replacement sequence.** The [regression test](js/tests/it/abort.rs:289) asserts cancellation of exactly the first ticket and successful completion of the second. It does not cover table saturation.
- **2 is fixed.** The [empty pump branch](host/linux/src/presenter.rs:1366) now drains explicit cancellations. The [server test](host/linux/src/presenter/abort_tests.rs:119) exercises replacement, both pump halves, and connection closure. Android uses this [same presenter pump](host/linux/src/android.rs:505).
- **3 is partially fixed.** Native `Job::Forgotten` now publishes and wakes, with a [focused test](data/src/placed.rs:1070). Worker messages [wake unconditionally](host/web/module-glue.js:447). The [bridge test](host/web/src/abi/abort_tests.rs:65) verifies collection, cancellation output, and aborted resumption. These component tests leave the page let-go case above uncovered.

For **(b)**, I found no wrong-request cancellation or inherent wake loop in the reviewed fixes:

- Both mutex guards are dropped before [the native wake call](data/src/placed.rs:377).
- [abortWake’s `setTimeout`](host/web/glue.js:1375) prevents synchronous wasm re-entry and coalesces pending wakes.
- Collection follows wake acknowledgement and precedes outcome selection in [Linux](host/linux/src/presenter.rs:1336) and [Apple](host/apple/src/abi.rs:356); fallback selection and re-notification preserve the refusal alternation.
- [Ordinary reply removal](js/src/lib.rs:1262) removes the specific ticket. Stream retention is harmful through cap eviction, rather than ticket reuse.

For **(c)**, **4 is sound for the current in-tree composition**: [Fieldnotes pairs JS with `Placed<Backup>`](apps/fieldnotes/data/src/lib.rs:39), and [Rust ABI requests have `fetch: None`](logic/abi/src/lib.rs:502). This is not a general namespacing guarantee: [Mixed concatenates aborts unchanged](data/src/mixed.rs:565).

**5 is acceptable as an explicitly documented limit**, but qualify it as applying to native and wasm targets. Targetless callbacks enter through [source.rs:375](js/src/source.rs:375); the web JS target already [passes the signal to browser fetch](host/web-js/admission.js:115).

For **(d)**, beyond the cap regression, I found no additional runtime regression in the fix diff.

Verdict: blockers — active fetch eviction at the cap; missing wasm page-realm wake during storage let-go delivery.
