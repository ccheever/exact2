```prompt Add a chat thread
Add a chat thread to the Exact app in this folder: a transcript that opens at its newest message, over a composer like Messages'. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/chat-thread.md as the source for each step. The exact2 checkout has the same page at docs/recipes/chat-thread.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say where the messages come from and who is in the thread, propose it from the app and ask me before you write code. Use the recipe's local `loadThread` and `sendMessage` sources in app.ts until I give you a server.
3. In app.contract, add a `resource` for the thread, a `mutation` for a sent message that `refreshes` the thread `then` scrolls to the end, and a `draft` state. Draw the transcript as a `list` with `virtualized=true`, `scroll-start="end"` and `scrollFollowEnd=true`, and give each message `overflow-wrap="anywhere"`.
4. Draw the composer below it: a `textarea` with `field-sizing="content"` and a Send button that is disabled while the draft is empty or a send is pending, with `retainFocus=true`. Give the screen `interactive-widget="resizes-content"`, so the composer stays over the keyboard.
5. Post trims the draft, sends it, clears it and keeps focus in the composer.
6. Add a test in app.test.contract that types a message, taps Send, runs `clock data`, and expects the composer to be empty and the message in the transcript.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me where the thread is and where its messages come from, what the tests and verify reported, and the command that opens the app: `bun exact.mjs web`.
```
