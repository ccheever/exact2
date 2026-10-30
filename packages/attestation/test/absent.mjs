// LLP 1133.028 §8.4 drive 11 for @exact/attestation's fixture app
// (test/fixture), on a target with no implementation: the status reads its
// declared default and data code's `attest` refuses NotSupportedError.
//
// usage: bun packages/attestation/test/absent.mjs <web|linux> [--keep]
import { checker } from '../../camera/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';
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
const fixture = makeFixture('attestation');
try {
  build(fixture, host);
  const s = await launch(fixture, host);
  try {
    await s.clock('settle');
    check(host, 'drive 11: the declared default', (await texts(s)).status === 'Available: false, provider: none', (await texts(s)).status);
    await s.tap('prove');
    const proved = await until(s, 'proved', (v) => !!v);
    check(host, 'drive 11: attest refuses NotSupportedError', proved === 'error NotSupportedError', proved);
  } finally { await s.close(); }
} finally {
  if (!argv.includes('--keep')) fixture.remove();
  else console.log(`kept ${fixture.app}`);
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
