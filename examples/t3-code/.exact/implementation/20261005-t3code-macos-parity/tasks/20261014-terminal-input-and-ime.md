---
name: 20261014-terminal-input-and-ime
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
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

## Cause and fix

**G1-1, G1-2.** The page is the reference's own Ghostty surface, unchanged: it sends what the input method commits
(`onCompositionEnd`) and ignores a composing keydown (`isTerminalCompositionKey`). The clone's difference is the native
key path in front of it. `T3TerminalView.routeTerminalKey` runs from a local key monitor and the key equivalents, both
ahead of WebKit's `keyDown`, which is where the input method first sees a key; Chromium's page sees a key only after
the input method. So the terminal's bindings and readline keys could act on a key the input method owned (⌥← sent
`ESC b` ahead of the syllable), and a ⌃ chord nobody handled went to WebKit, whose composing keydown the surface ignores:
⌃C left ㅎ composing (G1-2), and it was committed in front of the next paste (`ㅎecho`, G1-4 a). While text composes
(the page's composition state, below, or WebKit's marked text) the view now routes by `T3TerminalKeys.composing`:

- a ⌘ chord is the app's, as before;
- a ⌃ chord commits the composition as it stands (`unmarkText`, then `discardMarkedText`, WebKit's own order when a
  page ends a composition) and, once the page's `compositionend` has sent the text, is the terminal's (a binding, a
  readline key: ⌃L clears after 하) or nothing, as Chromium's composing keydown (key code 229) is: ⌃C commits ㅎ and sends
  no ^C, as the reference;
- ⌥Return is offered to the input context first, synchronously, once (this path can see a key three times), as
  Chromium's `interpretKeyEvents` offers it: the 2-Set Korean input method's Hanja list;
- every other key is WebKit's, whose `keyDown` offers it to the input method before the page; the sign-in terminal's
  Escape commits first and then goes to its PTY (AppKit would otherwise dismiss the form).

WebKit's own marked-text answer (`hasMarkedText` on the web view) waits on an editor state the page sends late and, in
a window that is not key, never (the AppKit test window): the page now posts its input's composing state
(`entry.ts` `observeComposition`: `compositionstart` → true, `compositionend` or `blur` → false, once each), and the view
takes whichever knows first.

The composition's *states* at the session's 60 ms cadence (the reference's ㅎ, 하 where the clone sent ㅎ, ㅏ) are not
reproducible here: no input method can be driven in agent mode, and both apps leaked intermediate text there (neither
gave `한글 ` once). The page code is the reference's; what differs is the engine (WebKit tells the input method to drop
a composition the page or engine ended, Chromium does not, so Chromium's input method continued ㅎ into 하). The clone
does not imitate that leak: see "Decision needed".

**G1-3.** AppKit gives the first responder only to a left press; the reference's pane activates on `onMouseDown` for
every button and the surface focuses its input on `pointerdown`. `T3TerminalView.WebView.otherMouseDown` now lets WebKit
handle the press (mouse reporting; macOS has no middle-click paste, `isMiddleClickPastePlatform`), then takes the first
responder and focuses the page's input (after WebKit's default, which moves the page focus off the input; Chromium's
moves it to the viewport's container), so the pane's focus-in activates it. In a bare AppKit window WebKit takes the
first responder itself and the isolated test passes on the feature tip too; in the app it did not (the agent drive and
`realinput-1011c` both kept the composer). The close confirm of a right-panel terminal surface
(`terminalSurfaceCloseCopy`) lists the surface's active terminal first, then the others in order, as
`ChatView.closeRightPanelSurface`.

**KW-8.** `terminal.contract:105` drops `box-sizing="border-box"` and `interpolate-size="allow-keywords"`. With Panel
animations at 400 ms, the drawer's shown height on each 50 ms frame of the agent clock is the same before and after,
opening (0, 56, 106, 151, 192, 226, 254, 272, 280 pt) and closing (280, 224, 174, 128, 88, 54, 26, 7, 0). The film op
(`screenshot … over … every`) reads the screen, black while the Mac is locked, so the frames are `screenshot … window`
steps with `clock +50`. README pitfall #99 and EXACT2-GAPS' row say it is retired.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| G1-1 | built; the input-method half not verifiable in agent mode. AppKit: while 한 composes ⌥Return is offered to the input context once and never encoded by the terminal (the ABC layout's context makes it a newline); without a composition it is Ghostty's `ESC CR` again; the routing table holds for 14 chords. Bun: the composing state goes out as one start and one end per composition. The Hanja list and the cadence states: real-input steps 1-2 | [g1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/9aaed593d481a6d5fa2dc32e21e97086a4a830ec/terminal-input-and-ime/g1-record.txt) |
| G1-2 | pass (AppKit, before/after): before, ⌃C with ㅎ composing sent nothing and ㅎ stayed composing, and ⌃L sent `0C` ahead of 하; after, ⌃C commits ㅎ once and sends no ^C, the next ⌃C is the shell's `03`, ⌃L commits 하 then clears. Under the real input method: real-input step 3 | [g1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/9aaed593d481a6d5fa2dc32e21e97086a4a830ec/terminal-input-and-ime/g1-record.txt) |
| G1-3 | pass (agent drive, before/after/reference): with the composer focused, a middle press on Terminal 1 left the focus in the composer and Terminal 2 active before; after, Terminal 1 has the focus (its filled cursor) and is the active pane, nothing pasted. Close order: Bun (the active terminal first, as `closeRightPanelSurface`). Under a real middle button: real-input step 4 | [image](https://raw.githubusercontent.com/ccheever/exact2/826efc3a18db4867c6a3592fa925c788c8679c5c/terminal-input-and-ime/g1-3-middle-press.png), [g1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/9aaed593d481a6d5fa2dc32e21e97086a4a830ec/terminal-input-and-ime/g1-record.txt) |
| KW-8 | pass (agent drive, before/after films): the same height at every frame, opening and closing | [film](https://raw.githubusercontent.com/ccheever/exact2/025ec6182df1c43bf1d016508b847baab9ecd75f/terminal-input-and-ime/kw8-drawer-film.png), [g1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/9aaed593d481a6d5fa2dc32e21e97086a4a830ec/terminal-input-and-ime/g1-record.txt) |

Drives (agent mode, 1280×840, `target/t3-audit/clone-drive.sh`, lane `terminal-input-and-ime`, base port 16420, a fresh
`--storage` each): before on this worktree built at the feature tip (`609f392d5`'s `examples/t3-code`), after on this
branch's bundle (`71c9602b9`'s app code). The reference column is `realinput-1011c` G1-3b (the reference under a real
middle button); the reference's drawer motion (`ChatView.tsx:1270`, `grid-template-rows` 0fr→1fr over the panel
duration) was not filmed: KW-8 compares the clone before and after.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout), a thread open
(Verification fixture › Timeline verification), the drawer open with one terminal at an empty prompt. Switch the input
source to 2-Set Korean for steps 1-3 (and back to ABC after); never send a message.

1. **G1-1 states.** Run `head -c 9 | xxd`; type g k s r m f Space at a normal pace, then at about 60 ms per key. Expected:
   `ed959c eab880 20` (한글 and a space) at a normal pace; at 60 ms record what each app sends (the reference sent ㅎ, 하).
   ⌃C between runs.
2. **G1-1 Hanja.** Run `head -c 3 | xxd`; type g k s (한 composing), ⌥Return. Expected (reference): the Hanja candidate
   window opens at the cursor cell (the reference's `head` got 한, `ed959c`, as it opened) and ⌥Return sends no line
   break; press 1: 韓 goes in at the next prompt. The clone before: no window, ⌥Return reached the shell as a line break.
3. **G1-2.** At an empty prompt type g (ㅎ composing), ⌃C. Expected (reference): ㅎ is committed at the prompt (`$ ㅎ`), no
   ^C, no new prompt, nothing left composing: ⌃U clears the line, and a second ⌃C prints ^C and a new prompt. Then g,
   ⌃L: ㅎ is committed and the screen clears.
4. **G1-3.** Split the drawer (⌘D: Terminal 1 | Terminal 2, Terminal 2 active), click the composer, press the middle
   button on Terminal 1's prompt. Expected (reference): the terminal takes the focus (its cursor fills) and Terminal 1
   becomes active, nothing is pasted; type `echo mid` and Return: it runs in Terminal 1. Then right panel › Terminal, ⌘D
   in it, press the middle button on its tab: "Close 2 terminals?" names the panel's active terminal first.

## Tests

- `terminal-host/src/entry.test.ts` (+1): the input's composing state goes out as states, one start and one end per
  composition, whatever its updates; a repeated start changes nothing; leaving the input ends it.
- `terminal-panel.test.ts` (+1, 1 changed): the surface close confirm names its active terminal first, then the others
  in order (`"Terminal 3", "Terminal 1", "Terminal 2"`, and after activating Terminal 2, `"Terminal 2", "Terminal 1",
  "Terminal 3"`); the split case now reads `"Terminal 2", "Terminal 1"`.
- `macos/tests/terminal` (AppKit, +4): `testControlChordCommitsTheCompositionFirst` (G1-2),
  `testOptionReturnGoesToTheInputContextFirstWhileComposing` (G1-1), `testCompositionRoutes` (the routing table, 14
  chords), `testMiddlePressFocusesTheTerminalWithoutPasting` (G1-3: first responder, the page's input focused, no bytes,
  nothing in the composer). 42 run, 0 failed; against the feature tip's modules (less the two that read the new API),
  40 run, 3 failures, all in the G1-2 test.

## Decision needed

G1-1's cadence states: at about 60 ms per key the reference sent the composition's intermediate states to the shell
(ㅎ, then 하: the shell got `ㅎ하…`, not `한글`), a leak of Chromium's input handling (its input method continued a
composition the engine had ended); the reference's own code (`surface.ts`) sends only what the input method commits.
The clone keeps sending only commits and does not imitate the leak. If real-input step 1 still shows the clone sending
separate jamo (ㅎ, ㅏ) at that cadence, should the clone imitate Chromium's states (which would also put ㅎ and 하 in the
shell), or is "한글 once at a normal pace" the bar?

## Not done / not verified

- G1-1 and G1-2 under the real 2-Set Korean input method (the Hanja window, the cadence states, ⌃C's commit): no input
  method can be driven in agent mode and the input source is not to be switched here; real-input steps 1-3.
- G1-3 under a real middle button: real-input step 4 (the agent's `auxclick` is a CGEvent through `NSApp.sendEvent`).
