// Runs each scenario of issue #107 in its own session and prints the reply and the probe's lines.
import { spawnSync } from 'node:child_process';
const [app, host = 'macos', out = ''] = process.argv.slice(2);
const scenarios = [
  ['tap probe mouse'],
  ['tap probe dblclick'],
  ['tap probe contextmenu'],
  ['tap probe modifiers Shift'],
  ['tap probe wheel 0 20'],
  ['tap probe wheel 0 20 modifiers Shift'],
  ['tap probe auxclick'],
  ['tap probe clicks 3'],
  ['tap probe wheel 0 20 at 10 10'],
  ['tap probe contextmenu at 10 10'],
  ['tap probe auxclick at 30 40 modifiers Meta'],
  ['tap probe down at 10 20 modifiers Shift', 'tap move 30 20', 'tap up'],
  ['tap probe drag 40 0 from 10 20 mouse modifiers Shift over 32'],
  ['tap probe mouse foo'],
  ['tap probe clicks 4'],
];
for (const ops of scenarios) {
  if (host === 'web' && ops[0] === 'tap probe mouse') continue; // the web's `mouse` is a canvas's
  const r = spawnSync('bun', ['exact.mjs', 'agent', host, '--size', '620x620', '--json', ...ops, host === 'web' ? 'state' : 'logs'], { cwd: app, encoding: 'utf8' });
  console.log(`=== agent ${host} ${ops.map(o => JSON.stringify(o)).join(' ')}`);
  const lines = r.stdout.trim().split('\n').filter(Boolean);
  for (const l of lines.slice(0, -1)) console.log(l);
  const err = r.stderr.trim().split('\n').filter(l => /^op \d/.test(l) || /error/i.test(l)).join('\n');
  if (err) console.log('  REFUSED: ' + err);
  try {
    const logs = JSON.parse(lines.at(-1));
    for (const h of logs.host ?? []) { const m = /probe (view|monitor) .*/.exec(h); if (m) console.log('  ' + m[0]); }
    for (const h of String(logs.slots?.log ?? '').split('\n').filter(Boolean)) console.log('  ' + h);
  } catch {}
}
