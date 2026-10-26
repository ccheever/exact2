// @exact/translation's suite under the agent (LLP 1133.027 §9, §12 stage 3),
// the same on each host named: the fixed language list, the detection rule,
// a download the drive completes, the translation script with a removed
// language (drive 8's module half), and on Apple the download sheet declined.
// No platform translator or detector is asked.
//
// usage: bun packages/translation/test/agent.mjs [web] [macos] [ios] [--sim <udid>]
import { spawnSync } from 'node:child_process';
import { checker, discard, makeFixture, run, shown } from '../../barcode-detection/test/fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['web', 'macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { check, failed } = checker();
const { app } = makeFixture();
const env = { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };
const avail = v => `prefer extension translation.translatorAvailability=${JSON.stringify(v)}`;

for (const host of hosts.length ? hosts : ['web']) {
  const apple = host !== 'web';
  if (host === 'macos') run('bun', ['exact.mjs', 'mac'], app, env);
  if (host === 'ios') run('bun', ['exact.mjs', 'ios', ...(sim ? ['--sim', sim] : [])], app, env);
  const ops = [...(host === 'ios' && sim ? ['--sim', sim] : []), 'clock data', 'tree pair', 'tree identifier',
    'tap create', 'clock data', 'tree translator', 'tap translate', 'clock data', 'tree translated',
    'tap create-detector', 'clock data', 'tap identify', 'clock data', 'tree language', 'tap say-ru', 'tap identify', 'clock data', 'tree language',
    'tap say-ko', 'tap identify', 'clock data', 'tree language', 'state detected', 'tap say-empty', 'tap identify', 'clock data', 'tree language',
    'tap pair-same', 'clock data', 'tree pair', 'tap create', 'clock data', 'tree translator-error',
    'tap pair-zh-tw', 'clock data', 'tree pair', 'tap pair-unknown', 'clock data', 'tree pair',
    avail({ availability: 'downloadable' }), 'tap pair-ja-en', 'clock data', 'tree pair', 'tap create', 'clock data', 'tree pair',
    avail({ availability: 'available' }), 'clock data', 'tree translator', 'tree pair', avail({}),
    'prefer extension translation.translations=[{"text":"The meeting tomorrow is at ten."},{"error":"InvalidStateError","reason":"not-installed"}]',
    'tap say-ru', 'tap translate', 'clock data', 'tree translated', 'tap translate', 'clock data', 'tree translated-error', 'clock data', 'tree pair',
    'tap destroy', 'clock data', 'tap translate', 'clock data', 'tree translated-error'];
  // Apple's download is the system's sheet; the drive answers it (§9).
  if (apple) ops.push('prefer extension translation.downloadSheet="decline"', avail({ availability: 'downloadable' }), 'tap create', 'clock data', 'tree translator-error', 'state translator');
  ops.push('state extensions.translation', 'logs');
  const r = spawnSync('bun', ['exact.mjs', 'agent', host, ...ops], { cwd: app, env, encoding: 'utf8', maxBuffer: 64 << 20 });
  const out = `${r.stdout}\n${r.stderr}`, pair = shown(out, 'pair'), langs = shown(out, 'language');
  check(host, 'the drive ran', r.status === 0, out.split('\n').slice(-20).join('\n'));
  check(host, 'a listed pair and the detector are available', pair[0] === 'available - -1' && shown(out, 'identifier')[0] === 'available -', `${pair[0]} ${shown(out, 'identifier')}`);
  check(host, 'a create answers a translator', shown(out, 'translator')[0] === 'translator-1 ja en -', shown(out, 'translator')[0]);
  check(host, 'an unscripted translation is the input behind the target\'s tag', shown(out, 'translated')[0] === '[en] 明日の会議は十時からです。', shown(out, 'translated')[0]);
  check(host, 'detection by script: ja (kana after Han), ru, ko, und', JSON.stringify(langs) === '["ja 0.9","ru 0.9","ko 0.9","und 1"]' && out.includes('results [{"confidence":0.9,"detectedLanguage":"ko"},{"confidence":0.1,"detectedLanguage":"und"}]'), JSON.stringify(langs));
  check(host, 'en to en-GB is one language: unavailable, unsupported-options, and a create fails', pair[1] === 'unavailable unsupported-options -1' && shown(out, 'translator-error')[0] === 'NotSupportedError', `${pair[1]} ${shown(out, 'translator-error')}`);
  check(host, 'zh-TW is zh-Hant, a listed language; an unknown one is unavailable', pair[2] === 'available - -1' && pair[3] === 'unavailable unsupported-options -1', `${pair[2]} ${pair[3]}`);
  check(host, 'a create while downloadable downloads until the drive says available', pair[4] === 'downloadable - -1' && pair[5] === 'downloading - 0' && shown(out, 'translator')[1] === 'translator-3 ja en -' && pair[6] === 'available - -1', `${pair.slice(4, 7)} ${shown(out, 'translator')}`);
  check(host, 'the script answers in order', shown(out, 'translated').at(-1) === 'The meeting tomorrow is at ten.', shown(out, 'translated').at(-1));
  check(host, 'a not-installed entry fails and announces the pair downloadable (drive 8)', shown(out, 'translated-error')[0] === 'InvalidStateError' && pair[7] === 'downloadable - -1', `${shown(out, 'translated-error')} ${pair[7]}`);
  check(host, 'a destroyed translator fails InvalidStateError', shown(out, 'translated-error').at(-1) === 'InvalidStateError' && /translator-3: destroyed by the app/.test(out), shown(out, 'translated-error').join());
  // The web's implementation reports its state into `state.extensions`; Apple's say what they did in the journal.
  if (!apple) check(host, 'state discloses each answer\'s origin', /"answered":\["createTranslator \(app\)"/.test(out), out.match(/"answered":[^\]]*\]/)?.[0]);
  if (apple) check(host, 'a declined download sheet fails the create NotAllowedError, declined', shown(out, 'translator-error').at(-1) === 'NotAllowedError' && /the download sheet was declined/.test(out), `${shown(out, 'translator-error')}`);
}
discard(app);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
