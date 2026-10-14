# First-wave continuation, 2026-10-14

The stopped work resumed from feature `609f392d52a5c3f65cda2f50e5e5824a01a4184e` in isolated task branches. The original detached checkout is unchanged. This report is partial: appearance tools and the Markdown size repair are still finishing, main adoption is pending, and none of these draft PRs has merged into the feature branch in this continuation.

## Published revisions

| Task | Draft PR | Published revision | Remaining limit |
| --- | --- | --- | --- |
| [terminal-links-and-close](../tasks/20261014-terminal-links-and-close.md) | [#531](https://github.com/ccheever/exact2/pull/531) | `d3f075a20892c469a15549fb5f78cbdbf0dfebf3` | Actual background onboarding environment test; original system-browser discrepancy remains unreproduced. |
| [terminal-input-and-ime](../tasks/20261014-terminal-input-and-ime.md) | [#533](https://github.com/ccheever/exact2/pull/533) | `ce8cc79bb16f0caa3d7372a7818ffa51b6f031f9` | IME consumed/declined Control handling; real middle press and live IME remain open. |
| [composer-chips-accessibility](../tasks/20261014-composer-chips-accessibility.md) | [#534](https://github.com/ccheever/exact2/pull/534) | `fd8c6541ef841649868e3559cf150faf37525bee` | Projection, ranges and labels reviewed; native AX proof passed; VoiceOver and terminal-send remain open. |
| [device-player-gestures](../tasks/20261014-device-player-gestures.md) | [#539](https://github.com/ccheever/exact2/pull/539) | `0cd3821cc21c07adf626bf06dfdabaff6bf35686` | Removed vacuous arithmetic check and preserved authentic resize/pill proof; real pointer/cursors remain open. |
| [cite-selection-anchor](../tasks/20261014-cite-selection-anchor.md) | [#540](https://github.com/ccheever/exact2/pull/540) | `706f3d3f44a718d19f90996e82258718ec6e3e49` | Pending/stale cancellation verified by tests and native drive; final real input remains open. |
| [chat-markdown-media-and-code](../tasks/20261014-chat-markdown-media-and-code.md) | [#541](https://github.com/ccheever/exact2/pull/541) | `c8b10717e5e6efe115b15b79bf3f80fedfd625bd` | Repeated occurrence verified natively; plan-size decision and real pointer remain open. |
| [native-helpers-and-text-menu](../tasks/20261014-native-helpers-and-text-menu.md) | [#544](https://github.com/ccheever/exact2/pull/544) | `4f81dc73ffb3984524ada9f8542ab3a67254e6ea` | Native Services and tracking timer proof passed; original real-grant/Finder observations remain unresolved. |
| [sidebar-hover-archive-and-glyph](../tasks/20261014-sidebar-hover-archive-and-glyph.md) | [#545](https://github.com/ccheever/exact2/pull/545) | `a6fa91916dad1a27b34fc9ad42f5a91c383e81a8` | Connected native hover/glyph drive passed; Archive dialog and real input remain open. |
| [pr-list-place-and-thread-picker](../tasks/20261014-pr-list-place-and-thread-picker.md) | [#546](https://github.com/ccheever/exact2/pull/546) | `6ff18f2251305c8f3f30a6ace8e425eab001367f` | Cancellation recovery and disabled-row Enter reviewed; connected scroll/picker real input remains open. |
| [branch-notice-and-navigation](../tasks/20261014-branch-notice-and-navigation.md) | [#547](https://github.com/ccheever/exact2/pull/547) | `4105091dbcdff5146dd431e9f0789658605d8cf6` | Actual Git, dirty Cancel/Confirm, Settings toast and draft relaunch passed; transparent screenshots provide no visual proof. |

## Review and integration

All listed repair implementations received source review. The composer repair additionally has independent review bound to its seven-file fingerprint `c157e0b26c513dcb9c371c667184cdc6573a2bfa0ebd15409ec1ece920403159`; the PR-list repair to 42 runtime/test files, `74f8c9eb6b8f883dbfec26d475cba358b56e5318092802dfbae4cf8fa691358b`. Their task records distinguish AppKit/agent checks from real input.

The local integration branch combines these ten branches. Markdown/Cite's conflicting import keeps the new morph and current Cite helpers. The PR-list merge keeps the sidebar's press forwarding in a single appearance subtree and keeps Markdown's media-focus installation while removing the retired R10 selection hook.

The branch-navigation merge `123d31131` preserves that same single-subtree behavior for ToastStack and adds the new confirmation label. Root comments were shortened to keep the source within the 1,500-line cap; caps and staged diff checks pass.

At integration `6ecc1e0cf`, the full app suite had 4,959 passes, one skip and two fixture failures: the sidebar action fixture omitted the Cite state now read by shared outside-press handling. `fb681d6e3` initializes that state; all 70 related sidebar/Cite/Usage tests pass afterward. Strict TypeScript passed on the integration. Caps and staged diff checks passed. These checks are not the final combined verification.

## Next action

Finish Appearance and measure the bounded Markdown size repair, compile/build and drive the combined app, then adopt current main with its ancestry preserved. Main's `migrate-1116-015.mjs` must run against the app before that merge; update Contract fixtures and source assertions as well. Run the repository gates on the final tree. Keep final real-input rows open while the desktop is locked. PR #541 remains draft pending the compiled-plan-size choice (33.3 MB to 37.2 MB, +11.7%). A behavior-preserving move of the shared hover action is being measured; no reduction is claimed yet.
