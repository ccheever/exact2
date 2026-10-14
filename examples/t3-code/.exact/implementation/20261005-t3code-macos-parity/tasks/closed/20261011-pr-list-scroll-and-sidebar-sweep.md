---
name: 20261011-pr-list-scroll-and-sidebar-sweep
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-list-scroll-and-sidebar-sweep
pr_url: https://github.com/ccheever/exact2/pull/468
verified_commit: 1f813789b
---

# The Pull Requests list's place across a window refocus, and the sidebar sweep's hover

## Outcome

The user decided on 2026-10-11 that these two framework gaps recorded but never filed are built now, matching the
reference T3 Code: reproduce each in the clone first, fix it in the clone if the clone causes it, and if it is a real
framework gap, write a minimal one-file repro for the coordinator to file on main (keeping or adding a clone workaround).

| Id | Clone (recorded) | Reference (check it first) |
| --- | --- | --- |
| PS-1 (X58) | The Pull Requests list goes back to its top when the window is focused again and the list re-reads (pr-list-live-refresh's investigation A and attempt 4); a re-read with no focus change and the minute tick keep the place | `_chat.pull-requests.tsx`: the list reads again on the window's `focus` (`useLiveRefresh`); React keeps the rows (`answered ?? carried`), so the reader stays where they were |
| PS-2 (X13) | During a row-action sweep the hover card or a control's tooltip open at the press stays until the release (the host sends no hover events during a pan); Escape cancels through an invisible window shortcut (r12); after a cancelled sweep the hover is not where the pointer is | `Sidebar.tsx` `startActionSweep` / `Sidebar.pointer.ts` `SidebarPointerSensor`: past 6 px the list takes `**:pointer-events-none`, so the card or tooltip goes as the sweep takes over; Escape (document `keydown`, capture) cancels; a cancelled release applies nothing and its click is swallowed; the rows take the pointer again |

## Steps

1. Reference first: the sweep driven with trusted mouse and key events over CDP on the reference lane (`ref-app.sh`,
   five fixture threads in the lane's T3 home); the list's live refresh read from source.
2. PS-1: reproduce on the feature tip's build (`t3-code-evidence-base`) with the GitHub lane (sandbox
   `daehyeonmun2021/playground`, 99 open pull requests) paired through Settings › Connections: wheel down, the window
   refocused (`prefer has-focus`), with and without a list read due; find what moves the list (the scroll node's id
   and offset in `layout`, the runner's trace in `logs`).
3. PS-2: the same sweep on the clone's agent driver (`tap … down`, `tap move`, `type … key Escape`, `tap up`), before
   and after, with the card open at the press and with the focus already on the pressed button.
4. Fix the clone's side; a framework cause gets a one-file repro in the report and a row in `EXACT2-GAPS.md`.
5. Tests; one agent drive of the after build; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| PS-1 | reproduce, then the cause; agent drive (window refocus with a list read due, the scroll node's id and offset) | before / after text record |
| PS-1 under a real refocus | real-input steps | — |
| PS-2a: the card or tooltip goes as the sweep takes over | reference (CDP, trusted events); agent drive with the focus already on the pressed button; Bun test | before / after / reference images |
| PS-2b: Escape cancels the sweep, the release applies nothing | reference; agent drive; Bun test | before / after / reference images |
| PS-2c: after a cancelled sweep the row under the pointer is hovered, through the release, no card until the pointer moves | reference; agent drive; Bun test | before / after / reference images |
| PS-2 under a real trackpad drag and real Escape | real-input steps | — |

## Reference

- PS-1: `apps/web/src/routes/_chat.pull-requests.tsx` (`useLiveRefresh(() => refreshList(true))`, `listData =
  answered ?? carried`, the list's `scrollRef`), `apps/web/src/hooks/useLiveRefresh.ts`.
- PS-2: `apps/web/src/components/Sidebar.tsx` `SIDEBAR_DRAG_DISTANCE = 6`, `startActionSweep`
  (`setActionSweep`, `**:pointer-events-none` on the list while `actionSweep !== null`), the row's
  `handleActionPointerDown`, the Settle button's `Tooltip` (`TooltipPopup` "Settle thread") and the row's
  `ThreadHoverCardPopup` under one `TooltipProvider` (`delay={150} closeDelay={0}`);
  `apps/web/src/components/Sidebar.pointer.ts` `SidebarPointerSensor` (`keydown` Escape → `cancel`, `finish`, the
  swallowed release click).

## Cause and fix

**PS-1 (X58): no clone change; the agent driver's first `prefer` was the cause.** Reproduced on the feature tip's build
with the GitHub lane (99 open pull requests) paired through Settings › Connections: after `tap pull-requests-scroll
wheel 0 1200` the list's scroll node (`layout pull-requests-search` → `node.scroll`) stood at `sy 1200`; after `prefer
has-focus false` / `prefer has-focus true` the search field and the scroll were new nodes (ids 2595 → 5708, 2589 → 5702)
at `sy 0`. The runner's trace shows why, at that `prefer`: `preferences (1 asked again) → epoch 33 (+3131 −3131 ~7)`,
three thousand nodes replaced, not a list read. `state` before and after: `device.prefersColorScheme` and the
`scheme` derive went `dark` → `light`. This Mac is set to Dark (`AppleInterfaceStyle Dark`); an agent session starts
with the runtime told `dark` (`DisplayPreferences.systemDark` reads the user default while `Agent.systemAppearance` is
nil) while its window draws in Aqua, and every `prefer` calls `systemContrast`, which sets `Agent.systemAppearance` from
the window's effective appearance (Aqua) and re-tells the preferences: `light`. The clone mounts the Pull Requests page
once per scheme (`PagesCover`: `when scheme == "dark"`, as its surfaces with `light-dark()` SVG glyphs are), so the page
mounts again and its scroll starts at the top. pr-list-live-refresh's investigation A and attempt 4 were each the
session's first `prefer`; B and C (no `prefer`) kept the place. With one `prefer` made first, refocus after refocus keeps
the place on the same scroll node, the list's read included (two reads of 72 KB each in the trace, `sy 1200` kept). A
read held off by the 10 s rule, the minute tick and Refresh behave the same (Refresh's own top is the agent's `tap`,
which scrolls its target into view first). A real refocus tells only the page facts (`didBecomeKey` →
`Session.tellPage`): the real-input steps below confirm it. One-file repro (this branch's exact2, `exact new`, macOS
agent): see "Framework problem met".

**PS-2 (X13): built in the clone.** The reference (trusted CDP mouse and key events): moving onto Settle swaps the row's
card for the "Settle thread" tooltip; the press closes it; past 6 px the list takes `**:pointer-events-none`, so nothing
is hovered and nothing pops up while the rows arm; Escape cancels (no row armed, the row under the pointer `:hover`);
the release applies nothing; the next pointer move opens that row's card. The clone before: the press's `focus`
(`focus=hoverCard("", false)`) only moved the overlays off (`sidebarHoverY = -10000`), leaving the window's hover on the
pressed row (`sidebarHoverId` stayed `sweep-4~active#settle`), and after a cancelled release the pressed row, not the one
under the pointer, was hovered (the host keeps the pressed row hovered through a pan and hovers nothing new until the
pointer moves). A press on a button that already held the focus runs no `focus`, so from the source it would keep the
card up through the sweep; that was read from the source, not shown: the drive's second gesture meant to press with the
focus already on Settle, but its press still ran the button's `focus` (`sidebarHoverY` -10000 at op 40, before and
after, after op 29 had set it from the row), so the case is left to real-input step 5. Now:

- ThreadSection's first arming calls the root's `sidebarHover(hoverId, false)` (handed down as `unhover`), so the card
  or a control's tooltip closes as the sweep takes over, whatever the focus did.
- A cancel keeps the row under the pointer as its rows' hover (`sweepRest`), through the release, until the window's
  hover count (`sidebarHoverSeq`, bumped by every sidebar hover event) moves on; `fn sweepRowHover` chooses (none while
  live, the resting row, else the window's). No card opens until the pointer moves, as the reference's. The pointer's
  first move, out of the section too, sends the leave of the control the host kept hovered through the pan (its
  `SidebarTip`, `hover=hoverCard(tip)`) through the window's `sidebarHover`, which ends the resting row. The section
  takes no `hover` of its own: the macOS host hovers one node at a time (X62, #322), so under a real pointer a section
  hover and the row under the pointer would trade the hover on every move (review round 1 removed the
  `hover=sectionHover` the first version had).
- Escape stays r12's window shortcut (`sweep-cancel-<section>`, only while live): the reference hears it from anywhere
  in the document, which a `key` handler on the section would not; the cancelled release sends `|cancel|` and the client
  applies nothing (r12-sidebar.test.ts).

## Framework problem met

- **The agent's first `prefer` flips the reported system appearance (X58, filing for the coordinator).** One-file app
  (`bun scripts/exact.mjs new`, `bun exact.mjs mac` on this branch's exact2), on a Mac set to Dark:

  ```text
  shape Viewport
    width: number
    height: number
    prefersColorScheme: string
    prefersReducedMotion: bool
  shape Page
    hasFocus: bool
    visibilityState: string
  shape Row
    n: number
  shape Rows
    items: list<Row>

  component X58app
    resource viewport = exactViewport() as shape Viewport
    resource page = exactPage() as shape Page
    resource rows = rows() as shape Rows
    view
      main testId="root" width="100%" height="100%" display="flex" flex-direction="column"
        text `scheme ${viewport.prefersColorScheme}, focus ${page.hasFocus}` testId="facts"
        when viewport.prefersColorScheme == "dark"
          scroll testId="list" flex=1 min-height=0
            each r in rows.items key=r.n
              text `dark row ${r.n}` testId=`row-${r.n}` height=24
        else
          scroll testId="list" flex=1 min-height=0
            each r in rows.items key=r.n
              text `light row ${r.n}` testId=`row-${r.n}` height=24
  ```

  (`app.ts`: `rows: () => ({ items: Array.from({ length: 100 }, (_, n) => ({ n })) })`.) Drive: `bun exact.mjs agent
  macos --size 600x500 "clock +2000 real" "layout row-10" "tap list wheel 0 600" "layout row-10" "prefer has-focus false"
  "prefer has-focus true" "layout row-10" "tap list wheel 0 600" "prefer has-focus false" "prefer has-focus true"
  "layout row-10"`. Result: the window draws Aqua while the text reads "scheme dark, focus true" (dark rows); after the
  wheel the list is at `sy 600`; after the first `prefer` (which names only `has-focus`) the trace says `preferences (1
  asked again) → epoch 3 (+101 −101 ~2)`, the list is a new node (3 → 104) at `sy 0` and reads "light row …"; the second
  pair of `prefer`s keeps `sy 600`. Expected: a `prefer` that does not name `prefers-color-scheme` leaves it as it is
  ("A fact not named stays as it is", `Agent.prefer`), and an agent session starts with the scheme its window draws.
  Code: `DisplayPreferences.systemDark` (user default while `Agent.systemAppearance` is nil) versus
  `AgentMac.systemContrast` (called by every `prefer`, it sets `Agent.systemAppearance` from the window's effective
  appearance). Clone workaround: none needed (a drive that measures a scroll across `prefer` makes one `prefer` first).
- **A child component's action cannot call an action prop whose value is a parent component's action** (found while
  building PS-2): ThreadSection calling `hoverCard(…)` (the window's `sidebarHover`) failed `contract build` with
  `[type-cannot-infer] cannot infer the type of parameter @capture:108:a11:0`; the hidden captures the inliner gives a
  component's actions (`contract/syntax/src/inline.rs`, `@capture:{n}:a{pi}:{j}`, `ty: None`) are untyped there. A root
  action handed down as it is works (the fix calls the root's `sidebarHover`). Not a parity gap; the message names an
  internal parameter. For the coordinator to judge.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| PS-1 | pass, no clone change: reproduced on the feature tip and traced to the agent's first `prefer` (the system appearance re-told, the per-scheme page mounted again); after it, a refocus with the list's read due (a 72 KB `pullRequests.list` reply in the trace) keeps `sy 1200` on the same scroll node, before and after alike; one-file repro for the coordinator (EXACT2-GAPS X58) | [record](https://raw.githubusercontent.com/ccheever/exact2/d9bf0946469c048d1cb0077224c9f6c7fe84c103/pr-list-scroll-and-sidebar-sweep/ps-record.txt), [one-file repro](https://raw.githubusercontent.com/ccheever/exact2/596c32887cd45e0e40fadd36d7c7175d25323a52/pr-list-scroll-and-sidebar-sweep/x58-one-file-repro.png) |
| PS-1 under a real refocus | open: real-input step 1 | — |
| PS-2a | pass in state; the images are the same before and after: with the sweep armed the window's hover is cleared (`sidebarHoverId` `""` at every `state`; before, it stayed on the pressed row's Settle, `sweep-4~active#settle`), and no card or tooltip shows during the sweep, as the reference. In these drives the press itself already moved the card off through the buttons' `focus`, before and after, so nothing visible differs. The case the arming's close is for, a press on a button that already holds the focus (no `focus` runs), was read from source, not shown (the second gesture's press still ran `focus`: `sidebarHoverY` -10000 at op 40, before and after): real-input step 5 | [image](https://raw.githubusercontent.com/ccheever/exact2/5afb0daf34797027922a51d4918141a11c4f071f/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape-r2.png) (row 2), [record](https://raw.githubusercontent.com/ccheever/exact2/35b8cf242c10be640f617741e0914d2ff147e9b9/pr-list-scroll-and-sidebar-sweep/ps-record-r2.txt) |
| PS-2b | pass (unchanged from r12, before and after): Escape cancels, no row stays armed, the release settles nothing (`settled 0`, `sweepEpoch` +1) | [image](https://raw.githubusercontent.com/ccheever/exact2/5afb0daf34797027922a51d4918141a11c4f071f/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape-r2.png) (rows 3, 4) |
| PS-2c | pass: after the cancelled release Sweep three (under the pointer) is the hovered row and no card opens until the pointer moves, as the reference, on the first gesture and on a second one from the same Settle; before, Sweep four (the pressed row) was | [image](https://raw.githubusercontent.com/ccheever/exact2/5afb0daf34797027922a51d4918141a11c4f071f/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape-r2.png) (rows 4, 5), [second gesture](https://raw.githubusercontent.com/ccheever/exact2/2dbe54b0e72c4b8e6dd7bb367e08be510f06a0e4/pr-list-scroll-and-sidebar-sweep/ps2-second-sweep-r2.png) (rows 3, 4) |
| PS-2: a released sweep still applies | pass: Settled, "Settled 2 threads, ⌘Z to undo" (Sweep two and one), as the reference; before, 3 (Timeline verification armed too: the feature tip `6db5cadea` predates SW-1, merged here) | [image](https://raw.githubusercontent.com/ccheever/exact2/108acc526dfcda3b6ce26e3b9069985abc549170/pr-list-scroll-and-sidebar-sweep/ps2-sweep-release-r2.png) |
| PS-2 under a real trackpad drag and real Escape | open: real-input steps 2-5 | — |

Review round 1 (2026-10-12): the PS-2 part of the after drive again on `d33c70ac1` (the section's hover removed, merged
with the feature tip `8fa9446cc`) in a fresh lane seeded the same way ([script](https://raw.githubusercontent.com/ccheever/exact2/bbbbfa7bab0ce9603940412dfc0eb939c342f432/pr-list-scroll-and-sidebar-sweep/drive-final-r2.sh.txt), [record](https://raw.githubusercontent.com/ccheever/exact2/35b8cf242c10be640f617741e0914d2ff147e9b9/pr-list-scroll-and-sidebar-sweep/ps-record-r2.txt)): the window's
hover is the same at every `state` as the first round's after drive, the first gesture's images differ only in the
relative times, and the second and third gestures now arm two rows, as the reference, where the first round armed three
(SW-1). The before and reference columns are the first round's.

Drives (agent mode, 1280×840, `final.sh`: [script](https://raw.githubusercontent.com/ccheever/exact2/8f690e40c018396b5a5292b14ffa379e0a48b50a/pr-list-scroll-and-sidebar-sweep/drive-final.sh.txt)): before on the evidence worktree at `6db5cadea`, after on this branch's bundle (merged with the feature tip `fe0e39c91`); the lanes' T3 homes hold five threads "Sweep one" … "Sweep five" ([seed](https://raw.githubusercontent.com/ccheever/exact2/349d06600a8863549f5adeee1005c55e911d4273/pr-list-scroll-and-sidebar-sweep/sweep-threads.py.txt), projection rows only, as the audit's `extras.py`); the GitHub lane's primary server (16523) paired through Settings › Connections. The reference: `ref-app.sh` on 16520 with the same five threads, driven by trusted CDP mouse and key events ([script](https://raw.githubusercontent.com/ccheever/exact2/9ce6af018582552524d5e96b071a75596e6c77e9/pr-list-scroll-and-sidebar-sweep/ref-final.mjs.txt); screenshots through `Page.captureScreenshot`: Playwright's own resizes the page, and a `resize` cancels the reference's sweep).

## Seen while building, not a finding row

- In the first round's agent drives the clone's sweep armed one row more than the reference on the second and third
  gestures of a session (Sweep two dragged 90 pt armed Sweep two, Sweep one and Timeline verification; the reference
  armed two). That was the read-after-assign bug adopt-main-fixes-r10's SW-1 fixed on the feature branch (`ade46167f`,
  #472): merged here in review round 1, the after drive arms two rows and settles 2 threads, as the reference. Nothing
  left to track.
- The agent's `tap settle-… hover` lights Settle but draws no "Settle thread" tooltip in these drives (the window's hover
  moves to `…#settle`, `sidebarHoverH` 0); the reference shows it. Not driven further (one live drive).

## For the coordinator's known-differences list

- **The Pull Requests list (and most surfaces) goes back to its top on a real appearance change.** The clone mounts the
  page once per scheme (`PagesCover`: `when scheme == "dark"`, as its surfaces whose SVG glyphs use `light-dark()`), so
  a real Light/Dark switch while reading the list mounts it again at its top; the reference keeps its place. The clone's
  own structure, the same remount PS-1 traced; not a framework gap and not this task's rows (EXACT2-GAPS X58 notes it).

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) whose T3 home has
the five sweep threads (`python3 -I sweep-threads.py <home>/userdata/statev2.sqlite` before the first launch), with
the GitHub lane paired. Never send a message.

1. **PS-1.** Pull Requests; scroll the list down with the trackpad about twenty rows. Click another app's window
   (Finder), wait 15 s, click the clone's title bar. Expected (reference): the same rows stay in view (a read may run;
   no jump to the top). Then ⌘Tab away and back: the same.
2. **PS-2a.** Hover Sweep four (its card opens), move onto its Settle (the card gives way to "Settle thread"), press and
   hold, drag down slowly onto Sweep three. Expected: the tooltip is gone from the press on, no card or tooltip while
   Sweep four and Sweep three show their Settle badges.
3. **PS-2b.** Still holding, press Escape: the badges go and Sweep three shows its hover actions; release: nothing is
   settled (no toast, Settled count unchanged).
4. **PS-2c.** Without moving: Sweep three stays the hovered row, no card. Move the pointer a little: Sweep three's card
   opens. Repeat 2-3, release, and move the pointer straight out of the sidebar (onto the chat): Sweep three's hover
   actions go. Move back over Sweep three and around inside it: its card stays open, no flicker.
5. Repeat 2-4 at once from the same Settle (a press on a button that already holds the focus moves no focus, the
   case the arming's own close covers): the same.
6. A sweep from Sweep two over Sweep one, released: "Settled 2 threads, ⌘Z to undo"; ⌘Z brings them back.

## Tests

- `pr-list-scroll-and-sidebar-sweep.test.ts` (5): `sweepRowHover` evaluated (none while live; the cancelled sweep's
  resting row while the window's hover count holds; the window's hover after it moves on); the rows take it, the cancel
  records the row under the pointer and the count, a new sweep forgets it, and the section's column takes no `hover` of
  its own (review round 1); the first arming (`armed == ""`) calls the root's `sidebarHover(hoverId, false)` handed down
  as `unhover` to all five sections, and the window's half counts hover events; Escape's shortcut only while live and the
  `|cancel|` release; PS-1's guard: the Pull Requests list mounts on the page and the scheme only, never on the window's
  focus or visibility, its rows keyed. The three PS-2 tests fail on the base (`6db5cadea`'s contracts), the two others
  pass there.
- `sidebar-sweep.test.ts` (SW-1's, merged in review round 1): a fourth test checks each sweep's first arming closes the
  window's hover once, a fresh sweep after a release included, and a cancelled sweep arms nothing more (it fails if the
  gate reads the slot `sweepKeys`, which an action sees from before it, instead of the move's `armed`). Since review
  round 2 it runs on #482's shared interpreter, `contract-action-fixture.ts` (`threadSection.call`, with `hoverId`,
  `hoverSeq` and `unhover` in its scope), whose `run` now executes a call of a function in scope (an action prop such as
  `unhover`); before, it threw `unsupported statement`. The other users of the fixture (`queued-drag.test.ts`) are
  unchanged.

## Review round 1 (2026-10-12)

| Finding | Severity | Result |
| --- | --- | --- |
| ThreadSection's `hover=sectionHover` is the first hover on an ancestor of the rows; the macOS host hovers one node at a time (X62, #322, fix #327 not adopted), so under a real pointer the section and the row would trade the hover each move, and it is redundant | blocking | fixed (`accfa868d`): the handler and `sectionHover` removed; the first move after a cancelled pan sends the leave of the control the host kept hovered (its `SidebarTip`) through the window's `sidebarHover`, which bumps `sidebarHoverSeq` and ends the resting row; the test pins that the section's column takes no `hover`; real-input step 4 now moves the pointer straight out of the sidebar and around inside a row |
| PS-2a's before-bug ("a press on a button that already held the focus kept the card") was never driven: the second gesture's press still ran `focus` | should-fix | fixed in the record, EXACT2-GAPS X13 and the PR: read from source, not shown; left to real-input step 5; PS-2a's measured difference is state only |
| The comment above `sectionHover` described the arming's `unhover` | should-fix | fixed: removed with the handler; the resting-hover comment says what ends it and why the section takes no hover |
| Two differences found here had no follow-up: the per-scheme remount on a real appearance change; one row more armed on later gestures | should-fix | the remount: "For the coordinator's known-differences list" above, EXACT2-GAPS X58 and the PR; the extra row: SW-1's fix (#472) merged here, the after drive now arms two rows and settles 2, as the reference |

## Review round 2 (2026-10-12)

| Finding | Severity | Result |
| --- | --- | --- |
| The PR no longer merged: the base moved to `ca4f28e35` (#482, followups-1012), which moved `sidebar-sweep.test.ts`'s ThreadSection interpreter into `contract-action-fixture.ts` without this PR's action-prop call branch; with the base's fixture all three SW-1 tests fail with `unsupported statement: unhover(hoverId, false)` against this PR's `sidebar.contract` | blocking | fixed (`d9770b097`, the merge): the call branch moved into the fixture's `run` and its header names the statement; the hover test runs on `threadSection.call`. The merged files on the base's fixture: 8 pass / 4 fail over `sidebar-sweep`, `queued-drag` and `pr-list-scroll-and-sidebar-sweep` (the three SW-1 tests and the hover test, each `unsupported statement: unhover(hoverId, false)`); with the branch, 12 pass |

## Checks

Review round 2, on the merge `d9770b097` (this branch merged with the feature tip `ca4f28e35`), each run once:

| Check | Exit | Note |
| --- | --- | --- |
| `sidebar-sweep.test.ts`, `queued-drag.test.ts`, `pr-list-scroll-and-sidebar-sweep.test.ts` | 1 then 0 | 8 pass / 4 fail on the base's fixture, 12 pass with the call branch |
| `bun test examples/t3-code --timeout 60000` | 0 | 4715 pass, 1 skip, 0 fail (319 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,225,369 bytes; `app.contract` 1,418 lines; the warnings are the feature branch's |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | this round changes a Bun test fixture only; no Rust or Swift |
| the five checks of CLAUDE.md | not run again | since round 1's run on `2acdadc0f` the base moved by #482 alone, which changes only `examples/t3-code` (no framework, no Rust); `contract build` above compiles the app |
| bundle and live drive | not run again | this PR's contracts are as in round 1's after drive; the merge adds #482's `composer-controls` and `pages-pr-timeline` changes, which #482 drove |

Review round 1, on `2acdadc0f` (`d33c70ac1`'s code: this branch merged with the feature tip `8fa9446cc`), each run once:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4711 pass, 1 skip, 0 fail (318 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,225,277 bytes; `app.contract` 1,418 lines; no warning on this change's lines (the `lower-colour-one-scheme` and other `analyze-read-after-assign` warnings are the feature branch's) |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust or Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | on `d33c70ac1` (the merge; framework changed), before the one after drive |

During review round 1: the touched Bun tests (`pr-list-scroll-and-sidebar-sweep.test.ts`, `sidebar-sweep.test.ts`, `r12-sidebar.test.ts`, `r11-upstream.test.ts`: 36 pass) and `contract build` after the edit and after the merge.

First round, on `2207574b3` (merged with `fe0e39c91`): the same checks, all 0 except `cargo test` 101 (3,744 passed, 1 failed: `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on main's `apps/color-gallery/app.contract:269`, main #440, inherited with round 9; gone with this round's merge). During development: the touched Bun tests and `contract build` after each contract edit (three builds refused while finding how ThreadSection may call the hover: "Framework problem met").

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, PS-1 | PLS-1 (B12) | Fail (clone): every refocus (a click back or ⌘Tab back) empties the Pull Requests list for a moment and brings it back at its top (reference: its rows stay in view) | [PLS-1-refocus-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/PLS-1-refocus-pair.png) |
| 2, PS-2a | SW-3 (B5) | Fail: the "Settle thread" tooltip stays after the press (reference: gone); at the release the clone opens the hovered row's card at once (reference: no card at 0.5 s) | [SW-3-pressed-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-3-pressed-pair.png) |
| 3–5, PS-2b, PS-2c and the press from a focused Settle | SW-2 (B5) | Fail: moving inside a row, its hover card closes and the row loses its hover while the pointer is still in it (reference: the row stays hovered with its card); PS-2c's "no card at the release" matches the reference (its FAIL withdrawn); step 5's focus half not reachable (Keyboard navigation off) | [SW-2-inside-end-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-2-inside-end-pair.png) |
| 6 | SW-1 (B5) | Pass in the clone; not verified against the reference, whose real-input sweep armed the rows and settled nothing on release | [SW-1-mid-drag-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-1-mid-drag-pair.png) |

Not passed: PLS-1, SW-2 and SW-3. PLS-1 is not X58 (real input sends no `prefer`); its cause is not traced.

## Delivery

Merged on 2026-10-12 as `cf16fe13d` (#468, squash) into `feat(example)/t3-code`. Merged from `origin/feat(example)/t3-code` (`fe0e39c91`, main adoption round 9 and #462) before the first round's checks, no conflicts; in review round 1 from `8fa9446cc` (main adoption round 10 and SW-1, #472, and later task PRs): `sidebar.contract`'s sweep conflicted with SW-1 and was resolved keeping both (SW-1's `let` bindings; this task's resting hover, and the first arming's `unhover` gated on the move's `armed`). In review round 2 from `ca4f28e35` (#482): `sidebar-sweep.test.ts` conflicted with #482's move of its interpreter into `contract-action-fixture.ts` and was resolved on the base's fixture, with this task's action-prop call branch ported into the fixture.

## Next action

None. Closed after `realinput-1011c`; PLS-1, SW-2 and SW-3 are in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
