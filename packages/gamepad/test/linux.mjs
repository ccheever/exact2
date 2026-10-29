// @exact/gamepad on the Linux host, headless (LLP 1133.022 §10, D10 items 1
// and 5): the fixture app made outside the repo, its Linux crate built with
// the package's Rust implementation composed, and driven with a test
// build's pads (`EXACT_AGENT_PADS`): the host's registry and its claims with
// a recorder for motors, the path a real pad takes from the registry on.
//
// - Bound (`--bind gamepad`), the list is the registry's, by index, with
//   each pad's family and labels; the pad pressed is the current one; a
//   rumble reaches the pad's motors and completes after its span; a reset
//   preempts a running one; a light is refused, since no pad here has one.
//   A pad that disconnects ends the effect playing on it, `preempted`, and
//   raises `gamepaddisconnected`.
//   `clock settle` crosses a rumble: its answer comes at the span's end on
//   the host's clock, not on wall time.
// - Under the agent unbound, the declared substitute answers, and nothing
//   reaches the registry.
//
// usage: bun packages/gamepad/test/linux.mjs [<app dir>]
//   (an app already made by fixture.mjs is reused, and built again)
import { existsSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { checker, makeFixture, run } from './fixture.mjs';

const given = process.argv[2] && resolve(process.argv[2]);
const app = given && existsSync(given) ? given : makeFixture(...(given ? [given] : [])).app;
const { check, failed } = checker();
run('bun', ['exact.mjs', 'linux'], app);

const pads = [
  { index: 0, id: 'DualSense Wireless Controller', vendor: '054c', product: '0ce6', pressed: [0] },
  { index: 1, id: 'Xbox Wireless Controller', vendor: '045e', product: '0b13' },
];
const env = { ...process.env, EXACT_AGENT_PADS: JSON.stringify(pads) };
const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', ...args], app, env).trim().split('\n').map(l => JSON.parse(l));
const texts = tree => Object.fromEntries((tree.nodes ?? []).filter(n => n.props?.testId).map(n => [n.props.testId, n.props.text ?? '']));

{
  const [, listed, , , buzzed, , , , , stopped, , logs] = drive('--bind', 'gamepad',
    'clock settle', 'tree', 'tap buzz', 'clock +300', 'tree',
    'tap long', 'clock +100', 'tap stop', 'clock settle', 'tree', 'tap amber', 'logs');
  const l = texts(listed), b = texts(buzzed), s = texts(stopped);
  check('bound', 'the registry\'s pads, the pressed one current', l.list === 'true 2 0', l.list);
  check('bound', 'each by its family, with its labels and effects', l.pads === '0:playstation:Square:dual-rumble 1:xbox:X:dual-rumble', l.pads);
  check('bound', 'a rumble on the current pad completes after its span', b.played === 'complete', `${b.played} ${b['played-error']}`);
  check('bound', 'a reset preempts the running rumble', s.played === 'preempted' && s.stopped === 'complete', `${s.played} ${s.stopped}`);
  const lines = JSON.stringify(logs);
  check('bound', 'the motors were asked as the call said', lines.includes('pad 0: rumble 1 0.5 for 200 ms'), lines.slice(0, 600));
  check('bound', 'the reset stopped them', lines.includes('pad 0: motors stopped'), lines.slice(0, 600));
  check('bound', 'a light is refused: no pad here has one', lines.includes('NotSupportedError'), lines.slice(0, 600));
}
{
  const [, tapped, settled, buzzed] = drive('--bind', 'gamepad', 'clock settle', 'tap buzz', 'clock settle', 'tree');
  check('bound', 'clock settle crosses a rumble to its end', settled.settled === true && settled.clock - tapped.clock >= 200 && texts(buzzed).played === 'complete',
    `${JSON.stringify(settled).slice(0, 300)} ${texts(buzzed).played}`);
}
{
  // A claim's lifetime: a pad that goes ends its effect, and the pads stay
  // the process's when the session ends.
  const ops = ['--bind', 'gamepad', 'clock +100', 'tree', 'tap long', 'clock +1500', 'tree'];
  const [, , , , stays] = drive(...ops);
  const leaving = { ...env, EXACT_AGENT_PADS: JSON.stringify([{ ...pads[0], leaves: 1000 }, pads[1]]) };
  const [, b, , , a] = run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', ...ops], app, leaving).trim().split('\n').map(l => JSON.parse(l));
  check('bound', 'a pad that stays keeps its effect running', texts(stays).played === '', texts(stays).played);
  check('bound', 'a pad that goes is listed, then gone', texts(b).list === 'true 2 0' && texts(a).list === 'true 1 none', `${texts(b).list} → ${texts(a).list}`);
  check('bound', 'its going raises gamepaddisconnected', texts(a).went === '0 ', JSON.stringify(texts(a).went));
  check('bound', 'and ends its effect, preempted', texts(a).played === 'preempted', texts(a).played);
}
{
  const [, listed, , , buzzed, logs] = drive('clock settle', 'tree', 'tap buzz', 'clock settle', 'tree', 'logs');
  const l = texts(listed), b = texts(buzzed);
  check('agent', 'unbound, the substitute answers an empty list', l.list === 'true 0 none', l.list);
  check('agent', 'unbound, a send for no pad fails NotFoundError', b['played-error'] === 'NotFoundError', b['played-error']);
  check('agent', 'and nothing reaches the registry', !JSON.stringify(logs).includes('rumble'), JSON.stringify(logs).slice(0, 400));
}
if (!given) rmSync(dirname(app), { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
