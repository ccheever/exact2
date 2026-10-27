surfaces: web
agent: Claude Code (Claude Opus 5.5)
task: the open channel's row in the web's split, and the channel's heading at launch

### Rough

- At a desktop width the channel Excord reopens at launch showed beside an unmarked list:
  the web's split marked only the row whose press pushed the detail. Discord's web app
  always marks the open channel. The host now takes the row the app says is current
  (`aria-current`, other than `false`) when no row's press pushed the detail, and Excord
  says which channel is open on its channel and DM rows, which also tells a screen reader.
- The reopened channel's heading had the browser's focus ring at launch. The host moves the
  focus to a pushed route's heading, and Chrome draws `:focus-visible` for a script's focus
  on a page no one has touched. The focus still moves; before any press it moves with no
  ring.

### Lean in

- The split's row and the route's focus each live in one place in the host
  (`navigation.js`, `input-glue.js`) with a Chrome test beside the others, so the fix was
  two lines each.

### Needed

- navigation: whether the Mac's and iPad's splits select the reopened channel's row is not
  checked here.

### Checkpoints

- web, 1100×800: a second launch reopens #general with its row marked and no ring on its
  heading; tapping another channel moves the mark.
