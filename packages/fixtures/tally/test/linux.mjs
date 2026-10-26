// @fixture/tally on the Linux host, headless (LLP 1133.022 D10 item 5): the
// fixture app made outside the repo, its Linux crate built with the
// package's Rust implementation composed by the build, and driven.
//
// - Bound (`--bind tally`), the implementation answers: its status names
//   the OS it was compiled for, a send adds to the count the session's
//   instance keeps and answers the call's origin, its emitted `tallied`
//   reaches the root's handler, and its announcement asks the status again.
// - Under the agent unbound, the declared substitute answers the status and
//   the send fails `NotSupportedError`: the implementation is not called.
//
// usage: bun packages/fixtures/tally/test/linux.mjs [<app dir>]
//   (an app already made by fixture.mjs is reused, and built again)
import { existsSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { checker, makeFixture, run } from './fixture.mjs';

const given = process.argv[2] && resolve(process.argv[2]);
const app = given && existsSync(given) ? given : makeFixture(...(given ? [given] : []));
const { check, failed } = checker();
run('bun', ['exact.mjs', 'linux'], app);

const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', ...args], app).trim().split('\n').map(l => JSON.parse(l));
const texts = (tree) => Object.fromEntries((tree.nodes ?? []).filter(n => n.props?.testId).map(n => [n.props.testId, n.props.text ?? '']));
const os = { darwin: 'macos', linux: 'linux', win32: 'windows' }[process.platform];

{
  const [, before, , , after, logs] = drive('--bind', 'tally', 'clock settle', 'tree', 'tap add', 'clock settle', 'tree', 'logs');
  const b = texts(before), a = texts(after);
  check('bound', 'the implementation answers its status', b.status === `true ${os} 0 true`, b.status);
  check('bound', 'a send is answered by the session\'s instance, with the app\'s origin', a.added === '2 app', a.added);
  check('bound', 'its emitted event reaches the root\'s handler', a.heard === '2', a.heard);
  check('bound', 'its announcement asks the status again', a.status === `true ${os} 2 true`, a.status);
  const lines = JSON.stringify(logs);
  check('bound', 'the runner bound it here, and the instance was made once', lines.includes('modules bound here: tally') && lines.split('tally: made').length === 2, lines.slice(0, 400));
}
{
  const [, before, , , after] = drive('clock settle', 'tree', 'tap add', 'clock settle', 'tree');
  const b = texts(before), a = texts(after);
  check('agent', 'unbound, the substitute answers the status', b.status === 'false substitute 0 true', b.status);
  check('agent', 'unbound, the send is not the implementation\'s', a.added === 'NotSupportedError', a.added);
}
if (!given) rmSync(dirname(app), { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
