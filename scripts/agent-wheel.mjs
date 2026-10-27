// The macOS driver's trackpad (LLP 1142 D3): `tap <target> wheel <dx> <dy> fling|hold`
// and `tap <target> wheel 0 0 release`, posted through the window server by
// `host/apple/wheel.swift` so AppKit runs the gesture as a person's.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** The helper, built once per source and Xcode into target/wheel-helper/<key>/ and published by one rename. */
export function wheelHelper() {
  const source = resolve(ROOT, 'host/apple/wheel.swift');
  const developer = spawnSync('xcode-select', ['-p'], { encoding: 'utf8' }).stdout.trim();
  const xcode = (() => { try { return readFileSync(resolve(developer, '../version.plist'), 'utf8'); } catch { return developer; } })();
  const key = createHash('sha256').update(JSON.stringify({ source: readFileSync(source, 'utf8'), xcode, arch: process.arch })).digest('hex').slice(0, 16);
  const dir = resolve(ROOT, 'target/wheel-helper', key), bin = resolve(dir, 'exact-wheel');
  if (!existsSync(bin)) {
    const stage = `${dir}.build-${process.pid}-${randomBytes(4).toString('hex')}`;
    try {
      mkdirSync(stage, { recursive: true });
      const built = spawnSync('xcrun', ['--sdk', 'macosx', 'swiftc', '-O', source, '-o', resolve(stage, 'exact-wheel')], { encoding: 'utf8' });
      if (built.status !== 0) throw new Error(`wheel helper: swiftc failed: ${built.stderr}`);
      try { renameSync(stage, dir); } catch (e) { if (!['EEXIST', 'ENOTEMPTY'].includes(e.code)) throw e; }
    } finally { rmSync(stage, { recursive: true, force: true }); }
  }
  return bin;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** One form, through `ask` (the carrier's request to the app). `held` keeps the open hold across operations:
 * `{ open: placement | null }`. AppKit's motion runs on the real clock, so a fling's reply waits for the scroller
 * to rest (at most 4 s) and the session's clock does not move. */
export async function platformWheel(ask, id, opts, held) {
  const form = opts.fling ? 'fling' : opts.hold ? 'hold' : 'release';
  if (form === 'release' && !held.open) throw new Error('wheel release: no held wheel is open');
  if (form !== 'release' && held.open) throw new Error(`wheel ${form}: a held wheel is open; \`tap <target> wheel 0 0 release\` lifts it first`);
  const where = await ask({ op: 'tap', id, wheel: opts.wheel, platform: true, ...(opts.at !== undefined ? { at: opts.at } : {}) });
  if (where.error) return where;
  const placed = form === 'release' ? held.open : where;
  const started = performance.now();
  const posted = await new Promise((done, failed) => {
    const p = spawn(wheelHelper(), [JSON.stringify({ pid: placed.pid, window: placed.window, point: placed.point, screen: placed.screen, mark: placed.mark, form, wheel: opts.wheel })]);
    let out = '', err = '';
    p.stdout.on('data', (d) => { out += d; });
    p.stderr.on('data', (d) => { err += d; });
    p.on('error', failed);
    p.on('exit', (code) => code === 0 ? done(JSON.parse(out)) : failed(new Error(`wheel ${form}: ${err.trim() || `the helper exited ${code}`}`)));
  });
  held.open = form === 'hold' ? where : null;
  // What the driver's events did: a scroller handed one to AppKit (a lift may go
  // to AppKit's own tracking and not past a scroller), and where it came to rest.
  const read = () => ask({ op: 'tap', id, wheel: [0, 0], platform: true });
  let after = await read(), still = 0;
  for (const until = performance.now() + 4000; form !== 'hold' && still < 3 && performance.now() < until;) {
    await sleep(50);
    const next = await read();
    still = String(next.offset) === String(after.offset) ? still + 1 : 0;
    after = next;
  }
  const delivered = after.delivered - where.delivered;
  if (form !== 'release' && delivered === 0) {
    held.open = null;
    throw new Error(`wheel ${form}: ${posted.posted} events were posted through the window server and no scroller took one: the display is locked or asleep, the window is covered, or nothing under the target scrolls`);
  }
  return {
    tapped: where.tapped, at: where.at, wheel: opts.wheel, [form]: true, delivery: 'platform', inputPath: 'CGEvent.postToPid',
    events: posted.posted, delivered, ...(after.scroller != null ? { scroller: after.scroller, from: where.scroller === after.scroller ? where.offset : null, offset: after.offset } : {}),
    ms: Math.round(performance.now() - started),
  };
}
