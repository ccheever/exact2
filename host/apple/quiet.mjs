// Exact's own Swift warnings, left out of an app's build output (LLP 1116.021 D3). A builder of
// an app outside this checkout can do nothing about a warning in host/apple/Sources, and a cold
// iOS build printed some forty of them, re-sent with the rest of the context at every later
// step. The app's own Swift, every error and everything else the toolchain says are kept, and
// a build of an app in this checkout (Exact's own development) keeps them all. A Swift
// diagnostic is a `<file>:<line>:<col>: warning: …` line, then its source excerpt (lines with a
// `|` gutter) and a blank line; a group's notes (`[#NoUsage]: <https://…>`) go when no kept
// line cites them.

const ANSI = /\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b\x07]*(?:\x1b\\|\x07)/g;
const plain = (line) => line.replace(ANSI, '');
const HEADER = /^(\/[^:]+\.swift):\d+:\d+: (warning|error|note): /;
const EXCERPT = /^\s*\d*\s*\|/;
const FOOTNOTE = /^\[#(\w+)\]: </;

let own = null;

/** Leaves Exact's own warnings out of what this build prints when `app` is outside the checkout `root`. */
export function quietFor(app, root) {
  own = app.dir.startsWith(`${root}/`) ? null : `${root}/`;
  return app;
}

/** `output` without the Swift warnings whose file is under `root` (unchanged when `root` is null). */
export function quiet(output, root = own) {
  if (!root || !output.includes('warning: ')) return output;
  const kept = [];
  let dropping = false;
  for (const line of output.split('\n')) {
    const header = HEADER.exec(plain(line));
    if (header) dropping = header[2] === 'warning' && header[1].startsWith(root);
    else if (dropping && !EXCERPT.test(plain(line))) {
      dropping = false;
      if (line.trim() === '') continue; // the dropped block's own blank line
    }
    if (!dropping) kept.push(line);
  }
  const text = plain(kept.join('\n'));
  const cited = (name) => text.split(`[#${name}]`).length > text.split(`[#${name}]: <`).length;
  return kept.filter((line) => {
    const note = FOOTNOTE.exec(plain(line));
    return !note || cited(note[1]);
  }).join('\n');
}
