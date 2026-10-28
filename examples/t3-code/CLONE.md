# T3 Code clone handoff

Updated October 28, 2026, Toronto time. Finish the one-to-one clone with pstack
handling routine execution and review. The legacy scheduled coordinator stays
disabled. Use this handoff when taking over the work.

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
branch. Integrate the checkpoints into the working branch as concrete dependencies
require them.

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

## Execution milestones

Nate chose iPhone first, then iPad. Desktop remains in the overall plan.
Keep the complete one-to-one goal across these milestones.

1. **Get the iPhone app running.** Start from mobile checkpoint `250d863` on one
   working branch. Keep its reference pin `365aa87982`. Use the normal mobile
   build from `mobile/README.md` and fix actual build or launch failures.
   Finish with Home open in the simulator and a saved screenshot.
2. **Prove environment connection.** Drive Home, Add environment, connection
   failure, successful connection, and return to Home. Restart the app and
   confirm the environment remains saved.
3. **Finish the main chat flow.** Open a project, create a thread, send a message,
   receive streamed output, stop a response, and reopen the conversation after
   restart. Show the working flow and run its focused tests.
4. **Close the remaining phone differences.** Use a short list of reference
   screens, flows and states. Match each through the real interface, including
   settings, attachments, terminal, loading, empty, error and offline behavior.
   Finish each slice with comparison screenshots and a verified commit.
5. **Complete iPad and desktop parity.** Repeat the flows on each target and fix
   its layout, navigation, keyboard, accessibility and motion differences.
   Desktop uses a separate reference version and needs its own integration work.
6. **Verify and deliver.** Run the required checks on the integrated code, check
   persistence and performance, and confirm an installable build and tester
   access. Resume Apple account work when Nate chooses to do so.

Mobile selects its own Contract entry. The unfinished desktop Root extraction
does not inherently block the first mobile build. Bring other checkpoint changes
into the working branch when a concrete build or behavior dependency needs them.

## Execute with pstack

Use `pstack:poteto-mode` to select the applicable playbook and handle implementation,
debugging, and review. Follow Nate's configured pstack model roles. Choose the
smallest working change that advances the next app flow.

- Routine source edits, dependency installation, builds, tests, simulator use,
  local commits, and normal pushes to PR216 are authorized. Make reversible
  implementation decisions without asking Nate to manage the work.
- Start with one implementation owner. Delegate bounded work on disjoint files
  after the iPhone baseline runs. Give shared entry points, build output, and
  each simulator one owner. Review independently at working milestones.
- Build, drive the changed flow, fix observed differences, then commit and push.
  Use focused checks during development and run the required integrated checks
  before delivery. A passing build alone does not establish parity.
- After three failed fixes for one target, stop that approach and report the
  blocker briefly. Continue independent flows within the existing scope.
  Collect decisions only Nate can make into one handoff after independent work.
- Keep progress in this file and the PR. Report the working flow, remaining
  mismatch, and pushed commit. Keep useful logs and screenshots with the work.
- Finish when the reference flows, visual behavior, persistence, accessibility,
  and performance are verified on iPhone, iPad, and desktop. Record incomplete
  acceptance explicitly. Apple account work remains deferred.
