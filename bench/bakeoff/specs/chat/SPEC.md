# Chat

An iPhone chat thread with one friend, Sam.

Data: `data/messages.json`, 520 messages (`id`, `from` "me" or "sam", `text`, `at`). Ship them as the history.

1. The thread shows messages as bubbles, mine on the right and Sam's on the left, oldest at top, with a date heading where the day changes. It opens scrolled to the newest message.
2. A composer at the bottom sends a message; the thread stays at the bottom showing it, and the composer clears. Empty messages cannot be sent.
3. The composer stays above the keyboard; the newest messages stay visible when the keyboard opens.
4. Sam replies "👍" or a short canned reply two seconds after each message you send, with a "Sam is typing…" indicator meanwhile.
5. Long-press a message to copy or delete it.
6. Scrolling up through history works smoothly all the way to the first message; a button returns to the newest message when you are far up.
7. Sent messages survive relaunching the app.
