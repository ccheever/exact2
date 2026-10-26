# Photos

An iPhone photo library browser.

Data: `data/photos.json`, 1,500 photos (`id`, `file`, `width`, `height`, `taken`, `album`, `favorite`); `file` names a JPEG in `data/images/` (up to 1,200 px on the long side). Ship the data and the images with the app.

1. The main screen is a grid of every photo, four across with thin gaps, square thumbnails cropped from the centre, oldest at the top and opening scrolled to the newest at the bottom, as Photos does. Day headings separate the days.
2. Tapping a thumbnail opens it full screen at its own aspect ratio; swipe left and right to move between photos; swipe down to close.
3. Favorite and unfavorite a photo from the full-screen view; a heart marks favorites in the grid.
4. An Albums tab lists the eight albums with a cover and a count; an album opens a grid of its photos.
5. Select mode: select several photos in the grid and favorite or delete them together.
6. Favorites and deletions survive relaunching the app.
