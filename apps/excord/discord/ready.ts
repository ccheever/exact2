// A large READY taken apart without parsing it whole (llp/0001 §4.1): on Hermes a turn has 100 ms, and
// JSON.parse took about 7.5 ms a megabyte on an M1 Max, so a phone's parse of a large account's READY
// would not fit in one. The scan finds `d.guilds` and each guild in it, a part a call; the rest of the
// frame parses at once without them, and each guild parses when it is applied.

/** Where a READY's guilds are in its text: the array's span and each element's. */
export interface ReadySpans {
  guilds: [number, number];
  elements: [number, number][];
}

/** A resumable scan of one frame's text: `step` goes on from where the last call stopped. */
export interface ReadyScan {
  text: string;
  at: number;
  // The open containers, `{` or `[`, outermost first.
  open: number[];
  // At the frame's top level and in `d`: the last string read, and the key a `:` made of it.
  last: string;
  key: string[];
  inD: boolean;
  guildsAt: number;
  elementAt: number;
  spans: ReadySpans | null;
  elements: [number, number][];
  failed: boolean;
}

const OBJECT = 0x7b, ARRAY = 0x5b;
const STRUCTURE = /[{}[\]":]/g;

export function startScan(text: string): ReadyScan {
  return { text, at: 0, open: [], last: '', key: [], inD: false, guildsAt: -1, elementAt: -1, spans: null,
    elements: [], failed: false };
}

/** Scan up to `budget` characters further. True when the scan has finished: `spans` holds the guilds,
 * or is null for a frame that is not a READY with a guilds array. */
export function step(scan: ReadyScan, budget: number): boolean {
  const { text } = scan;
  const stop = Math.min(text.length, scan.at + budget);
  STRUCTURE.lastIndex = scan.at;
  while (scan.at < stop) {
    const found = STRUCTURE.exec(text);
    if (!found) { scan.at = text.length; break; }
    const i = found.index, c = text.charCodeAt(i), depth = scan.open.length;
    scan.at = i + 1;
    if (c === 0x22) {
      // A string: its end is the next quote not escaped by an odd run of backslashes.
      let end = i;
      for (;;) {
        end = text.indexOf('"', end + 1);
        if (end < 0) { scan.failed = true; return finish(scan); }
        let slashes = 0;
        while (text.charCodeAt(end - 1 - slashes) === 0x5c) slashes++;
        if (slashes % 2 === 0) break;
      }
      if (depth === 1 || (depth === 2 && scan.inD)) scan.last = text.slice(i + 1, Math.min(end, i + 33));
      scan.at = end + 1;
      STRUCTURE.lastIndex = scan.at;
      continue;
    }
    if (c === 0x3a) {
      if (depth === 1 || (depth === 2 && scan.inD)) scan.key[depth] = scan.last;
      continue;
    }
    if (c === OBJECT || c === ARRAY) {
      if (depth === 1 && c === OBJECT && scan.key[1] === 'd') scan.inD = true;
      if (depth === 2 && scan.inD && c === ARRAY && scan.key[2] === 'guilds' && !scan.spans) scan.guildsAt = i;
      if (depth === 3 && scan.guildsAt >= 0 && !scan.spans && c === OBJECT) scan.elementAt = i;
      scan.open.push(c);
      continue;
    }
    // A close.
    const opened = scan.open.pop();
    if (opened === undefined || (opened === OBJECT) !== (c === 0x7d)) { scan.failed = true; return finish(scan); }
    if (depth === 4 && scan.elementAt >= 0 && !scan.spans) { scan.elements.push([scan.elementAt, i + 1]); scan.elementAt = -1; }
    if (depth === 3 && scan.guildsAt >= 0 && !scan.spans) {
      scan.spans = { guilds: [scan.guildsAt, i + 1], elements: scan.elements };
    }
    if (depth === 2 && scan.inD) { scan.inD = false; scan.key[2] = ''; }
    if (depth === 1) return finish(scan);
  }
  if (scan.at >= text.length) return finish(scan);
  return false;
}

function finish(scan: ReadyScan): true {
  if (scan.failed || scan.open.length || !scan.spans) scan.spans = null;
  scan.at = scan.text.length;
  return true;
}

/** The frame with its guilds array emptied, which parses as the READY without them. */
export function withoutGuilds(text: string, spans: ReadySpans): string {
  return text.slice(0, spans.guilds[0]) + '[]' + text.slice(spans.guilds[1]);
}
