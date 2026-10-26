// A linked package's design on the local wire (LLP 1121 D11, stage 11 A2
// part 2, *1.14*): read-only, but a change to one on disk is a change to
// what the app's design folds, so clients are told as of a hand edit (a
// `changed` event naming it in `edited`, with the packages' digests), a
// scrub competes, and the server's own reads are taken again. What the
// design server says to watch (`hello {paths: true}`'s `watch`) is
// watched: each linked package's directory, whole (a design made, renamed
// by its `package.json`, in a directory of its own), and each directory a
// package is installed in (a link retargeted, an install replacing it); an
// event there reads again (`changed`); one listed once is watched while it
// is there. A held proposal is planned again on
// accept, on the folds as they are then (the design server's write checks),
// so one the change makes invalid is refused.

import { statSync, watch } from 'node:fs';

/** The tracker; call it with each `hello {paths: true}` reply's
 * `packages` and `watch` (absent: none), and whether that read is the
 * baseline. `log` says a directory that can't be watched. */
export function packageDesigns({ announce, changed, log = () => {} }) {
  const digests = new Map(), watchers = new Map();
  const follow = (dir, recursive) => {
    let ino = null;
    try { ino = statSync(dir).ino; } catch { /* not there now */ }
    const held = watchers.get(dir);
    if (held && held.ino === ino) return;
    held?.watcher.close();
    watchers.delete(dir);
    if (ino === null) return;
    try { watchers.set(dir, { ino, watcher: watch(dir, { recursive }, () => changed()) }); } catch (e) { log(`tamber: cannot watch ${dir}: ${e.message}`); }
  };
  // Every directory ever listed, watched while it is there: one a read lists no longer while it
  // stands (a manifest or an export missing for a while) still says when it is mended.
  const listed = new Map(), installs = new Set(), there = new Map();
  const ino = dir => { try { return statSync(dir).ino; } catch { return null; } };
  const track = (packages = [], baseline = false, watched = {}) => {
    for (const dir of watched.roots ?? []) listed.set(dir, true);
    for (const dir of watched.links ?? []) if (!listed.has(dir)) listed.set(dir, false);
    for (const path of watched.installs ?? []) installs.add(path);
    // Each one's presence as of its listing: what the look compares with.
    for (const dir of [...listed.keys(), ...installs]) if (!there.has(dir)) there.set(dir, ino(dir));
    for (const [dir, recursive] of listed) follow(dir, recursive);
    // A design's text and the package SVGs it draws (`assets`, stage 13): an SVG edit changes `draws`.
    // A package with no design whose SVGs the build reads is `watch.assets`' (`<package>/assets/`).
    const now = new Map([...packages, ...(watched.assets ?? [])].map(p => [p.file, `${p.digest ?? ''}:${p.assets ?? ''}`]));
    const edited = [...new Set([...now.keys(), ...digests.keys()])].filter(file => now.get(file) !== digests.get(file));
    digests.clear();
    for (const [file, digest] of now) digests.set(file, digest);
    if (!baseline && edited.length) announce(edited, packages.map(({ package: name, file, digest }) => ({ package: name, file, digest })));
  };
  // What no watch can say (a directory outside the app removed, then made again where nothing above
  // it is watched; a link's target made later): each listed directory and install path looked at
  // every 2 s, and one that came or went reads again.
  const look = setInterval(() => {
    for (const dir of [...listed.keys(), ...installs]) {
      const now = ino(dir);
      if (there.get(dir) !== now) changed();
      there.set(dir, now);
    }
  }, 2000);
  look.unref?.();
  track.close = () => { clearInterval(look); for (const { watcher } of watchers.values()) watcher.close(); watchers.clear(); };
  return track;
}
