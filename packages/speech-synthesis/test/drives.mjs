// LLP 1133.012 §10's Markdown reader drives (Listen) under the agent, on the
// fixture document apps/markdown/assets/listen-fixture.md (three paragraphs of
// twenty words). The implementation's agent path speaks on the agent's clock,
// 170 words a minute at rate 1. The person's drives (the screen locked, a call,
// another app's audio) are test/device.md's.
//
// usage: bun packages/speech-synthesis/test/drives.mjs [web|macos|ios] [--shots <dir>]
//   Builds nothing: build the reader for the host first (`bun host/web/build.mjs
//   markdown`; `bun host/apple/build.mjs markdown-apple`, `--ios --sim <udid>` for a
//   simulator), which the driver checks is current.
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'macos', 'ios'].includes(a)) ?? 'web';
const shots = argv.includes('--shots') ? argv[argv.indexOf('--shots') + 1] : null;
const fixture = resolve(root, 'apps/markdown/assets/listen-fixture.md');
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
let failures = 0;
const check = (n, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${host} ${n} ${name}${ok ? '' : `: ${detail}`}`);
};
const run = Date.now().toString(36);

/** The reader with the fixture open: by the command line on the Mac, else through Open…'s picker. */
async function launch(name) {
  const s = await open({ host, app: 'markdown', storage: `listen-${run}-${name}`, documents: host === 'macos' ? [fixture] : undefined });
  await s.clock('settle');
  if (host !== 'macos') {
    await s.tap('open-document');
    await s.clock('settle');
    const [held] = ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'open-file');
    await s.answer('type', `@${held.ticket}`, fixture);
    await s.clock('settle');
  }
  return s;
}
const read = async (s, path) => (await s.op({ op: 'state' }))[path.split('.')[0]]?.[path.split('.')[1]];
const word = async (s) => {
  const { nodes } = await s.tree();
  return nodes.find((n) => n.props?.testId === 'spoken-word')?.props?.text ?? null;
};
const ids = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));

async function each(n, body) {
  let s;
  try {
    s = await launch(n);
    await body(s);
  } catch (e) {
    check(n, 'ran', false, e.stack ?? e.message);
  } finally {
    await s?.close?.();
  }
}

await each('listen', async (s) => {
  check('listen', 'the fixture is open', (await read(s, 'derives.readable'))?.length === 3, JSON.stringify(await read(s, 'derives.title')));
  await s.tap('listen');
  await s.clock('+1000');
  if (shots) await s.screenshot(resolve(shots, `listen-${host}.png`));
  check('listen', 'the third word of the first paragraph is marked', (await word(s)) === 'paragraph' && (await read(s, 'derives.speakingBlock')) === '0', `${await word(s)} in ${await read(s, 'derives.speakingBlock')}`);
  await s.tap('pause');
  await s.clock('+2000');
  const voice = await read(s, 'resources.voice');
  check('listen', 'paused, the mark has not moved', voice.paused && (await word(s)) === 'paragraph', JSON.stringify(voice));
  await s.tap('pause');
  await s.clock('+22000');
  const last = await read(s, 'resources.last');
  check('listen', 'the last utterance ends', last.outcome === 'end' && last.id === (await read(s, 'slots.spokenIds'))?.[2], JSON.stringify(last));
  check('listen', '"Finished" shows', (await ids(s)).has('listen-finished'), [...(await ids(s))].join(' '));
});

await each('twice', async (s) => {
  await s.tap('listen');
  await s.clock('+500');
  await s.tap('listen');
  await s.tap('listen');
  await s.clock('settle');
  const voice = await read(s, 'resources.voice'), spoken = await read(s, 'slots.spokenIds');
  check('twice', 'only one queue speaks', spoken.length === 3 && voice.utterance === spoken[0] && JSON.stringify(voice.queue) === JSON.stringify(spoken.slice(1)), JSON.stringify({ spoken, voice }));
});

await each('stop', async (s) => {
  await s.tap('listen');
  await s.clock('+1000');
  await s.tap('stop');
  await s.clock('settle');
  const voice = await read(s, 'resources.voice');
  check('stop', 'Stop cancels the queue', !voice.speaking && voice.queue.length === 0 && !(await ids(s)).has('spoken-word'), JSON.stringify(voice));
});

await each('fails', async (s) => {
  // The second paragraph speaks from 7.1 s to 14.1 s: the third, the next to begin, fails.
  await s.tap('listen');
  await s.clock('+10000');
  await s.prefer({ extension: 'speech-synthesis.fail="synthesis-failed"' });
  await s.clock('+20000');
  const last = await read(s, 'resources.last');
  check('fails', 'the error shows', last.error === 'synthesis-failed' && (await ids(s)).has('listen-error'), JSON.stringify(last));
});

console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
