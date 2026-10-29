// The transcript's list keeps the first and last message mounted, a row whose sound plays and a full-screen app's row
// (MessagesTimeline alwaysRenderKeys, 6838e31b9c), each once.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { keptTranscriptRows, noteAudioPlaying } from '../../src/timeline/timeline-kept-rows';
import { chatLocal } from '../../src/timeline/timeline-presentation';
import type { Native } from '../../src/client/protocol';

const row = (id: string, kind: string, extra: object = {}) => ({ id, kind, ...extra });
const opened = (threadId = 't1') => Object.assign(new T3Client(), { threadId });

describe('the rows the transcript keeps mounted', () => {
  test('the first and last message, past the rows that are not messages', () => {
    const rows = [row('status', 'status'), row('u1', 'user'), row('w1', 'work'), row('a1', 'assistant'), row('u2', 'user'), row('a2', 'assistant'), row('end', 'thread-now')];
    expect(keptTranscriptRows(opened(), rows)).toEqual(['u1', 'a2']);
    // A workflow's opening stands for its user message; one message is kept once.
    expect(keptTranscriptRows(opened(), [row('o', 'workflow'), row('w', 'work')])).toEqual(['o']);
    expect(keptTranscriptRows(opened(), [row('w', 'work'), row('c', 'checkpoint')])).toEqual([]);
  });

  test('a row whose sound plays, until every sound in it stops', () => {
    const client = opened(), rows = [row('u1', 'user'), row('a1', 'assistant'), row('u2', 'user'), row('a2', 'assistant')];
    noteAudioPlaying(client, 'u2', '1memo.wav');
    noteAudioPlaying(client, 'u2', '1note.wav');
    expect(keptTranscriptRows(client, rows)).toEqual(['u1', 'a2', 'u2']);
    noteAudioPlaying(client, 'u2', '0memo.wav');
    expect(keptTranscriptRows(client, rows)).toEqual(['u1', 'a2', 'u2']);
    noteAudioPlaying(client, 'u2', '0note.wav');
    expect(keptTranscriptRows(client, rows)).toEqual(['u1', 'a2']);
    // A row no longer in the transcript names nothing; another thread starts with no sound playing.
    noteAudioPlaying(client, 'gone', '1memo.wav');
    expect(keptTranscriptRows(client, rows)).toEqual(['u1', 'a2']);
    noteAudioPlaying(client, 'a1', '1memo.wav');
    client.threadId = 't2';
    expect(keptTranscriptRows(client, rows)).toEqual(['u1', 'a2']);
  });

  test('an app shown full screen keeps its row', () => {
    const rows = [row('u1', 'user'), row('doc', 'inline-document', { inlineDocuments: [{ fullscreen: true }] }), row('doc2', 'inline-document', { inlineDocuments: [{ fullscreen: false }] }), row('a1', 'assistant')];
    expect(keptTranscriptRows(opened(), rows)).toEqual(['u1', 'a1', 'doc']);
  });

  test('the audio chip reports through chatLocal', async () => {
    const client = opened(), native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    await chatLocal(client, native, 'audio-playing', 'u2', '1memo.wav');
    expect(keptTranscriptRows(client, [row('u1', 'user'), row('u2', 'user'), row('a1', 'assistant')])).toEqual(['u1', 'a1', 'u2']);
  });
});
