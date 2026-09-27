# `clip-path: path('M0 0 Z 1')`, and the same `d` on an SVG `path`, hangs the kernel and grows memory without bound

**Status:** Open
**Systems:** Kernel (`kernel/src/svg/path.rs`, `kernel/src/clip.rs`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1055.000 (SVG `d`), LLP 1001 (`clip-path`)

After a `Z`, a trailing number makes the parser repeat the close-path command implicitly. `Z` takes no arguments, so the repeat consumes nothing, and the loop spins, appending to `ends` on every iteration (`kernel/src/svg/path.rs:147-156`, `:249`).

**Reproduced:** neither `ClipPath::parse("path('M0 0 Z 1')")` nor `svg::parse_d("M0 0 Z 1")` had returned after 5 s.

**Origin:** the loop is in Charlie's SVG `d` parser (`cb57a5d1`), and was already reachable from `path d=`. Seth's merge `a5f03a59` also exposed it through CSS `clip-path`, where `clip.rs` now calls `parse_d_whole`. A style value an app computes at runtime can hang the host.

**Fix:** refuse numbers after `Z`/`z`, as the SVG grammar does (and browsers stop rendering the path at that error), and assert that each command iteration consumes input. Add both strings as tests.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max; reproduced by the verifier. Verification: reproduced.
