# Stream and bound harness file reads before building tool output

**Status:** Open
**Systems:** Harness data, Harness tools
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** apps/harness/data/src/tools.rs:281, LLP 1101.001 P17

`read_file` reads the entire file with `std::fs::read` before checking its first 8,000 bytes for binary content or applying the requested line offset and limit. It builds the selected output and transcript lines before the 100,000-byte model-result cap. A small requested window is therefore not a bounded read.

Verified by calling the actual tools.rs implementation from a temporary Rust harness (only presentation and unused shell dependencies stubbed): `read_file({path: <8 MiB sparse file>, limit: 1})` immediately reports a binary file, but a counting allocator records an 8,388,608-byte allocation first. A multi-gigabyte binary file would allocate it all to return the same short error. A giant text line is also copied into transcript output even when the model result is truncated.

The shell tool already keeps a bounded head and tail; filesystem tools should give similarly reliable behavior when an agent encounters logs, datasets, generated files or binaries.

Sniff a bounded prefix before reading further, stream to the requested line window, and enforce output limits while accumulating. Bound an individual line and the transcript representation as well as model text. Include cancellation during scanning to a distant offset. Clearly report truncation.

Acceptance: requesting one line from a large binary or text file uses bounded memory; huge single lines do not enter the transcript intact; cancellation stops a long scan; ordinary offset/limit and Unicode behavior are preserved.
