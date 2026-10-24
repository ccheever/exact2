import { test, expect } from 'bun:test';
import { implementation } from '../web/index.js';

// The web implementation's agent path against a context of fakes (LLP 1133.012
// §9): the session clock is `clock.now`, the journal `lines`, and the store a map.
function context() {
  const clock = { now: 0 }, lines = [], files = new Map(), held = [];
  const ctx = {
    agent: true, now: () => clock.now, log: (l) => lines.push(l), changed: () => {},
    tracks: {
      lookup: () => null, subscribe: () => null,
      hold: (kind) => { if (held.includes(kind)) throw Object.assign(new Error('held'), { code: 'NotReadableError' }); held.push(kind); return () => held.splice(held.indexOf(kind), 1); },
    },
    files: {
      read: async (p) => files.get(p), write: async (p, b) => { files.set(p, b); }, remove: async (p) => { for (const k of [...files.keys()]) if (k.startsWith(p)) files.delete(k); },
      mkdir: async () => {}, list: async (p) => [...files.keys()].filter((k) => k.startsWith(`${p}/`)).map((k) => k.slice(p.length + 1)),
    },
  };
  return { ctx, clock, lines, files, held };
}

test('a recognition whose reply was dropped is aborted, and its hold released', async () => {
  const { ctx, lines, held } = context(), impl = implementation(ctx);
  const info = await impl.startSpeechRecognition({ microphone: true, continuous: true });
  expect(held).toEqual(['audio']);
  expect(await impl.startSpeechRecognition({ microphone: true })).toEqual({ error: 'NotReadableError' });
  impl.dispose([info.id]);
  const state = await impl.speechRecognitionState({ recognition: info.id });
  expect(state).toMatchObject({ state: 'ended', error: 'aborted' });
  expect(lines.some((l) => l.includes(`${info.id}: its reply was dropped; aborted`))).toBe(true);
  expect(held).toEqual([]);
});

test('a phrase arrives a word at a time, its final appended to the transcript', async () => {
  const { ctx, clock, files } = context(), impl = implementation(ctx);
  const info = await impl.startSpeechRecognition({ microphone: true, interimResults: true, transcript: 'app:/data/t.transcript' });
  impl.preferred({ heard: 'pick up milk' });
  // A word every 300 ms, the final 500 ms after the last.
  clock.now = 650;
  const interim = await impl.speechRecognitionState({ recognition: info.id });
  expect(interim.interim.alternatives[0].transcript).toBe('pick up');
  expect(interim.results).toEqual([]);
  clock.now = 1350;
  expect((await impl.speechRecognitionState({ recognition: info.id })).finalCount).toBe(0);
  clock.now = 1400;
  const done = await impl.speechRecognitionState({ recognition: info.id });
  expect(done.state).toBe('ended');
  expect(done.finalCount).toBe(1);
  const stopped = await impl.stopSpeechRecognition({ recognition: info.id });
  expect(stopped).toMatchObject({ finalCount: 1, files: 1, error: null });
  const line = JSON.parse(new TextDecoder().decode(files.get('app:/data/t.transcript/00000.jsonl')).trim());
  expect(line.alternatives[0].transcript).toBe('pick up milk');
});

test('processLocally where the recognizer cannot keep audio on the device fails service-not-allowed', async () => {
  const { ctx, held } = context(), impl = implementation(ctx);
  impl.preferred({ speechRecognitionStatus: { local: false } });
  const info = await impl.startSpeechRecognition({ microphone: true, processLocally: true });
  expect(info.error).toBe('service-not-allowed');
  expect(held).toEqual([]);
});
