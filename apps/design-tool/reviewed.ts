// A draft's digests as first reviewed (the third hands-on pass's #2): Apply writes against them, so another client's
// write since is refused by name (409, `by`), never silently built on. The pin is the open draft's alone: a new
// value drafted is a new review (its diff shows the file as it is), and the draft's end (written, cancelled, another
// file) or a new server run drops it. The person's own writes move it along when it held the files they read: what
// they wrote is no one else's. A refusal re-pins to the files the server answered with, so a second Apply writes
// knowingly.
type Files = Record<string, string>;
const pins = new Map<string, Files>();
let asked = 0;
// One's own recent writes, each with the latest ticket when it landed: a proposal asked before one and answered after
// it pins the files that write read, and is moved along at once (else Apply refuses one's own write).
const own: { at: number; before: Files; after: Files }[] = [];
const along = (pin: Files, before: Files, after: Files) => {
  const wrote = Object.keys(before);
  return wrote.length && wrote.every(f => pin[f] === before[f]) ? { ...pin, ...Object.fromEntries(wrote.filter(f => f in after).map(f => [f, after[f]])) } : pin;
};

export const reviewed = {
  /** A proposal asked: its ticket. Only the latest asked may pin, so a slow answer for a value since replaced (or a
   * draft since put away) neither drops the open draft's pin nor brings its own back. */
  ask: (): number => ++asked,
  /** The draft `key`'s pinned digests, pinned now to `files` if it has none (and every other draft's dropped), when
   * `ticket` is the latest proposal asked; else what it has, pinning nothing. */
  pin(key: string, files: Files, ticket = asked): Files {
    if (ticket === asked && !pins.has(key)) { pins.clear(); pins.set(key, own.filter(w => w.at >= ticket).reduce((p, w) => along(p, w.before, w.after), files)); }
    return pins.get(key) ?? files;
  },
  get: (key: string): Files | undefined => pins.get(key),
  /** After a refusal: the files as the server has them now, the next Apply's. */
  repin(key: string, files: Files) { pins.set(key, files); },
  /** A draft ended (`key`), or every draft (a new server run, the edit put away). */
  end(key?: string) { if (key === undefined) pins.clear(); else pins.delete(key); },
  /** One's own write: `before` the digests it read of the files it wrote, `after` every file's after it. */
  advance(before: Files = {}, after: Files = {}) {
    if (!Object.keys(before).length) return;
    own.push({ at: asked, before, after }); if (own.length > 16) own.shift();
    for (const [key, pin] of pins) pins.set(key, along(pin, before, after));
  },
};
