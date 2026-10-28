# T3 Code clone handoff

Updated October 27, 2026, Toronto time. Nate is rethinking the plan for finishing
the one-to-one clone. Automatic coordination is disabled. Use this note when
choosing the next approach.

## Current state

The clone is incomplete. The desktop app is in `examples/t3-code`; the separate
iPhone and iPad app is in `examples/t3-code/mobile`.

These implementation checkpoints are pushed to GitHub:

| Work | Commit | Branch |
| --- | --- | --- |
| Root app modules and compiler fixes | `61b7997` | [Root checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-root-checkpoint-20261028) |
| App refactors and focused tests | `0e6c9ae` | [App checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-contract-apfs-r1-20261024) |
| Mobile implementation and release work | `250d863` | [Mobile checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-draft-apfs-r1-20261024) |

[PR216](https://github.com/ccheever/exact2/pull/216) remains a draft on a divergent
branch. The three checkpoints still need a deliberate integration plan.

- The desktop app extraction is unfinished. The complete proposed Root is
  2,059 lines against the repository's 1,500-line rule. Current Contract keeps
  ordinary resources, mutations and tasks at Root. No fitting replacement was
  selected.
- Focused compiler, chat and command tests passed. Whole-app behavior, visual
  parity and final integrated checks remain unverified.
- The InlineShortcut comparison has no complete verdict. Its cached test
  executable is missing, so another test run needs a fresh build.
- An iOS release build reached Apple validation. Export compliance still blocks
  tester availability. Apple account work remains deferred at Nate's request.

Application tests, technical drafts and local build evidence are retained.
Consult them for specific implementation questions rather than as a task queue.

## Basic next steps

1. Choose the implementation approach and exact reference version for each
   platform. Keep the existing code as the starting point.
2. Integrate the selected work into one branch. Resolve the Root structure and
   build blockers, then build and launch the current app.
3. List the reference app's screens, flows and states. Match them one at a time,
   including loading, empty, error, offline and relaunch behavior.
4. Compare the running apps on matching devices and settings. Check appearance,
   interactions, keyboard, accessibility, motion, persistence and performance.
5. Run the required checks on the final integrated code and confirm that the
   installable iOS build is available to testers.
