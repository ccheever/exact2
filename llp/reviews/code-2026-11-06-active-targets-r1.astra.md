# Computed active paint: review round 1

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `7686941f0c` on `70365218ab`.
- **Method:** one brief (sha256 `4b2f77d3e1728f179c7a56f9552fba16e1a95acb44e6887a0e21efb8864f92e7`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths and its commits name that frozen source.

Two P2 findings remain after the four independent passes.

1. **P2 — Grouped UIKit rows accept active paint that their native cells ignore.** The admission at [states.rs:427](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-gaps-review-iwuti13f/checkout/contract/lower/src/states.rs:427) includes a standard `button` containing text under `list appearance="auto" → section`. Adding `:active background-color="#ff0000"` now compiles, but `Kernel::grouped_row` still classifies it as a standard cell. [GroupedListIOS.swift:1077](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-gaps-review-iwuti13f/checkout/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:1077) removes carried NodeViews and uses `UIListContentConfiguration`; native highlighting never applies the press targets. The row activates, but its pressed background remains UIKit’s highlight. Connect these targets to native cell highlighting. The new row test covers only an ordinary virtualized list’s kernel values, so it misses this case.

2. **P2 — A computed target appearing during a stationary press does not start painting.** [states.rs:156](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-gaps-review-iwuti13f/checkout/contract/lower/src/states.rs:156) now accepts `background-color=(on ? "#ff0000" : none)`. Start pressing with all targets absent, then let a timer set `on=true` while holding: the committed target becomes red, but the press remains unpainted. [input-glue.js:473](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-gaps-review-iwuti13f/checkout/host/web/input-glue.js:473) skips creating both paint state and its style observer when no target exists at press-down; subsequent repaint requests visit existing paint states only. Apple similarly leaves `pressPaint.down` false, and Linux records no `Contact.press`. Refresh feedback when a held node gains its first target. The new clearing test always has other paint targets and dispatches completed actions, so it cannot detect this failure.

These host limitations predate the commit; its broader admission exposes them through newly accepted `:active` programs. I reproduced the second failure using the web driver in a bounded VM with DOM stubs, including a successful subsequent-press control. The native findings follow source paths, without pixel verification.

The patch reuses existing machinery: both web targets use private press properties and `input-glue.js`; ordinary UIKit rows use `PressFeedback`/`PressPaint`; AppKit table tracking drives authored row paint while preserving selection; Linux uses its existing press presenter and painter. No additional material findings emerged in precedence, token routing, explicit-native-button handling, or simplification.

Verification completed:

- Independently reran the four new regression tests: **all passed**.
- The author’s positive log subsequently completed: **21 state-part tests passed**.
- The negative baseline records four expected refusals; its recorded source hashes match the parent.
- Candidate and parent browser logs each report **20 passes and the same touch-timing failure** at `press.test.mjs:541`.

Full gates, semantic difftests, and native UI execution remain unverified. No source changes or Firefox execution. Public-name retirement and border/shadow support were kept outside this review’s scope.