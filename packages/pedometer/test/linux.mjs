// @exact/pedometer under the declared substitute on the Linux host,
// headless (LLP 1133.026 §11, §15 stage 0): Linux has no step counter, so
// the status reads `available` false until the drive prefers it true, as a
// drive may on any target to drive the UI; then the count's track, a
// `since` a step moves, a hidden stretch, and data code's reader. Linux
// declares no `motion-fitness` permission (D19 item 2): no ticket is held
// here, which the iOS drives and `contract/cli/tests/it/pedometer.rs` cover.
// The fixture app is the one `exact new` makes outside the repo.
//
// usage: bun packages/pedometer/test/linux.mjs [--app <dir>]
//   --app: a fixture an earlier run made and built, as it stands.
import { resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

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
  const s = await open({ host: 'linux', app, storage: `pedometer-linux-${Date.now().toString(36)}` });
  try {
    await s.clock('settle');
    check('linux', 'the status reads absent', (await text(s, 'steps-status')) === 'false unknown', await text(s, 'steps-status'));
    await extension(s, 'pedometer.pedometerStatus={"available":true}');
    await s.clock('settle');
    check('linux', 'a preferred status drives the UI', (await text(s, 'steps-status')) === 'true unknown', await text(s, 'steps-status'));
    await s.tap('ask');
    await s.clock('settle');
    const state = await s.state();
    check('linux', 'no permission here: the request answers at once', (await text(s, 'asked')) === 'unknown' && !(state.tickets ?? []).length, `${await text(s, 'asked')} ${JSON.stringify(state.tickets)}`);

    const [first, since] = (await text(s, 'count')).split(' ').map(Number);
    check('linux', 'a count starts at 0, since its first ask', first === 0 && since > 0, await text(s, 'count'));
    await extension(s, 'pedometer.pedometer=[{"at":0,"steps":0},{"at":60000,"steps":110},{"at":120000,"steps":230}]');
    await s.clock('+61000');
    await s.clock('settle');
    check('linux', 'the track\'s step holds from its instant', (await text(s, 'count')) === `110 ${since}`, await text(s, 'count'));
    check('linux', 'data code\'s reader reads the same track', (await text(s, 'counted')) === '110', await text(s, 'counted'));

    await s.prefer({ 'visibility-state': 'hidden' });
    await s.clock('+600000');
    await s.prefer({ 'visibility-state': 'visible' });
    await s.clock('+2000');
    await s.clock('settle');
    check('linux', 'a hidden stretch reads the steps the track reached', (await text(s, 'count')) === `230 ${since}`, await text(s, 'count'));

    await extension(s, 'pedometer.pedometer=[{"at":0,"steps":12,"since":1790812800000}]');
    await s.clock('+2000');
    await s.clock('settle');
    check('linux', 'a step\'s own `since` moves it, as a reboot would', (await text(s, 'count')) === '12 1790812800000', await text(s, 'count'));

    await extension(s, 'pedometer.pedometer=clear');
    await s.tap('from-now');
    await s.clock('+2000');
    await s.clock('settle');
    const [, from] = (await text(s, 'count')).split(' ').map(Number);
    check('linux', 'a reader\'s `start` is its `since`', from > since, await text(s, 'count'));
    const extensions = JSON.stringify((await s.state()).extensions?.pedometer ?? {});
    check('linux', 'state.extensions shows the reader, not a count', extensions.includes('pedometer') && !extensions.includes('"steps":230'), extensions);
  } finally { await s.close(); }
} catch (e) {
  check('linux', 'ran', false, e.stack ?? e);
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
