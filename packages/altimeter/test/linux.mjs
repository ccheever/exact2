// @exact/altimeter under the declared substitute on the Linux host,
// headless (LLP 1133.026 §11, §15 stage 0): Linux has no barometer, so the
// status reads `available` false until the drive prefers it true; then the
// altitude's track over its group's zero, the barometer's batch, a hidden
// stretch's `interrupted`, and the zero kept across it. The fixture app is the pedometer suite's
// (`packages/pedometer/test/fixture`), which reads both packages.
//
// usage: bun packages/altimeter/test/linux.mjs [--app <dir>]
//   --app: a fixture an earlier run made and built, as it stands.
import { resolve } from 'node:path';
import { checker, makeFixture, root, run } from '../../pedometer/test/fixture.mjs';

const argv = process.argv.slice(2);
const kept = argv.includes('--app') ? argv[argv.indexOf('--app') + 1] : null;
const { open, textOf } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const { app } = kept ? { app: kept } : makeFixture();
if (!kept) run('bun', ['exact.mjs', 'linux'], app);

const text = async (s, testId) => {
  const nodes = (await s.tree()).nodes, node = nodes.find((n) => n.props?.testId === testId);
  return node ? textOf(nodes, node) ?? '' : null;
};
const extension = (s, value) => s.prefer({ extension: value });

try {
  const s = await open({ host: 'linux', app, storage: `altimeter-linux-${Date.now().toString(36)}` });
  try {
    await s.clock('settle');
    check('linux', 'the status reads absent', (await text(s, 'height-status')) === 'false unknown', await text(s, 'height-status'));
    await extension(s, 'altimeter.altimeterStatus={"available":true}');
    await s.clock('settle');
    check('linux', 'a preferred status drives the UI', (await text(s, 'height-status')) === 'true unknown', await text(s, 'height-status'));

    const [zero, since] = (await text(s, 'climb')).split(' ').map(Number);
    check('linux', 'a group starts at 0 m with the moment of its zero', zero === 0 && since > 0, await text(s, 'climb'));
    check('linux', 'the barometer reads standard air', (await text(s, 'air')).startsWith('1013.25 '), await text(s, 'air'));
    await extension(s, 'altimeter.altimeter=[{"at":0,"relativeAltitude":0},{"at":30000,"relativeAltitude":86}]');
    await s.clock('+31000');
    await s.clock('settle');
    check('linux', 'a track gives the height, the zero unmoved', (await text(s, 'climb')) === `86 ${since} false`, await text(s, 'climb'));
    const [, samples] = (await text(s, 'air')).split(' ').map(Number);
    check('linux', 'the barometer\'s batch holds the samples since its last answer', samples > 0 && samples <= 16, await text(s, 'air'));

    await s.prefer({ 'visibility-state': 'hidden' });
    await s.clock('+60000');
    await s.clock('settle');
    check('linux', 'a poll while hidden reads interrupted', (await text(s, 'climb')).endsWith(' true'), await text(s, 'climb'));
    await s.prefer({ 'visibility-state': 'visible' });
    await s.clock('+2000');
    await s.clock('settle');
    check('linux', 'after the return the height and its zero hold', (await text(s, 'climb')) === `86 ${since} false`, await text(s, 'climb'));
  } finally { await s.close(); }
} catch (e) {
  check('linux', 'ran', false, e.stack ?? e);
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
