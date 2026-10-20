# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~3..HEAD, the change, Expose's conversion and the first two rounds.
- **Method:** one brief (sha256 `7704276c616e726fc2591a34ad1dc51e26009e423206125c186fa02228d1fe57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** nothing blocking; a P3 (the JS scrub refusal naming the old inline cases).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

I'll review the stage 6 commit C changes read-only, starting with the review skill and the round-2 fixes against the earlier rulings.This is a read-only review of the last three commits, so I’ll inspect the diffs and the named call sites without touching the worktree.The round-2 family lookup is the risky part. I’ll trace how a stack is keyed from the compiler through the JS map.Auto-compacting conversation (80% full)...
Conversation compacted.
I'll finish the read-only pass on the remaining round-2 and round-1 paths, then report only defects that still fail a concrete case.Nothing is blocking. Round 2's fixes still hold, and so do round 1's.

[P3] JS scrub error still names the old inline cases — `host/web-js/dev.mjs:444`

A value-only edit of `type.body` (or `textFace`) is refused, and the page rebuilds on release, which is what the protocol now says. The error still says the token "is written inline on this page (a font weight, an SVG attribute)". Someone scrubbing a type or family token is told to look for a `font-weight` or SVG attribute that is not there.
