surfaces: macos
agent: Claude Code (Claude Opus 5.5)
task: the Mac transcript dropping frames as it scrolls, on a real account

### Rough

- On the fixture the end of the transcript scrolled and bounced cleanly; on Charlie's
  account it jerked. A ⌥⌘T trace from his MacBook showed two causes. Of 82 missed frames,
  67 followed scroll events that moved the content, each about 10 ms of main-thread work
  with no change to the tree; the fixture's rows cost about 0.3 ms, so this waits on a
  fixture account as heavy as his (QUEUE). Another 8 followed a gateway update that
  re-asked the server list, the rail, the direct messages, the channel list and the
  deep-linked channel, losing 13 to 22 ms that no runner phase claims: the data module's
  own time. That happened 214 times in 283 seconds, and each left the view as it was.
- The lists shared one version, which moved with any change in any channel but the one on
  screen: a typing indicator, a reaction or a message anywhere on the account. The
  fixture's account is one quiet server, so the stall never showed there.
- Each list now has its own version: the server list and rail move with what they show
  (unread and mentions, names, order), the DM list and the channel lists also move with a
  message or read in a channel they list, and typing elsewhere moves none of them. Twelve
  events in another channel (six typing, six messages) asked 60 list queries before and 6
  after.
- The same trace's plan sites put 31,193 evaluations in the member list, 98 rows built 318
  times, all of them in the member sheet, a `dialog` that was never open: a closed dialog
  still builds an eager `for`'s rows (`docs/agent-pitfalls.md`), and the members resource
  moved with every presence op 37 sent for the list's range. The sheet now builds its rows
  only while it is open, and the resource follows presence only while a member list shows.
- A `sample` of an agent-driven drive put most of each wheel in the presenter's pass over
  the whole window, for batches that carried no ops. A change that skipped those passes for
  an empty batch broke four XCTest classes, which apply an empty batch to mean "read the
  window again" (a window that arrived, a list that changed direction), and nearly all of
  the empty batches turned out to be the agent's own clock steps (`docs/agent-pitfalls.md`).
  The change was dropped.

- A second trace from his account (e2fc73048e) put the frames it still missed in frames
  that built rows: about 5.6 ms of apply, 4 ms of runtime and 8 ms no phase claims. Its plan
  sites counted each row site `moved` about 9,500 times, about five rows a layout; a long
  scroll back through #general on the fixture shows the same ratio (94 rows built, 356
  layouts, 1,755 moves of a row's root). `moved` counts every node whose absolute frame
  changed, so a row shifting in the list counts all its nodes, while the Apple host emits
  only the parent-relative frames that changed (`host/apple/src/layout.rs`): it is not the
  cost.
- His rows are not heavier than the fixture's: the trace built the same sites, one plain
  text run a row and few pictures. Each frame that built rows waited on the owner thread
  about 14 times (`Owner.sync`, the trace's runtime segments): macOS still builds a moving
  list's rows on main, which iOS stopped doing (LLP 1072 stage 5). Turning that on for
  macOS took his A/B to no late frame in 600, but the ordering it needs kept failing review
  (QUEUE, `parked/mac-async-fills`), so the jank stays until it lands.

### Lean in

- The trace named the work by plan site once it was joined to a plan of the same contract
  (`agent.mjs trace <file> --plan`). The journal's `query` lines between scroll lines
  showed which resources re-asked, which a summary of the late frames alone did not.

### Needed

- A late frame's unattributed time names no cause; the data module's calls were found
  only by reading the journal lines around it.
- Under the agent no frame is presented, so a fixture drive measures what work runs, not
  how long it takes; and the agent's own batches swamp a profile of it.

### Checkpoints

- macOS, the fixture: six typing indicators and six messages in #help with #general open
  ask the channel list six times, and nothing else.
- web: `app.test.contract` 80 of 80, `routing.test.contract` 4 of 4; `bun test` in
  `apps/excord`, 656 pass. In a narrow window the member sheet opens on the fixture's 19
  rows, which were held before it opened.
- Not seen yet: Charlie's account on his MacBook, which is where the jerk was.
