// LLP 1133.028 §8.4 drive 11 for @exact/age-range's fixture app
// (test/fixture), on a target with no implementation: the status reads its
// declared default and the request refuses NotSupportedError before any
// ticket. Excord's part of drive 11 is drives.mjs's.
//
// usage: bun packages/age-range/test/absent.mjs <web|linux> [--keep]
import { checker } from '../../camera/test/fixture.mjs';
import { texts } from '../../photos/test/fixture.mjs';
import { build, launch, makeFixture } from '../../tracking-consent/test/fixture.mjs';

const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'linux', 'windows'].includes(a)) ?? 'web';
const { check, failed } = checker();
// The Linux host built on a Mac is a macOS binary, and the runner reads a
// declaration's per-target words by the target it was built for, so there it
// answers macOS's: this drive's Linux answer needs a Linux machine.
if (host === 'linux' && process.platform !== 'linux') {
  console.error('linux: the Linux host on this machine answers by its own target, not linux; run this drive on Linux');
  process.exit(2);
}
const fixture = makeFixture('age-range');
try {
  build(fixture, host);
  const s = await launch(fixture, host);
  try {
    await s.clock('settle');
    check(host, 'drive 11: the declared default', (await texts(s)).status === 'Available: false', (await texts(s)).status);
    await s.tap('ask');
    await s.clock('settle');
    const st = await s.state();
    check(host, 'drive 11: no ticket', !(st.pending ?? []).some((p) => p.device), JSON.stringify(st.pending));
    const lines = (await s.logs({ all: true })).lines.join('\n');
    check(host, 'drive 11: the request refused NotSupportedError', /requestAgeRange[^\n]*NotSupportedError/.test(lines) && (await texts(s)).age === '',
      lines.split('\n').filter((l) => /age/i.test(l)).slice(-4).join('\n'));
  } finally { await s.close(); }
} finally {
  if (!argv.includes('--keep')) fixture.remove();
  else console.log(`kept ${fixture.app}`);
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
