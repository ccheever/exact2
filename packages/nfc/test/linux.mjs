// @exact/nfc on the Linux host, headless (LLP 1133.035 §12 stage 0, D18):
// the fixture app made outside the repo, its Linux crate built and driven
// under the agent. Linux has no cell, so the status reads `available` false
// and every request fails `NotSupportedError`, as in production.
//
// usage: bun packages/nfc/test/linux.mjs
import { dirname } from 'node:path';
import { rmSync } from 'node:fs';
import { checker, makeFixture, run } from '../../contacts/test/fixture.mjs';

const { check, failed } = checker();
const { app } = makeFixture('nfc');
try {
  run('bun', ['exact.mjs', 'linux'], app);
  const replies = run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json',
    'clock settle', 'tap scan', 'tap write', 'clock settle', 'tree'], app)
    .trim().split('\n').map(l => JSON.parse(l));
  const tree = replies.at(-1);
  const t = Object.fromEntries((tree.nodes ?? []).filter(n => n.props?.testId).map(n => [n.props.testId, n.props.text ?? '']));
  check('linux', 'absent: the status reads unavailable', t.status?.startsWith('false'), t.status);
  check('linux', 'a scan fails NotSupportedError', t['scan-error']?.startsWith('NotSupportedError'), t['scan-error']);
  check('linux', 'a write fails NotSupportedError', t['write-error']?.startsWith('NotSupportedError'), t['write-error']);
} finally {
  rmSync(dirname(app), { recursive: true, force: true });
}
process.exit(failed() ? 1 : 0);
