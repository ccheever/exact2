// The transcript's list keeps the first and last message mounted, a row whose sound plays and a full-screen app's row
// (MessagesTimeline alwaysRenderKeys, 6838e31b9c), each once.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { keptTranscriptRows, noteAudioPlaying } from '../../src/timeline/timeline-kept-rows';
import { chatLocal } from '../../src/timeline/timeline-presentation';
import type { Native } from '../../src/client/protocol';
import { snapshot } from '../../src/client/presentation';
import { appText } from '../support/app-files';

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
    const sounds = { audio: [{ id: 'memo.wav' }, { id: 'note.wav' }] };
    const client = opened(), rows = [row('u1', 'user'), row('a1', 'assistant', sounds), row('u2', 'user', sounds), row('a2', 'assistant')];
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

  test('the snapshot names the rows, and the transcript\'s list keeps what it names', () => {
    const thread = { projection: { visibleTurnItems: ['u1', 'a1', 'u2', 'a2'].map(id => ({ sourceThreadId: 't1', sourceItemId: id,
      item: { id, messageId: id, type: id.startsWith('u') ? 'user_message' : 'assistant_message', text: `text ${id}` } })) }, hasMore: false, historyCursor: null };
    const client = Object.assign(new T3Client(), { threadId: 't1', connection: 'connected', thread }) as T3Client;
    expect((snapshot(client) as { transcriptKeep?: string[] }).transcriptKeep).toEqual(['["t1","u1"]', '["t1","a2"]']);
    const list = appText('src/shell/app-main.contract').split('\n').filter(line => /^\s*list .*virtualized=true/.test(line) && line.includes('scroll=transcriptScrolled'));
    expect(list).toHaveLength(1);
    expect(list[0]).toContain(' keep-mounted=data.transcriptKeep ');
  });

  test('the audio chip reports through chatLocal', async () => {
    const client = opened(), native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    await chatLocal(client, native, 'audio-playing', 'u2', '1memo.wav');
    expect(keptTranscriptRows(client, [row('u1', 'user'), row('u2', 'user', { audio: [{ id: 'memo.wav' }] }), row('a1', 'assistant')])).toEqual(['u1', 'a1', 'u2']);
  });

  test('a sound whose chip is no longer drawn stops keeping its row, as an unmounted chip does (onAudioUnmount)', () => {
    const client = opened(), first = row('u1', 'user'), last = row('a9', 'assistant');
    const sound = { audio: [{ id: 'memo.wav' }] }, markdown = { md: { chips: [{ kind: 'media', size: 'audio' }] } };
    const keep = (...middle: object[]) => keptTranscriptRows(client, [first, ...middle as ReturnType<typeof row>[], last]);
    // A folded agent message draws no body: its playing chip went with it, and stays gone once the message opens again.
    noteAudioPlaying(client, 'agent', '1memo.wav');
    expect(keep(row('agent', 'user', { ...sound, attribution: 'agent', expanded: true }))).toEqual(['u1', 'a9', 'agent']);
    expect(keep(row('agent', 'user', { ...sound, attribution: 'agent', expanded: false }))).toEqual(['u1', 'a9']);
    expect(keep(row('agent', 'user', { ...sound, attribution: 'agent', expanded: true }))).toEqual(['u1', 'a9']);
    // An attachment chip by its id; a Markdown sound's node while the row's Markdown holds a sound.
    noteAudioPlaying(client, 'u5', '1memo.wav');
    noteAudioPlaying(client, 'a5', '1media-a5-b2-r1');
    expect(keep(row('u5', 'user', sound), row('a5', 'assistant', markdown))).toEqual(['u1', 'a9', 'u5', 'a5']);
    expect(keep(row('u5', 'user', { audio: [] }), row('a5', 'assistant', { md: { chips: [] } }))).toEqual(['u1', 'a9']);
    // A row that left the transcript (a rewind, a folded turn) forgets its sounds.
    noteAudioPlaying(client, 'u6', '1memo.wav');
    expect(keep()).toEqual(['u1', 'a9']);
    expect(keep(row('u6', 'user', sound))).toEqual(['u1', 'a9']);
  });
});
