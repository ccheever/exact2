// Why a live proposal would not apply in place on the JS dev loop's pages
// (LLP 1121 stage 6; tamber-server.mjs `proposeHere` makes such a one
// `rebuild`): a token the pages write inline (a font-weight number, an SVG
// attribute's) whose value moves. What can't be known yet is a rebuild too,
// never live (theme2's ruling of 2026-10-23 10:01): with no design reader
// built, a proposal does not wait on a compile, so it says so and takes the
// safe class, as a motion value no row reads does.

/** `texts`: the proposal's files; `file`: the design's among them; `ready()`:
 * whether the reader is built; `read(text)`: its design (`{tokens}` or
 * `{error}`), read only; `pages()`: the design the pages run, asked once the
 * read is in (a build may land meanwhile); `same(a, b)`: two tokens alike.
 * Null when the edit applies in place. */
export async function inlineWhy({ texts, file, ready, read, pages, same }) {
  if (!(file in texts)) return null;
  if (!ready()) return 'the design reader is not built yet, so what these pages write inline is not yet known: rebuild assumed';
  const next = await read(texts[file]);
  const from = pages();
  if (next.error) return `the design reader could not read it (${next.error}): rebuild assumed`;
  if (!from) return 'the design these pages run is not read yet: rebuild assumed';
  if (next.tokens.length !== from.tokens.length) return 'it changes the design\'s tokens, which no page previews in place';
  const inline = next.tokens.find((t, i) => t.inline && t.name === from.tokens[i].name && !same(t, from.tokens[i]));
  return inline ? `\`${inline.name}\` is written inline on this page (${inline.writer ?? 'a font weight, an SVG attribute'})` : null;
}
