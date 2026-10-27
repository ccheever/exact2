// @exact/gamepad on macOS (LLP 1133.022 §10, D4, D10 item 1): the fixture app
// made outside the repo, built with the package's Swift, and driven bound
// (`--bind gamepad`) with a drive's pads (`EXACT_AGENT_PADS`): snapshot
// controllers in the process's slot table, whose motors and light are a
// recorder, the real path from the registry on. No pad attached to the Mac
// is read or moved under the agent.
//
// - The list is the registry's, by index, each pad's family by its category
//   or vendor, with the family's labels; the pad pressed is the current one.
// - A rumble reaches the pad's motors and completes after its span on the
//   session's clock, which `clock settle` crosses; a reset preempts a
//   running one; a light reaches the pad, and `setLight()` gives it back.
//   A pad that disconnects ends the effect playing on it, `preempted`, and
//   raises `gamepaddisconnected`.
// - Under the agent unbound, the declared substitute answers, and nothing
//   reaches the registry.
//
// usage: bun packages/gamepad/test/apple.mjs [<app dir>]
//   (an app already made by fixture.mjs is reused, and built again)
import { existsSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { checker, makeFixture, run } from './fixture.mjs';

const given = process.argv[2] && resolve(process.argv[2]);
const app = given && existsSync(given) ? given : makeFixture(...(given ? [given] : [])).app;
const { check, failed } = checker();
run('bun', ['exact.mjs', 'mac'], app);

const pads = [
  { index: 0, id: 'DualSense Wireless Controller', category: 'DualSense', pressed: [0] },
  { index: 1, id: 'Xbox Wireless Controller', vendor: '045e' },
];
const env = { ...process.env, EXACT_AGENT_PADS: JSON.stringify(pads) };
const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'macos', '--reset-storage', '--json', ...args], app, env).trim().split('\n').map(l => JSON.parse(l));
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
  check('bound', 'the light reached the pad', lines.includes('pad 0: light #e8ad61'), lines.slice(0, 600));
}
{
  const [, tapped, settled, buzzed] = drive('--bind', 'gamepad', 'clock settle', 'tap buzz', 'clock settle', 'tree');
  check('bound', 'clock settle crosses a rumble to its end', settled.settled === true && settled.clock - tapped.clock >= 200 && texts(buzzed).played === 'complete',
    `${JSON.stringify(settled).slice(0, 300)} ${texts(buzzed).played}`);
}
{
  // A claim's lifetime: a pad that goes ends the effect playing on it. The
  // list's new answer is a request on real time, which `clock data` lands.
  const ops = ['--bind', 'gamepad', 'clock +100', 'tree', 'tap long', 'clock +1500', 'clock data', 'tree'];
  const [, , , , , stays] = drive(...ops);
  const leaving = { ...env, EXACT_AGENT_PADS: JSON.stringify([{ ...pads[0], leaves: 1000 }, pads[1]]) };
  const [, b, , , , a] = run('bun', ['exact.mjs', 'agent', 'macos', '--reset-storage', '--json', ...ops], app, leaving).trim().split('\n').map(l => JSON.parse(l));
  check('bound', 'a pad that stays keeps its effect running', texts(stays).played === '', texts(stays).played);
  check('bound', 'a pad that goes is listed, then gone', texts(b).list === 'true 2 0' && texts(a).list === 'true 1 none', `${texts(b).list} → ${texts(a).list}`);
  check('bound', 'its going raises gamepaddisconnected', texts(a).went === '0 ', JSON.stringify(texts(a).went));
  check('bound', 'and ends its effect, preempted', texts(a).played === 'preempted', texts(a).played);
}
{
  // A light is a claim: `setLight()` gives back what this session set.
  const [, , , logs] = drive('--bind', 'gamepad', 'clock +100', 'tap amber', 'tap unlit', 'logs');
  const lines = JSON.stringify(logs);
  check('bound', 'setLight() gives the light back', lines.includes('pad 0: light #e8ad61') && lines.includes('pad 0: light given back'), lines.slice(0, 600));
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
