// @exact/gamepad's declared substitute on the web JS target (LLP 1133.022 §9):
// the fixture app driven under the agent with no implementation bound, so
// the web runtime's substitute answers as the runner's does natively
// (`contract/cli/tests/it/gamepad.rs`): pads the drive prefers, filled with
// the family's labels and raising `gamepadconnected` and
// `gamepaddisconnected`; a rumble on the current pad answered after its
// span on the agent clock, which `clock settle` crosses; a reset preempting
// it; a light kept by pad; and no current pad refused.
//
// usage: bun packages/gamepad/test/web-agent.mjs [<app dir>]
import { rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { checker, makeFixture, run } from './fixture.mjs';

const given = process.argv[2] && resolve(process.argv[2]);
const { app } = given ? { app: given } : makeFixture();
const { check, failed } = checker();
const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'web', '--json', ...args], app).trim().split('\n').map(l => JSON.parse(l));
const texts = tree => Object.fromEntries((tree.nodes ?? []).filter(n => n.props?.testId).map(n => [n.props.testId, n.props.text ?? '']));
const pads = JSON.stringify({ gamepads: [{ index: 0, family: 'playstation' }, { index: 1, family: 'xbox' }], current: 0 });

const [, empty, , , listed, , , buzzing, , buzzed, , , , , stopped, , lit, , , gone, , , refused] = drive(
  'clock settle', 'tree',
  `prefer extension gamepad.getGamepads=${pads}`, 'clock settle', 'tree',
  'tap buzz', 'clock +100', 'state', 'clock settle', 'tree',
  'tap long', 'clock +100', 'tap stop', 'clock settle', 'tree',
  'tap amber', 'state',
  `prefer extension gamepad.getGamepads=${JSON.stringify({ gamepads: [{ index: 1, family: 'xbox' }], current: null })}`, 'clock settle', 'tree',
  'tap buzz', 'clock settle', 'tree');

check('substitute', 'no pad by default', texts(empty).list === 'true 0 none', texts(empty).list);
const l = texts(listed);
check('substitute', 'preferred pads, the current one', l.list === 'true 2 0', l.list);
check('substitute', 'filled with each family\'s labels and effects', l.pads === '0:playstation:Square:dual-rumble 1:xbox:X:dual-rumble', l.pads);
check('substitute', 'each raises gamepadconnected', l.came === '0:playstation 1:xbox ', l.came);
const effects = buzzing.extensions?.gamepad?.effects ?? [];
check('substitute', 'a rumble runs for its span, on the current pad', effects.length === 1 && effects[0].index === 0 && effects[0].result === null && effects[0].origin === 'app', JSON.stringify(effects));
check('substitute', 'and completes when the clock crosses it', texts(buzzed).played === 'complete', JSON.stringify(texts(buzzed)));
const s = texts(stopped);
check('substitute', 'a reset preempts the running rumble', s.played === 'preempted' && s.stopped === 'complete', `${s.played} ${s.stopped}`);
const lights = lit.extensions?.gamepad?.lights ?? [];
check('substitute', 'a light is kept by pad', lights.length === 1 && lights[0].index === 0 && lights[0].color === '#e8ad61', JSON.stringify(lights));
check('substitute', 'a pad gone raises gamepaddisconnected', texts(gone).went === '0 ', texts(gone).went);
check('substitute', 'with no current pad a rumble fails NotFoundError', texts(refused)['played-error'] === 'NotFoundError', JSON.stringify(texts(refused)));

if (!given) rmSync(dirname(app), { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
