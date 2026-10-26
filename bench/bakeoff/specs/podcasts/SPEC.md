# Podcasts

An iPhone podcast episode browser (no audio needed).

Data: `data/episodes.json`, 420 episodes of 12 shows (`id`, `show`, `title`, `date`, `minutes`, `summary`, `played`). Ship it.

1. The main screen lists every episode newest first: show, title, date, length, and whether it has been played.
2. A switch or filter shows only unplayed episodes.
3. A Shows screen lists the 12 shows with their episode and unplayed counts; tapping one lists that show's episodes.
4. An episode's screen shows its summary, date and length, and a Play button that marks it played (show a simple "playing" state; no audio).
5. Mark episodes played or unplayed from the list itself, without opening them.
6. Add an episode to an Up Next queue; Up Next can be reordered and items removed.
7. Played state and Up Next survive relaunching the app.
