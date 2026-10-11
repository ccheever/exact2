---
name: 20261011-unverified-real-input-rows
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-unverified-real-input-rows
pr_url: PR_URL_PENDING
verified_commit: null
---

# Rows never verified under real input: make the ones that can run ready for one final session

## Outcome

STATUS "Known differences" row 11 (the user asked on 2026-10-11 to fix what can be fixed now). The rows below never ran
under real input. Rows that wait for main PR #327 (X62) stay out: #261's reaction pill tooltip, #307's Usage popover
email tooltip and the press on its padding, #311's PR-link card.

| Id | Row | Why it never ran |
| --- | --- | --- |
| UV-1 | #311, the Code tab's age tip: under a real pointer it did not show on the label while the author tip on the same card did ([image](https://raw.githubusercontent.com/ccheever/exact2/1e242b9575b0549cf907453414401cb7e99b1372/realinput-1009/age-tip-real-pointer.png); "Hermes formats its date; with the reaction tooltip, to investigate") | Not investigated |
| UV-2 | #311, the Code tab's viewed-error tip (a fixture that fails only the viewed read), the short and withheld tips (a file the sandbox does not have), the viewed-here tip (a non-GitHub host) | No fixture |
| UV-3 | #311, the commit link in a comment (onto it, away, a click) | The fixture comment needs a linked SHA |
| UV-4 | #346, a muted tab behind another keeps its muted indicator, and its media plays on when shown again | Fixed in `b6b3e417e`, AppKit-tested, not driven by real input |

Details: [pr-links-previews-and-routing](closed/20261005-pr-links-previews-and-routing.md) "Real-input batch steps" A and B;
[browser-surface-automation](closed/20261005-browser-surface-automation.md) "Real-input batch steps" 4; STATUS "Next
real-input batch".

## Steps

1. UV-1: reproduce in agent mode with the lane's GitHub server (`tools/github-lane`) and read the hover path; compare with
   the author tip on the same card and with the reference (CDP). If the cause is the clone's, fix it; if it is X62 (main
   #327), say so with the evidence and leave it for round 9.
2. UV-2, UV-3: build the fixtures. GitHub writes only in the sandbox `daehyeonmun2021/playground` (a comment with a linked
   SHA, a short or withheld file); the viewed-error tip and a non-GitHub host through the lane's fixture proxy if the clone
   can be pointed at one without code changes, else a test plus the reason the row cannot run live. Keep the fixture
   scripts under `target/` and name them in the record.
3. Drive each row once in agent mode (hover by hit test) and write exact "Real-input batch steps" for UV-1..UV-4 (lane,
   fixture commands, steps, what passes).
4. A PR is needed only if code or tests change; otherwise the PR carries the record.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| UV-1 | cause found; fix with tests, or X62 evidence | before / after / reference images if fixed |
| UV-2..UV-4 | fixtures ready; agent-mode drive; real-input steps written | agent-mode images |

## Findings and what was built

- **UV-1, cause.** The reference has no tip on that age. `PullRequestReviewAnnotation.tsx` ReviewThreadCard draws the
  author as `PullRequestActorLabel` (a tooltip with the login) and the age as a bare
  `<span>{formatRelativeTimeLabel(comment.createdAt)}</span>`; the clone wrapped the age in a `LayerTip` with its full date.
  The reference over CDP (#168 › Code, the open thread off the diff): the pointer on the author shows "daehyeon-mun", on
  the age nothing ([image](https://raw.githubusercontent.com/ccheever/exact2/5215ec1c95da3aa368406219184db504aac46b11/unverified-real-input-rows/uv1-code-tab-age.png)). Agent mode on the feature tip, hover by hit test, did show the
  clone's date tip ("10/8/2026, 4:29:01 PM"), so the miss under a real pointer was not the date format (Hermes formats it)
  nor X62; it is moot once the tip is gone, and the real-input step below checks the reference's behaviour instead.
  The Timeline tab had the same extra tip on all four age kinds (`PullRequestTimelineTab.tsx` draws them as bare spans;
  its only tooltip is the stale verdict's), so they went too. Only the Summary tab's comment age keeps the full date
  (CommentMeta's tooltip). Built: `pages-pr-threads.contract` (the age is plain text with the test id
  `pull-request-thread-age-<comment id>`), `pages-pr-timeline.contract`, and `ageTip` removed from both projections.
- **UV-3, found while driving.** The commit link's tip (its URL, one word) ran on one line past its bubble and the window
  edge; the reference's TooltipPopup is `max-w-80 wrap-anywhere`, which wraps it in three lines. `hover-layer.contract`
  HoverText now has `overflow-wrap="anywhere"` ([image](https://raw.githubusercontent.com/ccheever/exact2/4f6995baf65e4c2688357d8105abfa4dfe7e754e/unverified-real-input-rows/uv3-commit-link-tip.png)).
- **UV-2, UV-4.** No code change: fixtures, an agent-mode drive and the real-input steps below.

## Fixtures (under `target/uv/` in this branch's worktree, `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-unverified-real-input-rows`; copies on t3-code-evidence)

- [`fixture.mjs`](https://raw.githubusercontent.com/ccheever/exact2/b9f10cc91093fc4d1fb7747b6eb2c6ab0121008e/unverified-real-input-rows/fixture.mjs.txt): `start` runs the real-GitHub lane's primary server (`tools/github-lane`, signed-in
  lane gh, port 17163) behind [`proxy.mjs`](https://raw.githubusercontent.com/ccheever/exact2/6c30b00f05701de649eed5f0988d7398b8698c54/unverified-real-input-rows/proxy.mjs.txt) (17164) and writes a single-use `pairing-url` for the
  proxy's origin; `pair` writes a fresh one; `stop` stops both (recorded pids). Its gh is [`gh-fault.sh`](https://raw.githubusercontent.com/ccheever/exact2/35c91469579dd073ed4937cc42c6d0c0ad097a63/unverified-real-input-rows/gh-fault.sh.txt):
  the real gh, except the GraphQL `viewerViewedState` read for #115 fails (HTTP 502), so only #115's viewed read fails
  (viewed-error tip). The proxy forwards everything and rewrites `capabilities.viewedFiles` to `"environment"` in the
  replies for #132 (viewed-here tip: what GitLab, Bitbucket, Azure DevOps and Forgejo answer; the clone is pointed at it
  by pairing, no code change).
- `seed` (GitHub writes in `daehyeonmun2021/playground` only, done on 2026-10-11): pull request **#169** "Archive the weekly
  notes" (`docs/notes-archive`, 511 files: 510 notes and a 40,001-line `data/catalog.csv` whose patch GitHub withholds),
  so the viewed read stops after five pages (viewed-short tip) and the first slice is truncated (withheld tip); issue
  comment 6104938893 on **#132** naming `c46352e19b5d7c31b070eea56750469cdcce9e13` (the commit link). `unseed` closes #169,
  deletes its branch and the comment; run it after the real-input session. Read back by the lane server through the
  proxy: [probe](https://raw.githubusercontent.com/ccheever/exact2/6250068ff931af65f15fc589a0f6d2f141e2ff5b/unverified-real-input-rows/uv2-fixtures-probe.json).
- [`tone.mjs`](https://raw.githubusercontent.com/ccheever/exact2/9b024562d5d2e88df24ac88a81cb92235ebf9800/unverified-real-input-rows/tone.mjs.txt) (17165): `/a` a quiet 90 s tone whose time is the page title, `/b` a quiet page (UV-4).
- [`drive.sh`](https://raw.githubusercontent.com/ccheever/exact2/9963d1053dca463351ea5aff27383e95536c6d57/unverified-real-input-rows/drive.sh.txt) `before|after`: the one agent drive ([ops](https://raw.githubusercontent.com/ccheever/exact2/d23843a776f3d48531a1ef6d8c24c3681a7054cf/unverified-real-input-rows/drive-ops.txt)); `compose.py`, `ages.py`,
  `tabs.py` make the images and texts.

Lane note: the brief's base port 17160 puts the clone's embedded server on 17162, which a development build refuses
(`T3LocalBackend.laneRange` is 16000-16999: "Refusing the real T3 home / port 3773"). The drives therefore pair the GitHub
lane through the welcome wizard, and that server is the focused one; no port outside the lane was used.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| UV-1 Code tab age | pass (agent mode, hover by hit test): before, a full-date tip; after, none; the reference, none. The author tip shows in all three | [age](https://raw.githubusercontent.com/ccheever/exact2/5215ec1c95da3aa368406219184db504aac46b11/unverified-real-input-rows/uv1-code-tab-age.png), [author](https://raw.githubusercontent.com/ccheever/exact2/f65ed8dd39a2b85ac035ff48c33a5e378506c241/unverified-real-input-rows/uv1-code-tab-author.png) |
| UV-1 Timeline ages | pass: #168's Timeline, 17 ages, 17 wrapped by a hover handler before, 0 after | [uv1-timeline-ages.txt](https://raw.githubusercontent.com/ccheever/exact2/803128b3b49ca3ae378292644a173c17c7a384fd/unverified-real-input-rows/uv1-timeline-ages.txt) |
| UV-1 tests | `unverified-real-input-rows.test.ts`: the feature tip's sources 2 pass / 4 fail, this branch 6 / 0 | [tests-before-after.txt](https://raw.githubusercontent.com/ccheever/exact2/75ad577a5a00430a30cf6d494cfe60c49d12d9ea/unverified-real-input-rows/tests-before-after.txt) |
| UV-2 viewed-error, viewed-here, viewed-short, withheld | pass in agent mode (#115, #132, #169, #169): each icon's tip shows with the reference's words; real input open | [image](https://raw.githubusercontent.com/ccheever/exact2/d932411567f423e4f166b5f75c3879c22fb0e24f/unverified-real-input-rows/uv2-code-tab-tips.png) |
| UV-3 commit link | pass in agent mode: onto it, the URL tip wrapped in its bubble (as the reference); away, gone; a press records `…/commit/c46352e19b5d7c31b070eea56750469cdcce9e13`; real input open | [tip](https://raw.githubusercontent.com/ccheever/exact2/4f6995baf65e4c2688357d8105abfa4dfe7e754e/unverified-real-input-rows/uv3-commit-link-tip.png), [away and click](https://raw.githubusercontent.com/ccheever/exact2/a21b40cd38cd938a6152b8cd7c46a7bf1ffa5e5d/unverified-real-input-rows/uv3-commit-link-away-click.png), [uv3-commit-link.txt](https://raw.githubusercontent.com/ccheever/exact2/63f8898a5daad4d1bf332b30e856bb021e95fdb6/unverified-real-input-rows/uv3-commit-link.txt) |
| UV-4 muted tab | pass in agent mode: playing (6.5 s), muted (10.0 s, speaker-off), behind a second tab the chip keeps speaker-off (the media pauses at 11.2 s: WebKit's pause of a muted page out of the window, EXACT2-GAPS Browser surface "Mute"), shown again it plays on (13.9 s, 15.9 s); real input open | [image](https://raw.githubusercontent.com/ccheever/exact2/9b12ffa08fb376d2f62d3743ad4ed6a77e8e334a/unverified-real-input-rows/uv4-muted-tab.png), [uv4-tab-strip.txt](https://raw.githubusercontent.com/ccheever/exact2/3588a6fa633376124bd90181932fc5d902d10663/unverified-real-input-rows/uv4-tab-strip.txt) |

## Tests

- `unverified-real-input-rows.test.ts` (new): the Code tab card's author `LayerTip` stays and its age has none (test id
  `pull-request-thread-age-<id>`); no Timeline age has a `Tip` and `presentTimeline` rows and cards carry no `ageTip`; the
  Summary's comment card keeps its full-date tip; HoverText wraps anywhere within 20rem; a commit link's tip is its URL on
  the hover layer.

## Real-input batch steps

Lane: this branch's bundle (`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle` in the
worktree above), a normal launch of a lane copy as earlier batches did. Setup: `cd target/uv && bun fixture.mjs start`
(GitHub lane server 17163 behind the proxy 17164) and `bun tone.mjs 17165 &`; pair the copy with the URL in
`target/uv/pairing-url` (the welcome wizard's Pairing link, or Settings › Connections › Add environment; `bun fixture.mjs
pair` writes a fresh one). Open Pull Requests and search each number. Cleanup: `bun fixture.mjs stop`, stop tone.mjs, and
after the session `bun fixture.mjs unseed` (closes #169, deletes its branch and the #132 comment).

1. **UV-1.** #168 › Code › "Conversations not on the diff loaded so far" (open it) › the open thread's card. The pointer
   onto "daehyeon-mun": the login tip. Onto the age ("Nd ago"), at its left edge and its centre, rest 2 s: no tip.
   #168 › Timeline: onto any age: no tip. Passes when the author tip shows and no age has a tip (the reference's).
2. **UV-2.** #115 › Code: onto the triangle after "0 / 1 viewed": "The boxes below are whatever was last read, … GitHub CLI
   command failed."; away: gone. #132 › Code: onto the (i) after "viewed in T3 Code": "This host keeps no shared record …";
   away: gone. #169 › Code (wait for the count): onto the first triangle after "0 / 100 viewed": "This change has more
   files than the host will report ticks for …"; onto the second triangle (after the "·"): "The host withheld part of
   this diff — …"; away: gone. Passes when each tip shows on its icon and closes off it.
3. **UV-3.** #132 › Summary › Comments: onto `c46352e`: the URL tip, wrapped inside its bubble; away: gone. A click: the
   default browser opens `https://github.com/daehyeonmun2021/playground/commit/c46352e19b5d7c31b070eea56750469cdcce9e13`
   (close that tab); ⌘-click: the same, the panel does not move. Passes on all three.
4. **UV-4.** A thread's right panel › Browser › `http://127.0.0.1:17165/a`, click the page ("Play the tone"): the chip
   shows the speaker and "Tone N.Ns". Right-click the chip › Mute tab: speaker-off. `+` › Browser ›
   `http://127.0.0.1:17165/b`: the first chip keeps speaker-off. Click the first chip: its title's seconds grow again
   (they stood still while it was hidden, the declared WebKit pause). Passes when the muted icon stays throughout and the
   time advances once shown.

## Not done

- UV-2, UV-3 and UV-4 under real input: the steps above, for the coordinator's final session (no real input here).
- The reason the clone's age tip missed the real pointer was not traced: the tip no longer exists (the reference has
  none), and agent hover by hit test showed it, so no framework gap is claimed.

## Next action

None for this task: the coordinator checks conflicts, moves the PR to ready, and runs the real-input batch steps above
in the final session.
