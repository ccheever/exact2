# Code review: web press paint keeps authored transitions, keptBase narrow re-check (once, outside the cap) (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `1e5ec33b39`.
- **Method:** one brief (sha256 `aef4225ab2a81e8f397fae1000de964bf9b63d1a824cc1252c33063296b4de14`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

No findings.

`keptBase` is updated only at `host/web/input-glue.js:394`, after the cancel loop at `:354`. The loop still compares a transition that this read started (`!before.has(a)` and `a.transitionProperty in was`) with the base stored for that kept declaration. `was` is the pre-clear kept set (`:343`); `kept` is the post-clear set (`:346`). A keep carries the previous base; a write stores the value just read.

- **First write.** A new press builds a new state (`:469`). `state.inputs` is unset, so `kept` is cleared and `state.rule` is absent, so nothing is applied before the read. Opacity stores `String(held ?? base)` and color stores `cs.color`, the same values the new declarations are built from (`:370`, `:380`).
- **Keep.** Style, class, and `stale` are unchanged, so the declaration is copied through (`:367`, `:378`) and the base is the previous `state.keptBase` for that property. The re-read runs with that declaration applied (`:349`), and the painted `cs.color` / opacity is not stored.
- **Drop and rewrite.** An input or `stale` change clears `kept` after `was` is saved (`:344`). The cancel still sees the old base. The rule is held empty, the glue transition is cancelled, then `cs.color` and `computedStyleMap` opacity are read for the new declaration (`:380`, `:365` — both after `:354`). That is the base stored.
- **One property kept, the other rewritten.** The clear is all-or-nothing, so this is a later settle: the kept property carries its base; the other was not in `kept`, so its declaration was off during the read and its base is taken from that read.
- **endPaint, and a re-press on the same state.** `endPaint` (`:283`) drops the map entry; the next press allocates a new state and takes the first-write path. A re-press before the release finishes reuses the state with the same inputs, which is a keep, so the original base stays.

The cancel comparison is unchanged and now sees that origin: a target equal to it is the re-read’s own transition; any other target is the node’s base change.

Verdict: clear — `keptBase` is the look each kept declaration was written from, and the cancel loop judges against that look.
