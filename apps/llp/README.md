# LLP

A reader for a directory of LLP documents — the general Markdown reader
(`apps/markdown`) specialised for a numbered corpus (LLP 1033).

```sh
exact install llp          # ~/Applications/LLP.app, and `llpview` on PATH
llpview ~/projects/exact2/llp
llpview llp/1033-markdown-viewer.plan.md

exact run llp llp/
bun host/web/dev.mjs --app llp
```

What it adds to the general reader, all of it below the data seam:

- **The index**, in number order, with sub-documents (`1030.000`) indented
  under their parents, and each row's kind, status, and `current`/`foundation`
  overlay membership. Opening a folder opens its lowest-numbered document.
- **Search over the corpus**, not one document: every document's text, with a
  hit count per document. The corpus is read once and searched in memory.
- **The outline**, which shows one section at a time — a heading and
  everything under it until the next of equal or higher level. There is no
  scrolling to an anchor in v1, and for a 16,000-word document showing the
  section is the better of the two anyway. "Whole document" goes back.
- **Cross-references**: `LLP 1234`, `LLP 1030.000` and `RFC 0491` in running
  prose are links to the documents they name. A number the corpus does not
  have stays plain text; a code span stays code.

The document itself — the parser, the blocks, the runs, the type scale — is
`apps/markdown`. `data/` depends on `markdown-parse`. The `Blocks` and `Runs`
components in `app.contract` are copied from the general reader rather than
shared: Contract's `use … from` resolves only inside one app directory, so two
apps cannot share a `.contract` file today (LLP 1033, the landing note).
