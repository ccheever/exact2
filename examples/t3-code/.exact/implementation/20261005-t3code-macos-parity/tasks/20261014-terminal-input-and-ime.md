---
name: 20261014-terminal-input-and-ime
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-input-and-ime
pr_url: null
verified_commit: null
---

# The terminal's input method, middle press and drawer motion

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences and the workarounds
round 12 kept; the coordinator's triage (fix wave, clone task `terminal-input-and-ime`) gave this task four rows. The
user decided on 2026-10-11 that each is built now, matching the reference T3 Code.

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| G1-1 | Under 2-Set Korean (`realinput-1011c` B1, [G1-1b](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-1b-candidates-pair.png)) ⌥Return with "한" composing opens no Hanja candidate window: 한 is committed and ⌥Return reaches the shell as a line break; at the session's 60 ms key cadence each jamo went out on its own (ㅎ, ㅏ) | ⌥Return opens the Hanja candidate window at the cursor cell and the choice goes out once (韓); at that cadence the composition's states went out (ㅎ, 하); neither app gave `한글 ` once there |
| G1-2 | ⌃C with ㅎ composing at an empty prompt keeps it composing ([G1-2c](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-2c-pair.png)); the ㅎ leaked into the next paste as `ㅎecho` (G1-4 a) | ⌃C commits ㅎ at the prompt (`$ ㅎ`); no ^C, no new prompt |
| G1-3 | A middle press on an inactive pane neither focuses the terminal nor activates its pane; the focus stays in the composer ([G1-3b](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-3b-pair.png)); the "Close 2 terminals?" confirm of a panel tab lists "Terminal 6", "Terminal 7" in creation order | The press focuses the terminal and activates its pane, pastes nothing; the confirm lists the surface's active terminal first ("Terminal 7", "Terminal 6") |
| KW-8 | The drawer's clipping column says `box-sizing="border-box"` and `interpolate-size="allow-keywords"` (README pitfall #99), round 12 kept them | main `6621ff898` animates a numeric `height` with either box sizing and needs no keyword opt-in: remove both, the drawer opens and closes as before |

## Steps

1. Reference first: `ThreadTerminalDrawer.tsx` (pane `onMouseDown` activates, `TerminalViewport`), the Ghostty surface
   the clone runs unchanged (`surface.ts` `onPointerDown`, composition handlers, `isTerminalCompositionKey`),
   `ChatView.tsx` `closeRightPanelSurface` (the label order), `ChatView.tsx:1270` (the drawer's motion).
2. Before: build the feature tip (`609f392d5`) in this worktree; one agent drive films the drawer opening and closing
   and presses the middle button on an inactive pane.
3. G1-1, G1-2 (`T3TerminalView.swift`, `T3TerminalKeys.swift`, `terminal-host/src/entry.ts`): while the web view holds
   marked text the terminal's native key path (bindings, readline keys, the auth keys) and the page's `beforeKey` leave
   every key but a ⌘ or ⌃ chord to the input method; ⌥Return goes to the input context first; a ⌃ chord commits the
   marked text first and is the composition's, as Chromium's.
4. G1-3: the web view takes the first responder on a middle press (`otherMouseDown`) and focuses the page's input, so
   the pane's focus-in activates it; the surface close copy names the active terminal first.
5. KW-8: remove both attributes; film again.
6. Tests (Bun, AppKit `macos/tests/terminal`), the checks, one agent drive for the after images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G1-1 | entry.ts unit test; AppKit test with marked text through `NSApplication.sendEvent`; real-input steps (only a real input method reaches the candidate window and the cadence) | text before/after (test output) |
| G1-2 | AppKit test: marked ㅎ, then ⌃C: ㅎ goes out once, nothing composing, no ^C; real-input step | text before/after (test output) |
| G1-3 | AppKit test; agent drive (`tap … auxclick` on an inactive pane, then `state`); Bun test of the close copy | before / after / reference image |
| KW-8 | agent drive: the drawer opening and closing filmed (`screenshot … over 600 every 50`) before and after | before / after contact sheets and frame heights |

## Reference

- `apps/web/src/terminal/ghostty/surface.ts` (vendored unchanged as `terminal-host/vendor/ghostty/surface.ts`):
  `onCompositionStart`/`onCompositionEnd`/`onInput`, `isTerminalCompositionKey`, `onPointerDown` (`this.focus()` for
  every button; middle-click paste only where `isMiddleClickPastePlatform`, Linux and BSD).
- `apps/web/src/components/ThreadTerminalDrawer.tsx:1511-1530` (a pane's `onMouseDown` makes it active),
  `:745-782` `handleBeforeKey`.
- `apps/web/src/components/ChatView.tsx:5890-5912` `closeRightPanelSurface`: `[activeLabel, ...otherLabels]`.
- `apps/web/src/components/ChatView.tsx:1270-1276`: the drawer's row animates `grid-template-rows` 0fr→1fr over
  `--panel-animation-duration`, ease-out.
- `realinput-1011c` B1 notes (`target/t3-ui-parity/realinput-1011c/b1-2set/00-b1-notes.txt`, G1-1 to G1-4).
