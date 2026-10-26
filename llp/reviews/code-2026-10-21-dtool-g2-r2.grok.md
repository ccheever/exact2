# Code review: contract gallery, the variant gallery's G2, round 2 of 3 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `362cf2d950ef1419e65e771cf0ff9da1a747571dee95db0ac877a7c1953670f3`), shared by both reviewers, each blind to the other's review; HEAD `2c681530f3`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** no review: the run ended after a context compaction, three times, with no findings.
- **Disposition:**
  All of Astra's taken in 12e5a2fcb7 (round 2's commit):
  - 1 (destination): under a node_modules or target anywhere in its path; neither app.json nor app.contract written over unless ours; never a link.
  - 2 (design assets): the gallery's `assets` links to the app's (tested with an icon SVG).
  - 3 (exported shape names): read from the file the specifier reaches (`use Info as PublicInfo`, tested).
  - 4 (app inside its package): the loader's recorded origin (tested).
  - 5 (aliases bypass the root check): every root the gallery reads must be one the app reads.
  - 6 (relative uses): refused in place.
  - 7 (lowercase exports): brought under a capital.
  - 8 (compiler records): placeholders from the type checker's own declarations (tested with TextDecorations).
  Grok: no review. Three runs each ended after the CLI's automatic context compaction, before any finding; the last run's narration is kept below. Round 3 (both reviewers) covers this round's change.

I'll review the gallery commit read-only: first confirm each round-1 finding is closed, then re-read the whole `HEAD~1..HEAD` change against the plan.The review skill's GitHub workflow doesn't match this brief. I'll stay read-only and review `HEAD~1..HEAD` against the plan and the round-1 findings.Round 1 is one commit on the original gallery. I'll read the current generator, the tests, and the loader paths those findings named.The generator is in place. Next I’ll check discovery, placeholders, and refusal against the loader and the type system.Auto-compacting conversation (81% full)...
Conversation compacted.
I'll confirm each round-1 finding against the current gallery code, then review the feature as it stands at HEAD.
