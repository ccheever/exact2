# Markdown

A reader for Markdown files, on macOS, iOS and the web.

```sh
exact install markdown     # ~/Applications/Markdown.app, and `mdview` on PATH
mdview README.md
mdview ~/notes             # a folder opens its README.md

exact run markdown README.md   # from this repo, in the foreground, with its log
bun host/web/dev.mjs --app markdown
```

Open a document with `⌘O`, a Finder double-click or Open With, a path typed in
the field at the top, `mdview` from a shell, the Open… and Folder… buttons, or
by following a link inside a document that names a file beside it. All of
those are the same event: a value arriving at the `open-file` node, which is
a `doc:` path the host minted for what you chose. A second
`mdview` hands its file to the copy already running rather than starting
another.

Headings, prose with bold/italic/code/links, ordered and unordered lists,
block quotes, rules, fenced code, images, and pipe tables. Raw HTML is shown
and interpreted by nothing. Reference links, setext headings and footnotes are
not interpreted. Files are read up to 4 MB and must be UTF-8; one that will
not open leaves the document you were reading where it was and says why.

The strip on the left lists the Markdown beside the open file. Text selects
and copies; a link to a page opens in the browser.

## How it is put together

- `parse/` (`markdown-parse`) — the parser: text to ordered blocks and styled
  runs, and their conversion into plan values. No I/O, no host. `apps/llp`
  depends on this crate; it is what the two readers share.
- `data/` (`markdown-data`) — the data source: `open(path)`. Reads through
  storage requests under `fs.read doc:/` (stat, the folder's README, the
  file, the folder beside it) and parses on the host's worker
  through a continuation, so neither is on the thread that lays out. It owns
  the open document, which is why a refusal does not lose it.
- `app.contract` — the whole view. A document is a flat list of blocks
  (Contract inlines components syntactically and cannot recurse), and a
  paragraph is a `text` node whose children are its runs.
- `app.json` — `file_handlers` says what this opens; the macOS bake derives
  `CFBundleDocumentTypes` from it. `app.command` is `mdview`.

On the web the reader opens what Open… or Folder… chose, through the File
System Access API's pickers where the browser has them (Chromium); elsewhere
the pickers refuse, and the reader shows its welcome document. A typed path
opens nothing there: a page has no filesystem of its own.
