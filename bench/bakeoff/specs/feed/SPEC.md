# Feed

An iPhone social feed (no network: everything is local).

Data: `data/posts.json`, 400 posts (`id`, `author`, `handle`, `minutesAgo`, `text`, `image`, `carousel`, `likes`, `liked`, `replies`); `image` and each carousel card's `image` name a file in `data/images/` (JPEGs up to 900 px; `data/images/images.json` gives each one's width and height). Ship the data and the images with the app.

1. The main screen is one long feed, newest first. Each post shows the author, handle and relative time, its text in full (posts vary from one line to a paragraph), its image when it has one (full width, at the image's own aspect ratio, corners rounded), and its carousel when it has one: a horizontally scrolling row of cards, each an image with a title.
2. A post's like button toggles liked and the count; a reply count is shown.
3. Tapping a post opens it on its own screen with its image larger; tapping an image opens it full screen, dismissed by a swipe down.
4. Pull to refresh adds three new posts at the top (made up locally).
5. A compose button writes a text post, which appears at the top.
6. Likes and new posts survive relaunching the app.
