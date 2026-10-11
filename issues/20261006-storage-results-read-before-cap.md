# Bound native storage reads before allocating and encoding their results

**Status:** Open
**Systems:** Native storage, Data host, ibex2 filesystem
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1027.001 D2, data/host/src/native.rs, vendor/ibex2/src/stdlib/fs.rs

Native storage enforces its 16 MiB result limit after the file has been read, base64-encoded into a JSON value, and serialized. `native::run` checks the serialized length at data/host/src/native.rs:273–277. Document reads reach `std::fs::read` through `run_document`; the ordinary executor has the same whole-file read.

Verified with the actual storage continuation and a counting allocator: reading a 20 MiB temporary document returns `Unsupported: storage result exceeds its byte limit`, but the largest individual allocation is 55,924,078 bytes. The declared result cap is 16,777,216 bytes. Multiple copies coexist before the refusal. Larger files can exhaust the native process before it produces the bounded failure; a worker does not isolate the process heap.

This is separate from the update-store issue: it is the app's ordinary storage executor, reachable by a granted file read.

Limit reads while performing them, accounting for the encoded response's overhead before constructing it. An initial metadata check is useful but does not cover growth or special files; retain a read-time limit. Bound directory listings and other result-producing operations by the same response policy before materializing a giant result.

Acceptance: oversized files and a file that grows during reading produce the existing bounded refusal with memory independent of total input size. Boundary-size byte payloads still work, in Rust and TypeScript, and all native filesystem paths follow the same rule.
