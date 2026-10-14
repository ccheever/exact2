# First-wave continuation, 2026-10-14

The stopped work resumed from feature `609f392d52a5c3f65cda2f50e5e5824a01a4184e` in isolated task branches. The original detached checkout is unchanged. All eleven repair PRs are now merged into the remote feature branch at checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a` (GitHub readback, 03:04 UTC). The bounded main adoption has passed repository and native verification; PR [#549](https://github.com/ccheever/exact2/pull/549) is confirmed merged at feature `3710af842bc54521c298124156989950b6f2e68c` (03:08:53 UTC), preserving main's ancestry. The broader real-input qualifications remain open.

## Published revisions

| Task | Merged PR | Published revision | Remaining limit |
| --- | --- | --- | --- |
| [terminal-links-and-close](../tasks/20261014-terminal-links-and-close.md) | [#531](https://github.com/ccheever/exact2/pull/531) | `d3f075a20892c469a15549fb5f78cbdbf0dfebf3` | Actual background onboarding environment test; original system-browser discrepancy remains unreproduced. |
| [terminal-input-and-ime](../tasks/20261014-terminal-input-and-ime.md) | [#533](https://github.com/ccheever/exact2/pull/533) | `ce8cc79bb16f0caa3d7372a7818ffa51b6f031f9` | IME consumed/declined Control handling; real middle press and live IME remain open. |
| [composer-chips-accessibility](../tasks/20261014-composer-chips-accessibility.md) | [#534](https://github.com/ccheever/exact2/pull/534) | `fd8c6541ef841649868e3559cf150faf37525bee` | Projection, ranges and labels reviewed; native AX proof passed; VoiceOver and terminal-send remain open. |
| [device-player-gestures](../tasks/20261014-device-player-gestures.md) | [#539](https://github.com/ccheever/exact2/pull/539) | `0cd3821cc21c07adf626bf06dfdabaff6bf35686` | Removed vacuous arithmetic check and preserved authentic resize/pill proof; real pointer/cursors remain open. |
| [cite-selection-anchor](../tasks/20261014-cite-selection-anchor.md) | [#540](https://github.com/ccheever/exact2/pull/540) | `706f3d3f44a718d19f90996e82258718ec6e3e49` | Pending/stale cancellation verified by tests and native drive; final real input remains open. |
| [chat-markdown-media-and-code](../tasks/20261014-chat-markdown-media-and-code.md) | [#541](https://github.com/ccheever/exact2/pull/541) | `8b61e102c317da4be58a01cdc087860031f961b8` | Repeated occurrence and hover lifecycle passed natively; capture reduction removes the plan-size tradeoff; real pointer remains open. |
| [native-helpers-and-text-menu](../tasks/20261014-native-helpers-and-text-menu.md) | [#544](https://github.com/ccheever/exact2/pull/544) | `4f81dc73ffb3984524ada9f8542ab3a67254e6ea` | Native Services and tracking timer proof passed; original real-grant/Finder observations remain unresolved. |
| [sidebar-hover-archive-and-glyph](../tasks/20261014-sidebar-hover-archive-and-glyph.md) | [#545](https://github.com/ccheever/exact2/pull/545) | `a6fa91916dad1a27b34fc9ad42f5a91c383e81a8` | Connected native hover/glyph drive passed; Archive dialog and real input remain open. |
| [pr-list-place-and-thread-picker](../tasks/20261014-pr-list-place-and-thread-picker.md) | [#546](https://github.com/ccheever/exact2/pull/546) | `6ff18f2251305c8f3f30a6ace8e425eab001367f` | Cancellation recovery and disabled-row Enter reviewed; connected scroll/picker real input remains open. |
| [branch-notice-and-navigation](../tasks/20261014-branch-notice-and-navigation.md) | [#547](https://github.com/ccheever/exact2/pull/547) | `4105091dbcdff5146dd431e9f0789658605d8cf6` | Actual Git, dirty Cancel/Confirm, Settings toast and draft relaunch passed; transparent screenshots provide no visual proof. |
| [Appearance fonts](../tasks/20261007-installed-font-picker.md) and [theme inspection](../tasks/20261009-blocked-theme-usage-highlight.md) | [#548](https://github.com/ccheever/exact2/pull/548) | `31d46d0a21e1de85626ce04a0d81068fcd8cc6cd` | Native fonts 28/28 and Inspector 33/33 passed; variants, full persistence/keyboard matrix and VoiceOver remain open. |

## Review and integration

All listed repair implementations received source review. The composer repair additionally has independent review bound to its seven-file fingerprint `c157e0b26c513dcb9c371c667184cdc6573a2bfa0ebd15409ec1ece920403159`; the PR-list repair to 42 runtime/test files, `74f8c9eb6b8f883dbfec26d475cba358b56e5318092802dfbae4cf8fa691358b`. Their task records distinguish AppKit/agent checks from real input.

The local first-wave checkpoint `1e3152209` combines all eleven published branches, including Appearance runtime `3edaae268` and its final receipt head `31d46d0a2`. Markdown/Cite's conflicting import keeps the new morph and current Cite helpers. The PR-list merge keeps the sidebar's press forwarding in a single appearance subtree and keeps Markdown's media-focus installation while removing the retired R10 selection hook.

The branch-navigation merge `123d31131` preserves that same single-subtree behavior for ToastStack and adds the new confirmation label. Root comments were shortened to keep the source within the 1,500-line cap; caps and staged diff checks pass.

At integration `6ecc1e0cf`, the full app suite had 4,959 passes, one skip and two fixture failures: the sidebar action fixture omitted the Cite state now read by shared outside-press handling. `fb681d6e3` initializes that state; all 70 related sidebar/Cite/Usage tests pass afterward. Strict TypeScript passed on the integration. Caps and staged diff checks passed. These checks are not the final combined verification.

The hover controller now belongs to the root alongside its clock (`95ae39500`), avoiding 134 unrelated window-property captures per forwarded handler. Its isolated plan fell from 37,196,589 to 33,177,857 bytes, below the original 33,309,129-byte feature baseline. UI node and binding counts stayed unchanged. The fresh native fixture passed timing, repeated-image navigation, focus return, hover-away and Settings dismissal. PR #541 was updated and read back at the new head above.

The combined source at `7baeb1229` compiled at 27,894,502 bytes; strict application TypeScript passed. Its app root is 1,498 lines. The first combined Bun run had 4,966 passes, one skip and 12 failures, from invocation-order/markup expectations and fixtures. These were corrected without weakening their expectations. Appearance's native Inspector failure was repaired by sharing Escape ownership with Settings. At `128463953`, all 4,978 app tests passed, with one skip.

Appearance's independent review is bound to runtime `3edaae268` and documentation head `31d46d0a2`. It checked 576 outside-click state/effect comparisons, 33/33 native Inspector checks and 28/28 native font checks. The actual composer uses the selected Georgia family and the expanded local diff carries Menlo to code/gutters. The [durable packet](https://github.com/ccheever/exact2/blob/ffe98932fe5943814d09069cbb210d2f91c5d01d/appearance-theme-tools/20261014-resume/README.md) contains source/binary identities and authentic final screenshots. Its transparent baseline image is rejected; baseline tree/AX observations are separate from the final connected fixture. Both broader task records remain unverified for their outstanding matrix.

Main adoption is tracked in [round 14](../tasks/closed/20261014-adopt-main-fixes-r14.md). The migrated app passes the same 4,978 tests and strict TypeScript. The root source sweep found main's newly refused non-SVG transforms; `30be3a94a` converts four chevrons to individual rotation and removes an ignored monogram transform. The resulting plan compiles to 28,103,415 bytes, 15.6% below the original 33,309,129-byte feature baseline. The affected root test passed its clean rerun across 94 roots and 195 imports. The fresh workspace check and all 27 app Rust tests pass; the native build and connected ten-capture smoke also passed ([durable packet](https://github.com/ccheever/exact2/blob/144fe75ff80651336d1a7511f71dbb54923345d7/adopt-main-fixes-r14/20261014-integration/README.md)).

## Next action

All eleven first-wave PRs and adoption [#549](https://github.com/ccheever/exact2/pull/549) are merged. Required repository checks and the combined native drive passed at their recorded source revisions. The user unlocked the desktop and authorized temporary keep-awake on 2026-10-14; real-input qualifications remain open until individually demonstrated. PR #541's size/functionality choice is no longer needed after the measured reduction.

The first five serial repairs are now delivered:

| Task | Merge | Qualification status |
| --- | --- | --- |
| [Dialog focus](../tasks/20261014-dialog-focus-return-and-traps.md) | [#553](https://github.com/ccheever/exact2/pull/553), `6a7a3cad0cb5f66fb7b5d677d21f8e130356e5d7` | Bounded source delivered; task remains active for its recorded native qualifications. |
| [Legacy sidebar](../tasks/20261014-legacy-sidebar-parity.md) | [#557](https://github.com/ccheever/exact2/pull/557), `4520b0124b36cf24481a973f59a414ae8753dab1` | Native-agent rename and selection passed; physical-input qualifications remain open. |
| [Connections settings](../tasks/20261014-connections-network-settings.md) | [#566](https://github.com/ccheever/exact2/pull/566), `fbed7fadd8673ed58eaa98c585bccaf9f0bc5d16` | Final source, OS keyboard/focus and native-agent pointer passed. Normal relaunch, cross-process routes, physical OS drag, spoken VoiceOver and live tailnet remain unverified. |
| [Menu keys](../tasks/20261014-menu-keys-popover-commands.md) | [#568](https://github.com/ccheever/exact2/pull/568), `b229a2658029a18584b2d5efb4528f4cc2af98ac` | Two native agent drives; three failures of the first drive fixed in repair round 2 and passed on the retry, including physical Tab. Normal-launch NSMenu, G2 Enter under the agent driver and spoken VoiceOver remain unverified. |
| [Browser panel](../tasks/20261014-browser-panel-parity.md) | [#571](https://github.com/ccheever/exact2/pull/571), `70eec59fd8b345df77c5932eeb2a474b40a16e54` | Native agent drive passed G4-18, G4-19 and G4-16; G5-A3 by unit assertion. Physical hover and ⇧⌘J, G5-A3 natively and zoom below 100% remain unverified. |
| [CSS workaround retirements](../tasks/20261014-css-workaround-retirements.md) | [#577](https://github.com/ccheever/exact2/pull/577), `6ef85eb1c` | Native agent drive passed balance, placeholders, filter values, right- and bottom-edge flips and the pending card border. Physical bottom-edge popup, real Discard click, Limits dismissal, unresolved chip and long values at three sites remain unverified. |
| [Timeline and window hooks](../tasks/20261014-timeline-and-window-hooks.md) | [#584](https://github.com/ccheever/exact2/pull/584), `47f3ed1bd` | Native agent drive passed per-thread restore, minimap lit turns and 24-point jump, citation jump and title row with emulated display mode. Real full-screen, real-time motion, trackpad settling, reduced motion and IME remain unverified. |
| [Contract keys and menu bar](../tasks/20261014-contract-keys-and-menu-bar.md) | [#587](https://github.com/ccheever/exact2/pull/587), `aa4fd19c3` | Native agent drive passed launcher shortcuts, both key recorders (one repair), press modifiers and the archive menu. Real menu-bar tracking, US/2-Set chords, physical context menus, held modifiers, PR number menu, Send intent and VoiceOver remain unverified. |

All eight serial follow-ups merged by 2026-10-15 (feature head `aa4fd19c3`). Next: one final real-input session for the unverified rows of the eight tasks. The six held main features and #498 remain outside this authorization. Normal app activation repeatedly selected an older registered bundle even with PID targeting; use the successful isolated standalone path for subsequent native checks and retain the normal-launch limitation. The Connections task records complete restoration of the normal preferences and file roots.

Follow-up order (one task PR at a time): `dialog-focus-return-and-traps`,
`legacy-sidebar-parity`, `connections-network-settings`, `menu-keys-popover-commands`,
`browser-panel-parity`, `css-workaround-retirements`, `timeline-and-window-hooks`, then
`contract-keys-and-menu-bar`.

Evidence policy, 2026-10-14: the `t3-code-evidence` branch was deleted from the repository
(Charlie, #99: heavy evidence stays out of the repository). Evidence logs stay in each task
worktree's ignored `target/`; PR images are GitHub attachments (`gh issue comment --attach`).
