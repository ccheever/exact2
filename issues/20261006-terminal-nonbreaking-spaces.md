# Preserve nonbreaking whitespace in terminal text flow

**Status:** Open
**Systems:** terminal measurement, text flow
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** CSS whitespace oracle; host/terminal/src/measure.rs

`host/terminal/src/measure.rs:77` classifies every grapheme whose characters satisfy Rust's `char::is_whitespace` as a breakable one-column ASCII space. That includes U+00A0 NBSP and U+202F narrow NBSP. The shared collapse pass preserves NBSP, but the terminal tokenizer then discards its identity and allows a break.

Reproduced with this entry and `exact-terminal entry.contract --size 4x6 tree document`:

```contract
component App
  view
    column
      text "XX YY" testId="nbsp"
      text "Ready"
```

The NBSP text was laid out at 4×2 and printed as separate “XX” and “YY” lines. Under normal CSS whitespace/overflow wrapping, that unbroken five-cell word must not gain a break at NBSP.

Use the shared CSS whitespace distinctions and Unicode line-break opportunities rather than `is_whitespace` as the wrapping rule. Preserve the actual grapheme and its measured width.

Acceptance: NBSP and narrow NBSP remain nonbreaking through measurement and printing; ordinary spaces and preserved tabs/newlines keep the profile's specified behavior. Check normal, pre-wrap and explicit overflow-wrap cases.
