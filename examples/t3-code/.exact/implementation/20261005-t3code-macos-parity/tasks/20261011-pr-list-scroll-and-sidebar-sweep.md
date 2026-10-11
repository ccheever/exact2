---
name: 20261011-pr-list-scroll-and-sidebar-sweep
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-open-real-input-rows
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-list-scroll-and-sidebar-sweep
pr_url: null
verified_commit: null
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
pressed row, so a press on a button that already held the focus (any second sweep from the same button) kept the card
up through the sweep, and after a cancelled release the pressed row, not the one under the pointer, was hovered (the
host keeps the pressed row hovered through a pan and hovers nothing new until the pointer moves). Now:

- ThreadSection's first arming calls the root's `sidebarHover(hoverId, false)` (handed down as `unhover`), so the card
  or a control's tooltip closes as the sweep takes over, whatever the focus did.
- A cancel keeps the row under the pointer as its rows' hover (`sweepRest`), through the release, until the window's
  hover count (`sidebarHoverSeq`, bumped by every sidebar hover event) moves on or the pointer leaves the section
  (`hover=sectionHover`); `fn sweepRowHover` chooses (none while live, the resting row, else the window's). No card
  opens until the pointer moves, as the reference's.
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
| PS-2a | pass: with the sweep armed the window's hover is cleared (`sidebarHoverId` `""`; before, it stayed on the pressed row's Settle, `sweep-4~active#settle`), no card or tooltip during the sweep, as the reference; the press already moved them off through the buttons' `focus` (before and after) | [image](https://raw.githubusercontent.com/ccheever/exact2/17d1b73dee10743b6426ff73717c3d2a4b8f5376/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape.png) (row 2), [record](https://raw.githubusercontent.com/ccheever/exact2/d9bf0946469c048d1cb0077224c9f6c7fe84c103/pr-list-scroll-and-sidebar-sweep/ps-record.txt) |
| PS-2b | pass (unchanged from r12, before and after): Escape cancels, no row stays armed, the release settles nothing (`settled 0`, `sweepEpoch` +1) | [image](https://raw.githubusercontent.com/ccheever/exact2/17d1b73dee10743b6426ff73717c3d2a4b8f5376/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape.png) (rows 3, 4) |
| PS-2c | pass: after the cancelled release Sweep three (under the pointer) is the hovered row and no card opens until the pointer moves, as the reference; before, Sweep four (the pressed row) was | [image](https://raw.githubusercontent.com/ccheever/exact2/17d1b73dee10743b6426ff73717c3d2a4b8f5376/pr-list-scroll-and-sidebar-sweep/ps2-sweep-escape.png) (rows 4, 5) |
| PS-2: a released sweep still applies | pass (before and after): Settled, "Settled 3 threads, ⌘Z to undo" | [image](https://raw.githubusercontent.com/ccheever/exact2/251ba010078ef802fc339a00dc3283b58f353c17/pr-list-scroll-and-sidebar-sweep/ps2-sweep-release.png) |
| PS-2 under a real trackpad drag and real Escape | open: real-input steps 2-5 | — |

Drives (agent mode, 1280×840, `final.sh`: [script](https://raw.githubusercontent.com/ccheever/exact2/8f690e40c018396b5a5292b14ffa379e0a48b50a/pr-list-scroll-and-sidebar-sweep/drive-final.sh.txt)): before on the evidence worktree at `6db5cadea`, after on this branch's bundle (merged with the feature tip `fe0e39c91`); the lanes' T3 homes hold five threads "Sweep one" … "Sweep five" ([seed](https://raw.githubusercontent.com/ccheever/exact2/349d06600a8863549f5adeee1005c55e911d4273/pr-list-scroll-and-sidebar-sweep/sweep-threads.py.txt), projection rows only, as the audit's `extras.py`); the GitHub lane's primary server (16523) paired through Settings › Connections. The reference: `ref-app.sh` on 16520 with the same five threads, driven by trusted CDP mouse and key events ([script](https://raw.githubusercontent.com/ccheever/exact2/9ce6af018582552524d5e96b071a75596e6c77e9/pr-list-scroll-and-sidebar-sweep/ref-final.mjs.txt); screenshots through `Page.captureScreenshot`: Playwright's own resizes the page, and a `resize` cancels the reference's sweep).

## Seen while building, not a finding row

- In the agent drives the clone's sweep armed one row more than the reference on the second and third gestures of a
  session (Sweep two dragged 90 pt armed Sweep two, Sweep one and Timeline verification; the reference armed two); the
  first gesture matched (Sweep four and three). Before and after alike; the arming geometry is r11's, unchanged here.
- The agent's `tap settle-… hover` lights Settle but draws no "Settle thread" tooltip in these drives (the window's hover
  moves to `…#settle`, `sidebarHoverH` 0); the reference shows it. Not driven further (one live drive).
- The clone mounts its Pull Requests page (and most surfaces) once per scheme, so a real appearance change while
  reading the list also puts it back at the top, where the reference keeps its place.

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
   opens.
5. Repeat 2-4 at once from the same Settle (a press on a button that already holds the focus moves no focus, the
   case the arming's own close covers): the same.
6. A sweep from Sweep two over Sweep one, released: "Settled 2 threads, ⌘Z to undo"; ⌘Z brings them back.

## Tests

- `pr-list-scroll-and-sidebar-sweep.test.ts` (5): `sweepRowHover` evaluated (none while live; the cancelled sweep's
  resting row while the window's hover count holds; the window's hover after it moves on); the rows take it and the
  cancel records the row under the pointer and the count; the first arming calls the root's `sidebarHover(hoverId,
  false)` handed down as `unhover` to all five sections, and the window's half counts hover events; Escape's shortcut
  only while live and the `|cancel|` release; PS-1's guard: the Pull Requests list mounts on the page and the scheme
  only, never on the window's focus or visibility, its rows keyed. The three PS-2 tests fail on the base (`6db5cadea`'s
  contracts), the two others pass there.
