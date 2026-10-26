// @exact/barcode-detection's and @exact/text-detection's suite under the
// agent (LLP 1133.027 §9, §12 stages 1 and 2), the same on each host named:
// the fixture app's stills (drive 5), text read from a still where the
// platform has a recognizer (drive 6's module half), and a live scan of the
// camera's agent picture (drives 2 and 3). Vision answers for real on Apple
// under the agent; the web's agent answers barcodes from the fixture's
// pixels and has no text recognizer.
//
// The first text read in a newly built app waits while the Neural Engine
// compiles the recognizer, a minute or more, so the suite gives it two.
// `EXACT_VISION_FIXTURE=<dir>` keeps the fixture app there between runs.
//
// usage: bun packages/barcode-detection/test/agent.mjs [web] [macos] [ios] [--sim <udid>]
import { spawnSync } from 'node:child_process';
import { STILLS, boxOf, checker, discard, images, makeFixture, near, run, shown } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['web', 'macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { check, failed } = checker();
const { app } = makeFixture();
const env = { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };

/** The quote's four lines, as `make-fixtures.swift` drew them. */
const QUOTE = ['It is a truth universally acknowledged,', 'that a single man in possession', 'of a good fortune, must be', 'in want of a wife.'];

for (const host of hosts.length ? hosts : ['web']) {
  const reads = host !== 'web';
  if (host === 'macos') run('bun', ['exact.mjs', 'mac'], app, env);
  if (host === 'ios') run('bun', ['exact.mjs', 'ios', ...(sim ? ['--sim', sim] : [])], app, env);
  const ops = [...(host === 'ios' && sim ? ['--sim', sim] : []), '--storage', `vision-${process.pid}`, '--permission', 'camera=granted', 'clock data'];
  for (const [name, image, button] of STILLS) {
    ops.push('tap choose', `pick @image ${images[image]}`, 'clock data', `tap ${button}`, 'clock data', 'tree found', ...(name.includes('nothing') || name.includes('none') ? [] : ['tree code']));
  }
  ops.push('tap detect-unknown', 'clock data', 'tree found-error', 'tap detect-outside', 'clock data', 'tree found-error', 'tap detect-missing', 'clock data', 'tree found-error', 'tree reader', 'tree reader-en');
  if (reads) {
    for (const image of ['quote', 'rotatedQuote']) {
      ops.push('tap choose', `pick @image ${images[image]}`, 'clock data', 'tap read-text', ...Array(6).fill('clock data'), 'tree read', 'tree lines');
    }
    ops.push('tap read-missing', 'clock data', 'tree read-error');
  } else {
    ops.push('tap read-missing', 'clock data', 'tree read-error');
  }
  ops.push('prefer extension camera.picture={"path":"assets/test/isbn-9780747532743.png"}', 'tap open', 'clock settle', 'clock +300', 'clock data', 'tree scan', 'tree seen',
    'prefer extension camera.picture={"path":"assets/test/none.png"}', 'clock +1000', 'clock data', 'tree scan', 'tree seen',
    'tap toggle', 'clock +300', 'clock data', 'tree scan', 'tap toggle', 'tap stop', 'clock +300', 'clock data', 'tree scan', 'logs');
  const r = spawnSync('bun', ['exact.mjs', 'agent', host, ...ops], { cwd: app, env, encoding: 'utf8', maxBuffer: 64 << 20 });
  const out = `${r.stdout}\n${r.stderr}`, codes = shown(out, 'code'), found = shown(out, 'found');
  check(host, 'the drive ran', r.status === 0, out.split('\n').slice(-20).join('\n'));
  const want = STILLS.flatMap(([, , , w]) => w);
  check(host, 'each still answers its codes, D11\'s rule applied', JSON.stringify(codes.map(c => c.split(' ').slice(0, 2).join(' '))) === JSON.stringify(want), JSON.stringify(codes));
  check(host, 'the rotated JPEG is upright: 468x280 and the ISBN\'s box', found[6] === '468x280 1' && near(boxOf(codes[5] ?? '')), `${found[6]} ${codes[5]}`);
  check(host, 'none.png answers an empty list', found[7] === '400x300 0', found[7]);
  check(host, 'an unknown format, a path outside the grants, a missing file', JSON.stringify(shown(out, 'found-error').slice(-3)) === '["TypeError","NotAllowedError","NotFoundError"]', JSON.stringify(shown(out, 'found-error')));
  if (reads) {
    const read = shown(out, 'read'), lines = shown(out, 'lines');
    check(host, 'text is available, en-US and fr matched to the recognizer\'s tags', /^available - \d+$/.test(shown(out, 'reader')[0] ?? '') && shown(out, 'reader-en')[0] === 'available - en-US,fr-FR', `${shown(out, 'reader')} ${shown(out, 'reader-en')}`);
    check(host, 'the quote reads its four lines', read[0] === '1200x360 4' && lines[0] === QUOTE.join('|'), `${read[0]} ${lines[0]}`);
    check(host, 'the rotated quote reads upright, the same four lines', /^\d+x\d+ 4$/.test(read[1] ?? '') && lines[1] === QUOTE.join('|'), `${read[1]} ${lines[1]}`);
    check(host, 'a missing file fails NotFoundError', shown(out, 'read-error').at(-1) === 'NotFoundError', shown(out, 'read-error').join());
  } else {
    check(host, 'text on the web: NotSupportedError and unavailable, not-supported', shown(out, 'read-error').at(-1) === 'NotSupportedError' && shown(out, 'reader').at(-1) === 'unavailable not-supported 0', `${shown(out, 'read-error')} ${shown(out, 'reader')}`);
  }
  const scans = shown(out, 'scan'), seen = shown(out, 'seen');
  check(host, 'the agent picture\'s ISBN is scanned and seen', /^scanning \d+ 1 468x280 -$/.test(scans[0] ?? '') && /^ean_13 9780747532743 /.test(seen[0] ?? ''), `${scans[0]} ${seen[0]}`);
  check(host, 'a code for one frame: barcodes empty, seen keeps it (drive 3)', /^scanning \d+ 0 400x300 -$/.test(scans[1] ?? '') && /^ean_13 9780747532743 /.test(seen.at(-1) ?? ''), `${scans[1]} ${seen.at(-1)}`);
  check(host, 'a hidden viewfinder idles the scan, a stopped track ends it (drive 2)', /^idle /.test(scans[2] ?? '') && /^ended /.test(scans[3] ?? ''), `${scans[2]} ${scans[3]}`);
  check(host, 'the observer took no lease', /an observer joins, taking no lease/.test(out) && !/observer[^\n]*takes the lease/.test(out), out.split('\n').filter(l => /lease|observer/.test(l)).join(' | '));
}
discard(app);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
