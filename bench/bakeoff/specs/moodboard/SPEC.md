# Moodboard

An iPhone image-board browser with generated tiles instead of photos.

Data: `data/tiles.json`, 600 tiles (`id`, `title`, `board`, `hue`, `saturation`, `lightness`, `symbol`, `added`, `liked`). Draw each tile as a square of its HSL colour with its symbol centred. Ship the data.

1. The main screen is a grid of all tiles, three per row, newest first, filling the width.
2. A segmented control or menu filters by board (ten boards plus All).
3. Tap a tile to open it large, with its title, board and date; swipe left and right to move to neighbouring tiles.
4. Like and unlike a tile from its large view; a Liked screen shows liked tiles in the same grid.
5. Select mode: select several tiles in the grid and like or delete them together.
6. Likes and deletions survive relaunching the app.
