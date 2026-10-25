// A draft's digests as first reviewed (the third hands-on pass's #2): Apply writes against them, so another client's
// write since is refused by name (409, `by`), never silently built on. The pin is the open draft's alone: a new
// value drafted is a new review (its diff shows the file as it is), and the draft's end (written, cancelled, another
// file) or a new server run drops it. The person's own writes move it along when it held the files they read: what
// they wrote is no one else's. A refusal re-pins to the files the server answered with, so a second Apply writes
// knowingly.
type Files = Record<string, string>;
const pins = new Map<string, Files>();

export const reviewed = {
  /** The draft `key`'s pinned digests, pinned now to `files` if it has none (and every other draft's dropped). */
  pin(key: string, files: Files): Files {
    if (!pins.has(key)) { pins.clear(); pins.set(key, files); }
    return pins.get(key)!;
  },
  get: (key: string): Files | undefined => pins.get(key),
  /** After a refusal: the files as the server has them now, the next Apply's. */
  repin(key: string, files: Files) { pins.set(key, files); },
  /** A draft ended (`key`), or every draft (a new server run, the edit put away). */
  end(key?: string) { if (key === undefined) pins.clear(); else pins.delete(key); },
  /** One's own write: `before` the digests it read of the files it wrote, `after` every file's after it. */
  advance(before: Files = {}, after: Files = {}) {
    const wrote = Object.keys(before);
    if (!wrote.length) return;
    for (const [key, pin] of pins) {
      if (wrote.every(f => pin[f] === before[f])) pins.set(key, { ...pin, ...Object.fromEntries(wrote.filter(f => f in after).map(f => [f, after[f]])) });
    }
  },
};
