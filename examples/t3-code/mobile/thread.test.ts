import { mobileMarkdownBlocks } from './markdown-blocks';
import { mobileThreadMarkdownBlocks } from './thread-markdown';
import { queuedEditState, type MobileQueuedEditSession } from './queued-edit-memory';
import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, mergeHistory, obj, type Obj } from './shared/domain';
import { chatLocal, transcriptRows } from './shared/timeline-presentation';
import { timelineEntries } from './shared/timeline-rows';
import type { Native } from './shared/protocol';
import { mobileMessageTime, mobileThread, mobileThreadRows, mobileThreadComposer, mobileThreadAnswerFilesPrepare } from './thread';
import { mobileCodeTokens } from './thread-highlight';
import { mobileModelSelectionReady, mobileModelSelectionUnavailable } from './model-availability';
import { htmlRenderFileName, htmlRenderFrameHeight, htmlRenderFromToolItem, htmlRenderReferencesEqual, readHtmlRenderReference } from './html-render';
import { mobileMessageCopyCommand } from './client';
import { letGoAware } from './shared/let-go';
import { timelineView } from './shared/timeline-presentation';
import type { Files } from './shared/protocol';
import { mobileOutboxRead, mobileOutboxEnqueue } from './mobile-outbox';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { appText } from '../tests/support/app-files';

const now = Date.parse('2026-10-07T12:00:00Z');
const at = (seconds: number) => new Date(now + seconds * 1000).toISOString();
const item = (id: string, type: string, extra: Obj = {}): Obj => ({ id, type, threadId: 't1', runId: 'r1', nodeId: null,
  ordinal: 1, status: 'completed', startedAt: at(0), updatedAt: at(2), ...extra });
const user = (text: string) => item('user', 'user_message', { text, messageId: 'mu', inputIntent: 'turn_start', attachments: [], createdBy: 'user' });
const answer = (text: string) => item('answer', 'assistant_message', { text, messageId: 'ma', streaming: false });
function fixture(items: Obj[] = [], projection: Obj = {}): T3Client {
  const client = new T3Client();
  client.environmentId = 'env'; client.threadId = 't1'; client.projectId = 'p1';
  client.connection = 'connected'; client.configLive = true; client.shellLive = true; client.threadLive = true;
  client.scopes = ['orchestration:operate']; client.providerId = 'provider'; client.modelId = 'model';
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'provider', driver: 'codex', enabled: true, installed: true, models: [{ slug: 'model', name: 'Model' }] }] };
  client.shell.projects = [{ id: 'p1', workspaceRoot: '/repo' }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 't1', projectId: 'p1', title: 'Actual title' }, runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(10) }],
    attempts: [], nodes: [], checkpoints: [], turnItems: items, runtimeRequests: [], ...projection,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) } };
  return client;
}

describe('bounded transcript Markdown reuse', () => {
  test('thread Markdown keeps prose and code selectable like the reference reader', () => {
    const source = appText('mobile/thread-markdown.contract');
    expect(source).toContain('text token.text user-select="text"');
    expect(source).toContain('text block.text markup="markdown" user-select="text"');
  });

  test('thread Markdown tables expose table and header or body cell roles', () => {
    const source = appText('mobile/thread-markdown.contract');
    expect(source).toContain('view display="grid" role="table"');
    expect(source).toContain('view role=(item.header ? "columnheader" : "cell")');
  });

  test('completed Duo table response presents cells and alignment without changing Copy or adjacent code', () => {
    const code = '"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwx"';
    const body = ['T3-CONVERSATION-QA-04', '', '| Plant | Count | Note |', '|---|---:|---|',
      '| Moonbell | 7 | Chimes at dusk |', '| Cloudfern | 3 | Collects floating dew |',
      '| Starroot | 9 | Glows underground |', '', '```typescript', code, '```', '', 'END-04'].join('\n');
    const [row] = mobileThreadRows(fixture([answer(body)]), now);
    expect(row!.blocks.map(block => block.kind)).toEqual(['markdown', 'table', 'code', 'markdown']);
    const table = row!.blocks[1]!;
    expect(table.columns).toBe(3);
    expect(table.alignments).toEqual(['left', 'right', 'left']);
    expect(table.rows.map(row => row.cells.map(cell => cell.runs.map(run => run.text).join('')))).toEqual([
      ['Plant', 'Count', 'Note'], ['Moonbell', '7', 'Chimes at dusk'],
      ['Cloudfern', '3', 'Collects floating dew'], ['Starroot', '9', 'Glows underground'],
    ]);
    expect(table.rows.map(row => row.header)).toEqual([true, false, false, false]);
    expect(row!.blocks.filter(block => block.kind === 'markdown').map(block => block.text))
      .toEqual(['T3-CONVERSATION-QA-04\n\n', '\nEND-04']);
    expect(row!.blocks[2]!.text).toBe(code);
    expect(row!.blocks[2]!.tokens.map(token => token.text).join('')).toBe(code);
    expect(code.slice(1, -1)).toHaveLength(180);
    expect(row!.body).toBe(body);
    expect(JSON.parse(row!.copyConfiguration).text).toBe(body);
  });

  test('table promotion and edits preserve source identity and prior presentation snapshots', () => {
    const response = answer('😀 Intro\n\n| Name | Count |\n| --- | :');
    response.streaming = true;
    const client = fixture([response]);
    const pending = mobileThreadRows(client, now);
    expect(pending[0]!.blocks.map(block => block.kind)).toEqual(['markdown']);
    response.text = '😀 Intro\n\n| Name | Count |\n| --- | --: |\n| **Rose** | `7` |';
    const first = mobileThreadRows(client, now), old = structuredClone(first);
    const table = first[0]!.blocks.find(block => block.kind === 'table')!;
    expect(table.id).toBe('0:table:10');
    expect(table.rows[1]!.cells.map(cell => cell.runs)).toEqual([
      [{ id: '1:0:0', kind: 'strong', text: 'Rose' }], [{ id: '1:1:0', kind: 'code', text: '7' }],
    ]);
    response.text += '\n| Fern | 3 |';
    const next = mobileThreadRows(client, now);
    expect(next[0]!.id).toBe(first[0]!.id);
    expect(next[0]!.blocks[1]!.id).toBe(table.id);
    expect(next[0]!.blocks[1]!.rows).toHaveLength(3);
    expect(first).toEqual(old);
    expect(pending[0]!.blocks[0]!.text).toBe('😀 Intro\n\n| Name | Count |\n| --- | :');
    expect(JSON.parse(next[0]!.copyConfiguration).text).toBe(response.text);
    expect(mobileThreadRows(client, now)[0]!.blocks).toBe(next[0]!.blocks);
  });

  test('table projection preserves surrounding Markdown and falls back completely for unsupported cells or fences', () => {
    const table = '| A | B |\n| :---: | ---: |\n| a \\| b | 7 |';
    const source = `# Heading\n\nOne\nline.\n\n${table}\n\nTail\nline.`;
    const blocks = mobileThreadMarkdownBlocks(source, false);
    expect(blocks.map(block => block.kind)).toEqual(['markdown', 'table', 'markdown']);
    expect(blocks.map(block => block.text).join('')).toBe(source);
    expect(blocks[1]!.alignments).toEqual(['center', 'right']);
    expect(blocks[1]!.rows[1]!.cells[0]!.runs[0]!.text).toBe('a | b');
    for (const body of [
      `\x60\x60\x60text\n${table}\n\x60\x60\x60`, `\x60\x60\x60text\n${table}`, `~~~\n${table}`,
      '| A |\n| --- |\n| [link](https://example.test) |', '| A |\n| --- |\n| **bold *nested*** |',
    ]) {
      const actual = mobileThreadMarkdownBlocks(body, false);
      expect(actual.some(block => block.kind === 'table')).toBe(false);
      expect(actual.map(({ columns, alignments, rows, ...block }) => block)).toEqual(mobileMarkdownBlocks(body, false));
    }
    expect(mobileThreadMarkdownBlocks('Name\n:---\nValue', false)[0]).toMatchObject({ kind: 'table', columns: 1, alignments: ['left'] });
    const crlf = mobileThreadMarkdownBlocks(source.replaceAll('\n', '\r\n'), false);
    expect(crlf).toEqual(blocks);
  });

  test('table row cell and run budgets keep dense content complete without evicting small reusable blocks', () => {
    const body = '| A | B | C | D | E | F | G | H |\n| - | - | - | - | - | - | - | - |\n' + '| x | y | z | a | b | c | d | e |\n'.repeat(1_300);
    const client = fixture([user('Keep this.'), answer(body)]);
    const before = mobileThreadRows(client, now), after = mobileThreadRows(client, now);
    expect(after[0]!.blocks).toBe(before[0]!.blocks);
    expect(after[1]!.blocks).not.toBe(before[1]!.blocks);
    expect(after[1]!.blocks[0]!.rows).toHaveLength(1_301);
    expect(after[1]!.blocks[0]!.rows.at(-1)!.cells.map(cell => cell.runs[0]!.text)).toEqual(['x', 'y', 'z', 'a', 'b', 'c', 'd', 'e']);
    expect(JSON.parse(after[1]!.copyConfiguration).text).toBe(body);
  });

  test('ragged and empty-header tables retain complete source, while valid empty cells and numeric labels remain readable', () => {
    for (const table of ['| A | B |\n| - | - |\n| short |', '| A |\n| - |\n| extra | dropped |', '| | |\n| - | - |\n| x | y |']) {
      const body = `Before\n\n${table}\n\nAfter`, blocks = mobileThreadMarkdownBlocks(body, false);
      expect(blocks.some(block => block.kind === 'table')).toBe(false);
      expect(blocks.map(block => block.text).join('')).toBe(body);
    }
    const [block] = mobileThreadMarkdownBlocks('| A | B |\n| - | - |\n| | |\n| NaN | Infinity |\n| v2 | 1,2 |', false);
    expect(block!.kind).toBe('table');
    expect(block!.rows.map(row => row.cells.map(cell => cell.numeric))).toEqual([[false, false], [false, false], [false, false], [false, false]]);
    expect(block!.rows[1]!.cells.map(cell => cell.runs)).toEqual([[], []]);
  });

  test('adjacent table identities stay distinct across edits and table-to-code streaming preserves order', () => {
    const table = '| A | B |\n| - | -: |\n| first | 1 |', body = `${table}\n\nBetween.\n\n${table}`;
    const response = answer(body), client = fixture([response]);
    const first = mobileThreadRows(client, now)[0]!, saved = structuredClone(first);
    expect(first.blocks.map(block => block.kind)).toEqual(['table', 'markdown', 'table']);
    expect(new Set(first.blocks.map(block => block.id)).size).toBe(3);
    response.text = body.replace('first', 'edited longer');
    const edited = mobileThreadRows(client, now)[0]!;
    expect(new Set(edited.blocks.map(block => block.id)).size).toBe(3);
    expect(edited.blocks[0]!.id).toBe(first.blocks[0]!.id);
    expect(first).toEqual(saved);
    response.text = `Before\n\n\x60\x60\x60text\n${table}\n\x60\x60\x60\n\nAfter`;
    const fenced = mobileThreadRows(client, now)[0]!;
    expect(fenced.id).toBe(first.id);
    expect(fenced.blocks.map(block => block.kind)).toEqual(['markdown', 'code', 'markdown']);
    expect(fenced.blocks[1]!.text).toBe(table);
    expect(JSON.parse(fenced.copyConfiguration).text).toBe(response.text);
  });

  test('decoded table text participates in the existing aggregate text budget', () => {
    const cell = 'a'.repeat(70_000), body = '| A | B |\n| --- | --- |\n| ' + cell + ' | ' + cell + ' |';
    const items = [0, 1].map(index => item(`table-${index}`, 'assistant_message', { text: body, messageId: `m${index}`, streaming: false }));
    const client = fixture(items);
    const before = mobileThreadRows(client, now), after = mobileThreadRows(client, now);
    expect(after[0]!.blocks).not.toBe(before[0]!.blocks);
    expect(after[1]!.blocks).toBe(before[1]!.blocks);
    for (const row of after) {
      expect(row.blocks[0]!.rows[1]!.cells.map(cell => cell.runs[0]!.text)).toEqual([cell, cell]);
      expect(JSON.parse(row.copyConfiguration).text).toBe(body);
    }
  });

  test('streaming replaces changed blocks without rebuilding unchanged prose or retaining stale metadata', () => {
    const prompt = user('Keep **this** prompt.'), response = answer('First paragraph.');
    const client = fixture([prompt, response]);
    const before = mobileThreadRows(client, now, false, 'visit');
    response.text = 'First paragraph.\n\nNext paragraph.';
    response.streaming = true;
    response.updatedAt = at(120);
    const after = mobileThreadRows(client, now, false, 'visit');
    expect(after[0]!.blocks).toBe(before[0]!.blocks);
    expect(after[1]!.blocks).not.toBe(before[1]!.blocks);
    expect(after[1]).toMatchObject({ body: response.text, streaming: true, showMeta: false, timestamp: mobileMessageTime(at(120)) });
    expect(JSON.parse(after[1]!.copyConfiguration).text).toBe(response.text);
    expect(before[1]!.blocks).toEqual([{ id: '0', kind: 'markdown', text: 'First paragraph.', language: '', tokens: [], columns: 0, alignments: [], rows: [] }]);
    response.streaming = false;
    response.updatedAt = at(180);
    const completed = mobileThreadRows(client, now, false, 'visit');
    expect(completed[1]!.blocks).toBe(after[1]!.blocks);
    expect(completed[1]).toMatchObject({ streaming: false, showMeta: true, timestamp: mobileMessageTime(at(180)) });
  });

  test('long forward traversals retain a bounded tail through streaming and prepended history', () => {
    const items = Array.from({ length: 400 }, (_, index) => item(`answer-${index}`, 'assistant_message',
      { text: `Paragraph ${index}.`, messageId: `message-${index}`, streaming: false, ordinal: index + 1 }));
    const client = fixture(items, { runs: [{ id: 'r1', status: 'running', startedAt: at(0) }] });
    const first = mobileThreadRows(client, now);
    const second = mobileThreadRows(client, now);
    const content = (rows: typeof first) => rows.filter(row => row.kind === 'assistant');
    const a = content(first), b = content(second);
    expect(a).toHaveLength(400);
    expect(b.map(row => row.body)).toEqual(items.map(value => value.text));
    expect(b[0]!.blocks).not.toBe(a[0]!.blocks);
    expect(b.at(-1)!.blocks).toBe(a.at(-1)!.blocks);
    expect(b.filter((row, index) => row.blocks === a[index]!.blocks).length).toBeLessThanOrEqual(128);
    const third = content(mobileThreadRows(client, now));
    expect(third.at(-1)!.blocks).toBe(a.at(-1)!.blocks);
    client.thread = mergeHistory(client.thread!, { snapshotSequence: 1, nextCursor: null, hasMoreHistory: false,
      items: [{ position: 0, visibility: 'local', sourceThreadId: 't1', sourceItemId: 'older',
        item: item('older', 'assistant_message', { text: 'Older paragraph.', messageId: 'older', streaming: false, ordinal: 0 }) }] });
    const prepended = content(mobileThreadRows(client, now));
    expect(prepended.map(row => row.body)).toEqual(['Older paragraph.', ...items.map(value => value.text)]);
    expect(prepended.at(-1)!.blocks).toBe(a.at(-1)!.blocks);
  });

  test('theme, route and client ownership changes invalidate blocks without changing message identity', () => {
    const client = fixture([answer('```typescript\nconst answer = 42;\n```')]);
    let before = mobileThreadRows(client, now, false, 'visit')[0]!;
    for (const change of [() => { client.generation++; }, () => { client.threadEpoch++; },
      () => { client.environmentId = 'other'; }, () => { client.origin = 'https://other.example'; },
      () => { client.projectId = 'other-project'; }, () => { client.threadId = 'other-thread'; }]) {
      change();
      const after = mobileThreadRows(client, now, false, 'visit')[0]!;
      expect(after.id).toBe(before.id);
      expect(after.blocks).not.toBe(before.blocks);
      expect(JSON.parse(after.copyConfiguration).owner).toMatchObject({ generation: client.generation,
        epoch: client.threadEpoch, environment: client.environmentId, origin: client.origin, project: client.projectId, thread: client.threadId });
      before = after;
    }
    const dark = mobileThreadRows(client, now, true, 'visit')[0]!;
    expect(dark.blocks).not.toBe(before.blocks);
    expect(dark.blocks[0]!.tokens.map(token => token.text).join('')).toBe('const answer = 42;');
    expect(mobileThreadRows(client, now, true, 'new-visit')[0]!.blocks).not.toBe(dark.blocks);
    const otherClient = fixture([answer('```typescript\nconst answer = 42;\n```')]);
    expect(mobileThreadRows(otherClient, now, true, 'new-visit')[0]!.blocks).not.toBe(dark.blocks);
  });

  test('oversized messages remain complete and do not evict a reusable small message', () => {
    const huge = 'Long paragraph. '.repeat(40_000);
    const client = fixture([user('Keep this.'), answer(huge)]);
    const before = mobileThreadRows(client, now), after = mobileThreadRows(client, now);
    expect(after[0]!.blocks).toBe(before[0]!.blocks);
    expect(after[1]!.blocks).not.toBe(before[1]!.blocks);
    expect(after[1]!.blocks.map(block => block.text).join('')).toBe(huge);
    expect(JSON.parse(after[1]!.copyConfiguration).text).toBe(huge);
  });

  test('aggregate text and individual object budgets bound reuse without truncating content', () => {
    const items = Array.from({ length: 8 }, (_, index) => item(`large-${index}`, 'assistant_message',
      { text: `${index}:` + 'a'.repeat(40_000), messageId: `large-${index}`, streaming: false }));
    const client = fixture(items, { runs: [{ id: 'r1', status: 'running', startedAt: at(0) }] });
    const before = mobileThreadRows(client, now).filter(row => row.kind === 'assistant');
    const after = mobileThreadRows(client, now).filter(row => row.kind === 'assistant');
    expect(after.map(row => row.body)).toEqual(items.map(value => value.text));
    const reused = after.filter((row, index) => row.blocks === before[index]!.blocks);
    expect(reused.length).toBeGreaterThan(0);
    expect(reused.reduce((sum, row) => sum + row.body.length * 2, 0)).toBeLessThanOrEqual(512 * 1024);
    expect(after[0]!.blocks).not.toBe(before[0]!.blocks);

    const manyBlocks = '```\nx\n```\n'.repeat(17_000);
    const dense = fixture([user('Keep this.'), answer(manyBlocks)]);
    const first = mobileThreadRows(dense, now), second = mobileThreadRows(dense, now);
    expect(second[0]!.blocks).toBe(first[0]!.blocks);
    expect(second[1]!.blocks).not.toBe(first[1]!.blocks);
    expect(second[1]!.blocks).toHaveLength(17_000);
    expect(second[1]!.blocks.every(block => block.text === 'x' && block.tokens.map(token => token.text).join('') === 'x')).toBe(true);
    expect(JSON.parse(second[1]!.copyConfiguration).text).toBe(manyBlocks);
  });

  test('inherited source identities stay separate and code edits leave previous snapshots intact', () => {
    const first = answer('```typescript\nconst n = 1;\n```'), second = answer('Other source.');
    const client = fixture([first, second]);
    client.thread!.projection.visibleTurnItems[1]!.sourceThreadId = 'ancestor';
    const before = mobileThreadRows(client, now), saved = structuredClone(before);
    const unchanged = mobileThreadRows(client, now);
    expect(unchanged.map(row => row.id)).toEqual(['["t1","answer"]', '["ancestor","answer"]']);
    expect(unchanged[0]!.blocks).toBe(before[0]!.blocks);
    expect(unchanged[1]!.blocks).toBe(before[1]!.blocks);
    first.text = '```typescript\nconst n = 2;\n```';
    const edited = mobileThreadRows(client, now);
    expect(edited[0]!.blocks[0]!.tokens.map(token => token.text).join('')).toBe('const n = 2;');
    expect(edited[1]!.blocks).toBe(before[1]!.blocks);
    expect(before).toEqual(saved);
    client.threadId = 'other'; mobileThreadRows(client, now);
    client.threadId = 't1';
    expect(mobileThreadRows(client, now)[0]!.blocks).not.toBe(edited[0]!.blocks);
  });

  test('answer-file preparation leaves the visible route and theme cache intact', async () => {
    const client = fixture([user('Read this.'), answer('```typescript\nconst n = 1;\n```')]);
    const before = mobileThread(now, true, client, 'visible-visit');
    await mobileThreadAnswerFilesPrepare(before.answerFilesOwner, before.answerFilesRequest, now, undefined, client);
    const after = mobileThread(now, true, client, 'visible-visit');
    expect(after.rows[0]!.blocks).toBe(before.rows[0]!.blocks);
    expect(after.rows[1]!.blocks).toBe(before.rows[1]!.blocks);
    expect(after.rows).toEqual(before.rows);
  });
});

test('New agent attribution preserves two senders and exact automation precedence without rewriting messages', () => {
  const entry = (id: string, extra: Obj = {}) => item(id, 'user_message', { text: id, messageId: id, attachments: [], createdBy: 'agent', ...extra });
  const prefix = '[Triggered by schedule task: Nightly]\n\n';
  const items = [entry('first', { senderThreadId: 'sender-a' }), entry('second', { senderThreadId: 'sender-b' }), entry('no-sender'),
    entry('metadata', { senderThreadId: 'not-a-link', scheduledTaskId: 'schedule' }),
    entry('empty-metadata', { senderThreadId: 'not-a-link', scheduledTaskId: '' }),
    entry('legacy-agent', { text: prefix + 'agent scheduled body', senderThreadId: 'not-a-link' }),
    entry('legacy-id', { text: prefix + 'old scheduled body', createdBy: 'user', messageId: 'scheduled-task-message:nightly:42:manual', senderThreadId: 'not-a-link' }),
    entry('literal-user', { text: prefix + 'literal user body', createdBy: 'user', senderThreadId: 'not-a-link' }),
    entry('normal', { createdBy: 'user', senderThreadId: 'not-a-link' }),
    entry('unanchored', { text: 'literal ' + prefix + 'agent body', senderThreadId: 'sender-a' })];
  const client = fixture(items), before = JSON.stringify(client.thread!.projection), shared = transcriptRows(client, true);
  expect(shared.filter(row => row.kind === 'user')).toHaveLength(items.length);
  expect(shared.slice(0, 2).map(row => row.targetId)).toEqual(['sender-a', 'sender-b']);
  const rows = mobileThreadRows(client, now, false, 'attribution-visit');
  expect(rows.map(row => row.body)).toEqual(items.map(value => value.text));
  expect(rows.map(row => JSON.parse(row.copyConfiguration).text)).toEqual(items.map(value => value.text));
  expect(JSON.stringify(client.thread!.projection)).toBe(before);
  expect(rows.map(row => [row.attribution, obj(row).senderThreadId ?? null])).toEqual([
    ['Sent by another agent', 'sender-a'], ['Sent by another agent', 'sender-b'], ['Sent by another agent', ''],
    ['Sent by automation', ''], ['Sent by automation', ''], ['Sent by automation', ''], ['Sent by automation', ''],
    ['', ''], ['', ''], ['Sent by another agent', 'sender-a']]);
});

test('New agent attribution keeps inherited message sender in the current feed environment', () => {
  const message = item('inherited-agent', 'user_message', { text: 'Inherited task', attachments: [], createdBy: 'agent', senderThreadId: 'ancestor-sender' });
  const client = fixture([message]), inherited = client.thread!.projection.visibleTurnItems[0]!;
  inherited.visibility = 'inherited'; inherited.sourceThreadId = 'ancestor'; inherited.sourceItemId = 'ancestor-item';
  const data = mobileThread(now, false, client, 'inherited-visit'), row = data.rows[0]!;
  expect(data.loaded).toBe(true); expect(row.user).toBe(true);
  expect(transcriptRows(client, true)[0]!.targetId).toBe('ancestor-sender');
  expect({ environment: data.environmentId, sourceThread: data.threadId, attribution: row.attribution, sender: obj(row).senderThreadId ?? null })
    .toEqual({ environment: 'env', sourceThread: 't1', attribution: 'Sent by another agent', sender: 'ancestor-sender' });
});

test('New agent attribution keeps equal sender IDs separate in two environment snapshots', () => {
  const results = ['environment-a', 'environment-b'].map(environment => {
    const client = fixture([item('agent', 'user_message', { text: 'Agent task', attachments: [], createdBy: 'agent', senderThreadId: 'same-sender' })]);
    client.environmentId = environment;
    const data = mobileThread(now, false, client, environment + '-visit');
    expect(data.loaded).toBe(true); expect(data.rows).toHaveLength(1);
    return [data.environmentId, data.threadId, obj(data.rows[0]).senderThreadId ?? null];
  });
  expect(results).toEqual([['environment-a', 't1', 'same-sender'], ['environment-b', 't1', 'same-sender']]);
});

test('confirmed user intent badge preserves queued origin after promotion', () => {
  const message = user('Promoted follow-up'), client = fixture([message]);
  const before = mobileThreadRows(client, now, false, 'visit')[0]!;
  message.inputIntent = 'promoted_queued_to_steer';
  const row = mobileThreadRows(client, now, false, 'visit').find(row => row.user)!;
  expect(row.intent).toBe('queued → steer');
  expect(row.intentDescription).toBe('Originally queued, then promoted to steer the active turn');
  expect(row.intentTone).toBe('steer');
  expect([row.body, row.timestamp, row.copyConfiguration, row.toggleOp, row.toggleId])
    .toEqual([before.body, before.timestamp, before.copyConfiguration, before.toggleOp, before.toggleId]);
});

test('confirmed user badges use raw input intent and leave other message kinds unbadged', () => {
  const cases = [
    ['queued_turn', 'queued', 'Queued behind the active turn', 'queued'],
    ['steer', 'steer', 'Steered the active turn', 'steer'],
    ['turn_start', '', '', ''], [undefined, '', '', ''], ['unknown', '', '', ''],
  ];
  for (const [inputIntent, label, description, tone] of cases) {
    const message = user('Follow-up'); message.inputIntent = inputIntent;
    const client = fixture([message]);
    client.thread!.projection.visibleTurnItems[0]!.sourceThreadId = 'ancestor';
    client.thread!.projection.visibleTurnItems[0]!.sourceItemId = 'ancestor-item';
    const row = mobileThreadRows(client, now)[0]!;
    expect([row.intent, row.intentDescription, row.intentTone]).toEqual([label, description, tone]);
    expect(JSON.parse(row.copyConfiguration).owner).toMatchObject({ sourceThread: 'ancestor', sourceItem: 'ancestor-item' });
  }
  const assistant = answer('Reply'); assistant.inputIntent = 'queued_turn';
  const row = mobileThreadRows(fixture([assistant]), now)[0]!;
  expect([row.intent, row.intentDescription, row.intentTone]).toEqual(['', '', '']);
});

test('a saving local outbox row keeps its plain status without an intent badge', async () => {
  const client = fixture(); client.origin = 'https://intent.test';
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: client.origin, environmentId: 'env', threadId: 't1',
    messageId: 'outbox-message', commandId: 'outbox-command', text: 'Saved follow-up', attachments: [], createdAt: at(0), dispatchMode: 'steer' };
  let settle!: (value: unknown) => void, mutationId = '';
  const native: Native = { available: true, watch() {}, later(raw) {
    const request = raw as Obj;
    if (request.op !== 'mobileOutbox') throw new Error('Only the outbox fixture is admitted');
    if (request.action === 'read') return Promise.resolve({ ok: true, generation: client.generation, value: {
      ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], revisions: {}, tokens: {}, outcomes: [], mutations: [], transfers: [] } });
    mutationId = String(request.mutationId);
    return new Promise(resolve => { settle = resolve; });
  } };
  expect(await mobileOutboxRead(client, native)).toBe(true);
  const saved = mobileOutboxEnqueue(client, native, record);
  try {
    const row = mobileThreadRows(client, now)[0]!;
    expect([row.kind, row.intent, row.intentDescription, row.intentTone, row.timestamp, row.copyConfiguration, row.toggleOp])
      .toEqual(['pending', 'Saving…', '', '', 'Pending', '', '']);
  } finally {
    settle({ ok: true, generation: client.generation, value: { mutationId, messageId: record.messageId, status: 'failed',
      revision: 1, ownerEpoch: 'epoch', sequenceFloor: 1, record: null, removed: null, message: 'Fixture rejected save',
      current: { record: null, revision: 1, token: '', pending: false } } });
    await saved;
  }
});

test('a nonmessage plan row never receives a user input-intent badge', () => {
  const plan = item('plan', 'proposed_plan', { markdown: 'Inspect the configuration', inputIntent: 'queued_turn' });
  const row = mobileThreadRows(fixture([plan]), now)[0]!;
  expect([row.kind, row.user, row.body, row.intent, row.intentDescription, row.intentTone])
    .toEqual(['plan', false, 'Inspect the configuration', '', '', '']);
});

describe('ordinary message Copy command and native configuration', () => {
  function copyFixture(text: string, type = 'user_message', mode = 'success') {
    const entry = item('copy', type, { text, markdown: text, messageId: 'copy-message', streaming: false });
    const client = fixture([entry]); client.origin = 'https://copy.invalid'; client.generation = 3;
    const calls: Obj[] = [], writes: unknown[] = [];
    const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); },
      async atomicWriteFile(_path, bytes) { writes.push(JSON.parse(new TextDecoder().decode(bytes))); if (mode === 'save-error') throw new Error('disk full'); } } };
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Obj; calls.push(request);
      if (request.op === 'devicePresentation') return { ok: true, generation: 3, value: {} };
      if (request.op !== 'copyText') throw new Error(`Unexpected native operation ${String(request.op)}`);
      if (mode === 'let-go') throw { name: 'FetchError', kind: 'Aborted', message: 'The answer was let go.' };
      if (mode === 'rejected') return { ok: false, generation: 3, error: { kind: 'clipboard', message: 'Clipboard denied', uncertain: false } };
      if (mode === 'generation') client.generation = 4;
      return { ok: true, generation: 3, value: { copied: mode !== 'agent' } };
    } };
    const id = JSON.stringify(['t1', 'copy']);
    return { client, calls, writes, storage, native: letGoAware(native), id,
      run: (stamp = 'opaque-instance-and-attempt') => mobileMessageCopyCommand(client, id, stamp, now, letGoAware(native), storage) };
  }

  for (const [label, text, accepted] of [
    ['empty', '', false], ['whitespace', ' \n\t', true], ['exact limit', 'a'.repeat(1_000_000), true],
    ['over limit', 'a'.repeat(1_000_001), false], ['surrogate limit', '😀'.repeat(500_000), true],
    ['surrogate over limit', '😀'.repeat(500_000) + 'a', false],
  ] as const) test(`keeps actual UTF-16 Copy validation at ${label}`, async () => {
    const f = copyFixture(text), result = await f.run();
    const copies = f.calls.filter(row => row.op === 'copyText');
    expect(result.message).toBe(accepted ? '' : 'That message is no longer available.');
    expect(copies.length).toBe(accepted ? 1 : 0);
    if (accepted) expect(copies[0]).toEqual({ op: 'copyText', text, generation: 3,
      messageCopyStamp: 'opaque-instance-and-attempt', messageCopyId: f.id });
    expect(f.writes.length).toBe(accepted ? 1 : 0);
    expect(f.calls.filter(row => row.op !== 'copyText')).toEqual([expect.objectContaining({ op: 'devicePresentation' })]);
  });

  for (const type of ['user_message', 'assistant_message', 'proposed_plan']) test(`preserves raw ${type} payload and structured owner`, async () => {
    const raw = '# Raw\n```ts\nconst x = 1\n```', f = copyFixture(raw, type);
    const row = mobileThreadRows(f.client, now, false, 'visit|one')[0]!;
    expect(JSON.parse(row.copyConfiguration)).toEqual({ id: f.id, text: raw, owner: {
      origin: 'https://copy.invalid', environment: 'env', generation: 3, project: 'p1', thread: 't1', epoch: 0,
      route: 'visit|one', sourceThread: 't1', sourceItem: 'copy' } });
    if (type === 'proposed_plan') expect(row.showMeta).toBe(false);
    expect((await f.run()).message).toBe('');
    expect(f.calls.find(row => row.op === 'copyText')?.text).toBe(raw);
    expect(timelineView(f.client).copies.get(f.id)).toMatchObject({ nonce: 1, ok: true });
    expect(f.writes[0]).toMatchObject({ selections: { env: { projectId: 'p1', threadId: 't1' } } });
  });

  for (const [mode, message] of [['rejected', 'Clipboard denied'], ['agent', 'Copied message'],
    ['generation', 'The connection changed. Refresh before continuing.'], ['let-go', '']] as const)
    test(`preserves ${mode} result without confirmed-write success suppression`, async () => {
      const f = copyFixture('body', 'user_message', mode), result = await f.run();
      expect(result.message).toBe(message);
      expect(f.calls.filter(row => row.op === 'copyText')).toHaveLength(1);
      expect(f.writes).toHaveLength(mode === 'agent' ? 1 : 0);
      if (mode === 'rejected' || mode === 'generation') {
        expect(f.client.error).toBe(message); expect(timelineView(f.client).copies.get(f.id)?.ok).toBe(false);
      }
      if (mode === 'let-go') expect(timelineView(f.client).copies.has(f.id)).toBe(false);
    });

  test('retains save failure after a confirmed clipboard write', async () => {
    const f = copyFixture('body', 'user_message', 'save-error');
    expect((await f.run()).message).toBe('');
    expect(f.client.error).toBe('Could not save local drafts and preferences. Keep a copy before closing.');
    expect(f.writes).toHaveLength(1);
    expect(timelineView(f.client).copies.get(f.id)?.ok).toBe(true);
  });

  test('keeps invocation stamps independent across held, out-of-order command answers', async () => {
    const f = copyFixture('body'), entered = new Set<string>();
    const releases = new Map<string, (value: unknown) => void>();
    const entries = new Map<string, () => void>();
    const wait = (stamp: string) => new Promise<void>(resolve => { if (entered.has(stamp)) resolve(); else entries.set(stamp, resolve); });
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Obj; f.calls.push(request);
      if (request.op === 'devicePresentation') return { ok: true, generation: 3, value: {} };
      if (request.op !== 'copyText') throw new Error('Unexpected operation');
      const stamp = String(request.messageCopyStamp); entered.add(stamp); entries.get(stamp)?.();
      return new Promise(resolve => releases.set(stamp, resolve));
    } };
    const first = mobileMessageCopyCommand(f.client, f.id, 'one', now, native, f.storage);
    await wait('one');
    const second = mobileMessageCopyCommand(f.client, f.id, 'two', now, native, f.storage);
    await wait('two'); releases.get('two')!({ ok: true, generation: 3, value: { copied: true } });
    expect((await second).message).toBe('');
    releases.get('one')!({ ok: false, generation: 3, error: { kind: 'clipboard', message: 'First denied', uncertain: false } });
    expect((await first).message).toBe('First denied');
    expect(f.calls.filter(row => row.op === 'copyText').map(row => row.messageCopyStamp)).toEqual(['one', 'two']);
    expect(f.calls.filter(row => row.op !== 'copyText').map(row => row.op)).toEqual(['devicePresentation', 'devicePresentation']);
  });

  test('untagged ordinary and code Copy retain their command results and request shape', async () => {
    const f = copyFixture('body');
    expect((await f.client.command('chatlocal:copy', f.id, '', now, f.native, f.storage)).message).toBe('Copied message');
    expect((await f.client.command('chatlocal:copy-code', 'code', 'raw code', now, f.native, f.storage)).message).toBe('Copied code');
    expect(f.calls.filter(row => row.op === 'copyText')).toEqual([
      { op: 'copyText', text: 'body', generation: 3 }, { op: 'copyText', text: 'raw code', generation: 3 }]);
  });
});

describe('mobile V2 transcript presentation', () => {
  const commentary = (id: string, text: string, seconds: number, extra: Obj = {}) => item(id, 'assistant_message',
    { text, messageId: `message-${id}`, streaming: false, startedAt: at(seconds), updatedAt: at(seconds), ...extra });
  const command = (id: string, seconds: number, extra: Obj = {}) => item(id, 'command_execution',
    { input: id, output: '', exitCode: 0, startedAt: at(seconds), updatedAt: at(seconds), ...extra });

  test('keeps initial commentary before Worked while intermediate prose and tools share the existing fold', async () => {
    const client = fixture([user('Inspect'), commentary('first', 'I will inspect it.', 1), command('pwd', 2),
      commentary('middle', 'Checking status next.', 3), command('status', 4), answer('Done')]);
    const before = structuredClone(client.projection), key = (id: string) => JSON.stringify(['t1', id]);
    const collapsed = mobileThreadRows(client, now);
    expect(collapsed.map(row => row.id)).toEqual([key('user'), key('first'), 'turn-fold:r1', key('answer')]);
    expect(collapsed[1]).toMatchObject({ body: 'I will inspect it.', showMeta: false });
    expect(collapsed[2]).toMatchObject({ kind: 'work', expanded: false, toggleId: 'r1' });
    expect(collapsed[3]).toMatchObject({ body: 'Done', showMeta: true });
    expect(transcriptRows(client).some(row => row.id === key('first'))).toBe(false);
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    const expanded = mobileThreadRows(client, now);
    expect(expanded.map(row => row.kind)).toEqual(['user', 'assistant', 'work', 'entry', 'assistant', 'entry', 'assistant']);
    expect(expanded.filter(row => row.id === key('first'))).toHaveLength(1);
    expect(expanded.find(row => row.id === key('middle'))?.body).toBe('Checking status next.');
    expect(expanded[2]).toMatchObject({ toggleId: 'r1', expanded: true });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    expect(mobileThreadRows(client, now)).toEqual(collapsed);
    expect(client.projection).toEqual(before);
  });

  test('two assistant messages alone remain visible without a spurious Worked fold', () => {
    const client = fixture([user('Inspect'), commentary('first', 'Starting.', 1), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => row.body)).toEqual(['Inspect', 'Starting.', 'Done']);
    expect(mobileThreadRows(client, now).some(row => row.kind === 'work')).toBe(false);
  });

  test('fold stays at the first hidden tool when work precedes the initial assistant message', () => {
    const client = fixture([user('Inspect'), command('pwd', 1), commentary('first', 'Checked the directory.', 2), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => [row.kind, row.body])).toEqual([
      ['user', 'Inspect'], ['work', ''], ['assistant', 'Checked the directory.'], ['assistant', 'Done'],
    ]);
    expect(transcriptRows(client, true).find(row => row.kind === 'work')?.createdAt).toBe(at(1));
  });

  test('middle assistant prose can fold even when a turn has no tools', () => {
    const client = fixture([user('Explain'), commentary('first', 'Starting.', 1), commentary('middle', 'More detail.', 2), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => [row.kind, row.body])).toEqual([
      ['user', 'Explain'], ['assistant', 'Starting.'], ['work', ''], ['assistant', 'Done'],
    ]);
  });

  test('runless prompts retain their own initial commentary and independent fold ownership', async () => {
    const prompt = (id: string, seconds: number) => item(id, 'user_message',
      { text: id, runId: null, messageId: id, inputIntent: 'turn_start', attachments: [], startedAt: at(seconds), updatedAt: at(seconds) });
    const items = [prompt('one', 0), commentary('first-one', 'First turn.', 1, { runId: null }), command('pwd', 2, { runId: null }),
      commentary('final-one', 'Done one.', 3, { runId: null }), prompt('two', 4), commentary('first-two', 'Second turn.', 5, { runId: null }),
      command('status', 6, { runId: null }), commentary('final-two', 'Done two.', 7, { runId: null })];
    const client = fixture(items, { runs: [] }), collapsed = mobileThreadRows(client, now);
    expect(collapsed.map(row => row.kind)).toEqual(['user', 'assistant', 'work', 'assistant', 'user', 'assistant', 'work', 'assistant']);
    const folds = collapsed.filter(row => row.kind === 'work');
    expect(folds[0]!.toggleId).not.toBe(folds[1]!.toggleId);
    await chatLocal(client, { available: true } as Native, 'fold', folds[0]!.toggleId, '');
    expect(mobileThreadRows(client, now).filter(row => row.kind === 'work').map(row => row.expanded)).toEqual([true, false]);
    expect(mobileThreadRows(client, now).filter(row => row.body.endsWith('turn.'))).toHaveLength(2);
  });

  for (const state of ['running', 'streaming', 'failed', 'interrupted']) {
    test(`initial commentary remains visible without a completed-work fold during ${state}`, () => {
      const rows = [user('Inspect'), commentary('first', 'Starting.', 1), command('pwd', 2),
        commentary('final', 'Done', 3, { streaming: state === 'streaming' })];
      if (state === 'interrupted') rows.push(item('interrupt', 'run_interrupt_result', { interrupted: true }));
      const client = fixture(rows, { runs: [{ id: 'r1', status: state === 'running' ? 'running' : state === 'failed' ? 'failed' : 'completed',
        startedAt: at(0), completedAt: state === 'running' ? null : at(4) }] });
      const shown = mobileThreadRows(client, now);
      expect(shown.find(row => row.body === 'Starting.')).toBeDefined();
      expect(shown.some(row => row.kind === 'work')).toBe(false);
    });
  }

  for (const files of [[], [{ path: 'src/app.ts', additions: 2, deletions: 1 }]]) {
    test(`omits ${files.length ? 'populated' : 'empty'} checkpoints before row boundaries and retains reconnect projection`, () => {
      const checkpoint = (id: string) => item(id, 'checkpoint', { checkpointId: id, scopeId: 'workspace', files });
      const client = fixture([checkpoint('before'), user('Hello'), answer('Done'), checkpoint('after')]);
      const before = structuredClone(client.projection);
      const shared = transcriptRows(client);
      expect(shared.filter(row => row.kind === 'checkpoint')).toHaveLength(2);
      const expected = [
        { id: JSON.stringify(['t1', 'user']), kind: 'user', first: true, last: false, showMeta: true },
        { id: JSON.stringify(['t1', 'answer']), kind: 'assistant', first: false, last: true, showMeta: true },
      ];
      expect(mobileThreadRows(client, now)).toMatchObject(expected);
      client.connection = 'reconnecting';
      expect(mobileThread(now, false, client)).toMatchObject({ loaded: true, loading: false, rows: expected });
      expect(client.projection).toEqual(before);
      expect(transcriptRows(client)).toEqual(shared);
    });
  }

  test('retains genuine file-change work and disclosure while omitting its checkpoint', async () => {
    const files = [{ path: 'src/app.ts', additions: 2, deletions: 1 }];
    const items = [user('Update the app'), item('edit', 'file_change', {
      fileName: '/repo/src/app.ts', changes: [{ operation: 'update', path: '/repo/src/app.ts' }],
    }), answer('Updated')];
    const client = fixture([...items, item('checkpoint', 'checkpoint', { checkpointId: 'cp1', scopeId: 'workspace', files })]);
    const withoutCheckpoint = fixture(items);
    expect(mobileThreadRows(client, now)).toEqual(mobileThreadRows(withoutCheckpoint, now));
    expect(mobileThreadRows(client, now).map(row => row.kind)).toEqual(['user', 'work', 'assistant']);
    expect(mobileThreadRows(client, now)[1]).toMatchObject({ toggleOp: 'chatlocal:fold', toggleId: 'r1', expanded: false });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    await chatLocal(withoutCheckpoint, { available: true } as Native, 'fold', 'r1', '');
    const rows = mobileThreadRows(client, now);
    expect(rows).toEqual(mobileThreadRows(withoutCheckpoint, now));
    const edit = rows.flatMap(row => row.activities).find(activity => activity.id === JSON.stringify(['t1', 'edit']));
    expect(edit?.body).toContain('src/app.ts');
    expect(rows.some(row => row.kind === 'checkpoint')).toBe(false);
    expect(rows.at(-1)).toMatchObject({ kind: 'assistant', last: true, showMeta: true });
  });

  test('keeps scoped shared ids, hides mobile blank answers, and retains reconnect cache', () => {
    const client = fixture([user('Hello'), answer('')]);
    expect(mobileThreadRows(client, now).map(row => row.id)).toEqual([JSON.stringify(['t1', 'user'])]);
    client.connection = 'reconnecting';
    expect(mobileThread(now, false, client)).toMatchObject({ loaded: true, loading: false, title: 'Actual title', rows: [{ first: true, last: true }] });
    client.thread = null; client.connection = 'disconnected';
    expect(mobileThread(now, false, client)).toMatchObject({ loaded: false, emptyTitle: 'Messages not cached' });
  });
  test('disclosure mutates shared timeline state and projects real tool details', async () => {
    const client = fixture([user('Run tests'), item('command', 'command_execution', { input: 'bun test', output: 'passed', exitCode: 0 }), answer('Done')]);
    const folded = mobileThreadRows(client, now);
    expect(folded.map(row => row.kind)).toEqual(['user', 'work', 'assistant']);
    expect(folded[1]).toMatchObject({ toggleOp: 'chatlocal:fold', toggleId: 'r1', expanded: false });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    expect(mobileThreadRows(client, now).some(row => row.activities.some(activity => activity.body === 'bun test'))).toBe(true);
  });
  test('duration labels use supplied clock and streaming answers have no metadata', () => {
    const client = fixture([user('Go'), answer('Partial')], { runs: [{ id: 'r1', status: 'running', startedAt: at(0) }] });
    arr(client.projection.visibleTurnItems)[1]!.item = { ...answer('Partial'), streaming: true, status: 'running' };
    const rows = mobileThreadRows(client, now + 65_000);
    expect(rows.find(row => row.kind === 'working')?.title).toBe('Working for 1m 5s');
    expect(rows.find(row => row.kind === 'assistant')?.showMeta).toBe(false);
  });
  test('mobile approval defaults and explicit server order retain shared permission refusal', () => {
    const approval = item('approval', 'approval_request', { requestId: 'request', requestKind: 'command', prompt: 'Run command?' });
    const client = fixture([approval], { runtimeRequests: [{ id: 'request', status: 'pending', kind: 'command', createdAt: at(0), responseCapability: { type: 'live' } }] });
    expect(mobileThread(now, false, client).approvals[0]).toMatchObject({ title: 'command', disabled: false,
      options: [{ id: 'accept', label: 'Allow once' }, { id: 'acceptForSession', label: 'Allow session' }, { id: 'decline', label: 'Decline' }] });
    approval.options = [{ decision: 'decline', label: 'No' }, { decision: 'accept', label: 'Yes', warning: 'Runs a command' }];
    client.scopes = [];
    expect(mobileThread(now, false, client).approvals[0]).toMatchObject({ disabled: true, options: [{ id: 'decline', label: 'No' }, { id: 'accept', label: 'Yes' }] });
  });
  test('approval warning uses the first truthy provider warning without changing options or admission', () => {
    const approval = item('approval', 'approval_request', { requestId: 'request', requestKind: 'command', prompt: 'Run command?' });
    const client = fixture([approval], { runtimeRequests: [{ id: 'request', status: 'pending', kind: 'command', createdAt: at(0), responseCapability: { type: 'live' } }] });
    const view = () => mobileThread(now, false, client).approvals[0]!;
    expect(view()).toMatchObject({ warning: '', disabled: false });
    approval.options = [];
    expect(view()).toMatchObject({ warning: '', options: [] });
    const options = [{ decision: 'decline', label: 'No', warning: '' },
      { decision: 'acceptForSession', label: 'Allow session', warning: '  Future commands may run.\nConfirm this choice.  ' },
      { decision: 'accept', label: 'Yes', warning: 'Later warning' }];
    approval.options = options;
    expect(view()).toMatchObject({ warning: options[1]!.warning, disabled: false,
      options: options.map(option => ({ id: option.decision, label: option.label, warning: option.warning })) });
    expect(approval.options).toEqual(options);
    client.scopes = [];
    expect(view()).toMatchObject({ warning: options[1]!.warning, disabled: true });
    client.scopes = ['orchestration:operate'];
    client.local.pending[client.environmentId] = { method: 'orchestration.dispatchCommand', payload: { requestId: 'request' }, description: 'Respond', threadId: 't1', text: '', uncertain: false };
    expect(view()).toMatchObject({ warning: options[1]!.warning, disabled: true });
    delete client.local.pending[client.environmentId];
    approval.options = [{ decision: 'accept', label: 'Yes', warning: ' \n ' }, ...options];
    expect(view()).toMatchObject({ warning: ' \n ', disabled: false });
    approval.options = [{ decision: 'accept', label: 'Yes', warning: '' }, { decision: 'decline', label: 'No' }];
    expect(view().warning).toBe('');
  });
  test('composer admission follows actual synchronization, model and request permissions', () => {
    const client = fixture(); client.local.drafts[client.draftKey] = 'Inspect source';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: true, sendLabel: 'Send', modelUnavailable: false });
    client.connection = 'reconnecting';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, showReadOnlyNotice: false, modelUnavailable: false });
    client.connection = 'connected'; client.scopes = [];
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, showReadOnlyNotice: true });
    client.scopes = ['orchestration:operate']; client.modelId = 'missing';
    arr(client.config.providers)[0]!.driver = 'antigravity';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, modelUnavailable: true });
  });
  test('fenced blocks retain literal content and unknown languages remain readable', () => {
    const blocks = mobileMarkdownBlocks('Before\n\n```typescript\nconst a = 1;\n```\nAfter', false);
    expect(blocks.map(block => block.kind)).toEqual(['markdown', 'code', 'markdown']);
    expect(blocks[1]?.tokens.map(token => token.text).join('')).toBe('const a = 1;');
    expect(mobileCodeTokens('<unknown>', 'unregistered-language', true).map(token => token.text).join('')).toBe('<unknown>');
    for (const [body, expected] of [
      ['```text\nword```\nlast\n```', 'word```\nlast'],
      ['```text\none\n\n```', 'one\n'],
      ['````text\n```\nlast\n````', '```\nlast'],
      ['```text\n```~\nlast\n```', '```~\nlast'],
      ['  ~~~text\n  first\n second\n  ~~~~', 'first\nsecond'],
      ['```\n```', ''],
      ['```text\r\nfirst\r\nsecond\r\n```', 'first\nsecond'],
      ['```text\rfirst\rsecond\r```', 'first\nsecond'],
    ]) {
      const code = mobileMarkdownBlocks(body!, false);
      expect(code).toHaveLength(1); expect(code[0]?.kind).toBe('code'); expect(code[0]?.text).toBe(expected);
      expect(code[0]?.tokens.map(token => token.text).join('')).toBe(expected);
    }
  });
});

describe('HTML render metadata foundation', () => {
  const page = { attachmentId: 'source-thread-page.html', title: 'Revenue', height: 420 };
  const output = { htmlRender: page, message: 'Shown above the reply.' };
  const read = (value: unknown, extra: Obj = {}) => htmlRenderFromToolItem(item('render', 'dynamic_tool',
    { toolName: 'mcp__t3-code__html_render', output: value, ...extra }));

  test('reads provider envelopes and qualified tool identities without mutating compact projection data', () => {
    for (const [toolName, value] of [
      ['mcp__t3-code__html_render', [{ type: 'text', text: JSON.stringify(output) }]],
      ['t3-code.html_render', { structuredContent: output, content: [] }],
      ['t3-code-thread_1_html_render', JSON.stringify(output)],
      ['html_render', output],
      ['html_render completed', { content: [{ text: { text: JSON.stringify(output) } }] }],
    ] as const) {
      const compact = item('render', 'dynamic_tool', { toolName, output: value, outputOmitted: true });
      const before = structuredClone(compact);
      expect(htmlRenderFromToolItem(compact)).toEqual(page);
      expect(compact).toEqual(before);
    }
  });

  test('requires a successful original dynamic render rather than another tool or activity status', () => {
    for (const extra of [
      { type: 'command_execution' }, { status: 'inProgress' }, { status: 'running' }, { status: 'failed' },
      { toolName: 'mcp__other__html_render' }, { toolName: 'mcp__t3-code__html_preview' },
      { toolName: 't3-code-thread_html_render_extra' }, { toolName: null },
    ]) expect(read(output, extra)).toBeUndefined();
    expect(read(output, { outputOmitted: true })).toEqual(page);
  });

  test('keeps the first result data and nullish envelope precedence without searching later results', () => {
    const second = { htmlRender: { ...page, title: 'Other page' } };
    expect(read([{ text: output }, { text: second }])).toEqual(page);
    expect(read([{ text: '{}' }, { text: output }])).toBeUndefined();
    expect(read([{ text: 'not JSON' }, { text: output }])).toEqual(page);
    expect(read({ structuredContent: output, content: second })).toEqual(page);
    expect(read({ structuredContent: null, content: output })).toEqual(page);
    expect(read({ structuredContent: false, content: output })).toBeUndefined();
    expect(read({ result: output })).toBeUndefined();
  });

  test('accumulates exact failure markers across envelopes and later content blocks', () => {
    for (const failure of [{ isError: true }, { is_error: true }, { _tag: 'OrchestratorMcpFailure' },
      { error: { message: 'Failed' } }, { error: false }]) {
      expect(read({ ...failure, structuredContent: output })).toBeUndefined();
      expect(read([{ text: output }, { text: failure }])).toBeUndefined();
    }
    expect(read({ ...output, error: null, isError: false, _tag: 'OtherFailure' })).toEqual(page);
  });

  test('enforces UTF-8 and cumulative parsed-byte budgets, not an unrelated decoded-body limit', () => {
    const json = JSON.stringify(output), bytes = new TextEncoder().encode(json).byteLength;
    expect(read(json + ' '.repeat(16_384 - bytes))).toEqual(page);
    expect(read(json + ' '.repeat(16_385 - bytes))).toBeUndefined();
    const multibyte = JSON.stringify({ ...output, unused: '😄'.repeat(5000) });
    expect(multibyte.length).toBeLessThan(16_384);
    expect(read(multibyte)).toBeUndefined();
    expect(read([{ text: json + ' '.repeat(9000 - bytes) }, { text: '{}' + ' '.repeat(8998) }])).toBeUndefined();
    expect(read({ ...output, unused: 'x'.repeat(30_000) })).toEqual(page);
  });

  test('bounds envelope depth, block count and total visited nodes even after finding a render', () => {
    const wrap = (count: number) => Array.from({ length: count }).reduce<unknown>(value => ({ content: value }), output);
    expect(read(wrap(4))).toEqual(page);
    expect(read(wrap(5))).toBeUndefined();
    expect(read(Array.from({ length: 32 }, () => ({ text: output })))).toEqual(page);
    expect(read(Array.from({ length: 33 }, () => ({ text: output })))).toBeUndefined();
    const nodes = (atLimit: boolean) => Array.from({ length: 32 }, (_, index) => ({
      text: Array.from({ length: atLimit && index === 0 ? 2 : 3 }, () => ({ text: output })),
    }));
    expect(read(nodes(true))).toEqual(page); // 1 root + 32 inner arrays + 95 result objects.
    expect(read(nodes(false))).toBeUndefined();
    const cycle: Obj = {}; cycle.content = cycle;
    expect(read(cycle)).toBeUndefined();
  });

  test('normalizes title and height while preserving non-UUID attachment identity', () => {
    const original = { attachmentId: ' parent page.html ', title: '  ' + 'é'.repeat(210) + '  ', height: 420.6,
      heights: [[10000, 5000], [1, 0], [390, 300.4]] };
    const before = structuredClone(original), normalized = readHtmlRenderReference(original);
    expect(normalized).toEqual({ attachmentId: ' parent page.html ', title: 'é'.repeat(200), height: 421,
      heights: [[1, 80], [390, 300], [10000, 2000]] });
    expect(original).toEqual(before);
    expect(readHtmlRenderReference({ ...page, title: '   ', height: -9 })).toEqual({ ...page, title: 'HTML', height: 80 });
    for (const value of [null, { ...page, attachmentId: '' }, { ...page, attachmentId: 'x'.repeat(257) },
      { ...page, title: null }, { ...page, height: '420' }, { ...page, height: NaN }, { ...page, height: Infinity }]) {
      expect(readHtmlRenderReference(value)).toBeUndefined();
    }
  });

  test('omits an invalid optional measurement table without losing the published page', () => {
    for (const heights of [[], [[0, 300]], [[10001, 300]], [[390.5, 300]], [[390, NaN]], [[390, '300']],
      [[390, 300, 1]], [[390, 300], ['728', 400]], Array.from({ length: 25 }, (_, i) => [i + 1, 300])]) {
      expect(readHtmlRenderReference({ ...page, heights })).toEqual(page);
    }
    expect(readHtmlRenderReference({ ...page, heights: Array.from({ length: 24 }, (_, i) => [i + 1, 300]) })?.heights).toHaveLength(24);
  });

  test('uses taller responsive neighbors and author-requested scrolling caps at real frame widths', () => {
    const measured = readHtmlRenderReference({ ...page, height: 1500, heights: [[728, 1403], [390, 1290], [1000, 1660]] })!;
    expect(htmlRenderFrameHeight(measured, 320)).toBe(1290);
    expect(htmlRenderFrameHeight(measured, 559)).toBe(1403);
    expect(htmlRenderFrameHeight(measured, 728)).toBe(1403);
    expect(htmlRenderFrameHeight(measured, 1400)).toBe(1660);
    const responsive = readHtmlRenderReference({ ...page, height: 2000, heights: [[520, 900], [640, 450]] })!;
    expect(htmlRenderFrameHeight(responsive, 590)).toBe(900);
    expect(htmlRenderFrameHeight(responsive, 640)).toBe(450);
    expect(htmlRenderFrameHeight({ ...measured, height: 1403 }, 728, 1415)).toBe(1415);
    expect(htmlRenderFrameHeight(measured, 728, 5000)).toBe(2000);
    expect(htmlRenderFrameHeight({ ...measured, height: 600 }, 1400, 1415)).toBe(600);
    expect(htmlRenderFrameHeight(page, 728, 900)).toBe(420);
    expect(htmlRenderFrameHeight(page, 728, 300)).toBe(300);
  });

  test('compares all normalized identity fields and builds a safe readable HTML filename', () => {
    const measured = readHtmlRenderReference({ ...page, heights: [[728, 600], [390, 450]] })!;
    expect(htmlRenderReferencesEqual(measured, structuredClone(measured))).toBe(true);
    for (const changed of [{ ...measured, title: 'Other' }, { ...measured, attachmentId: 'other' },
      { ...measured, height: 421 }, { ...measured, heights: [[390, 450]] as const }]) {
      expect(htmlRenderReferencesEqual(measured, changed)).toBe(false);
    }
    expect(htmlRenderReferencesEqual(page, { ...page, heights: [] })).toBe(true);
    expect(htmlRenderFileName('  Revenue / FY:26\u0000  ')).toBe('Revenue FY 26.html');
    expect(htmlRenderFileName('  /:*?"<>|  ')).toBe('Page.html');
    expect(htmlRenderFileName('x'.repeat(121))).toBe('x'.repeat(120) + '.html');
  });
});

describe('dedicated HTML transcript rows', () => {
  const page = (id: string, extra: Obj = {}) => item(id, 'dynamic_tool', { toolName: 't3-code.html_render',
    output: { htmlRender: { attachmentId: `asset-${id}`, title: 'Report', height: 420 } }, ...extra });
  test('classifies before work grouping and preserves source order and inherited identity', () => {
    const client = fixture([user('Make a report'), page('page'), answer('Done')]);
    const row = arr(client.projection.visibleTurnItems)[1]!;
    row.visibility = 'inherited'; row.sourceThreadId = 'parent';
    const before = structuredClone(client.projection);
    const source = transcriptRows(client, true);
    expect(source.map(value => value.kind)).toEqual(['user', 'html-render', 'assistant']);
    expect(source[1]).toMatchObject({ id: JSON.stringify(['parent', 'page']), runId: 'r1',
      htmlRenderKey: JSON.stringify(['inherited', 'parent', 'page']) });
    expect(mobileThreadRows(client, now)[1]).toMatchObject({ kind: 'html-render', activities: [], media: [] });
    expect(client.projection).toEqual(before);
  });
  test('HTML survives turn and superseded-attempt folds while ordinary work can remain folded', () => {
    const client = fixture([user('Make a report'), item('tool', 'command_execution', { input: 'pwd', output: '/repo', exitCode: 0, nodeId: 'old' }),
      page('page', { nodeId: 'old' }), answer('Done')], { attempts: [{ id: 'old-attempt', runId: 'r1', rootNodeId: 'old', status: 'superseded' }], nodes: [] });
    const rows = transcriptRows(client, true);
    expect(rows.filter(row => row.kind === 'html-render')).toHaveLength(1);
    expect(rows.find(row => row.kind === 'html-render')?.id).toBe(JSON.stringify(['t1', 'page']));
    expect(rows.some(row => row.id === JSON.stringify(['t1', 'tool']))).toBe(false);
    expect(rows.some(row => row.kind === 'attempt' || row.kind === 'work')).toBe(true);
  });
  test('incomplete, failed, preview-only and non-HTML tool outputs keep ordinary work classification', () => {
    const values = [page('running', { status: 'in_progress' }), page('failed', { status: 'failed' }),
      page('preview', { toolName: 't3-code.html_preview' }), page('error', { output: { isError: true, content: { htmlRender: { attachmentId: 'bad', title: 'Bad', height: 80 } } } })];
    const client = fixture(values), entries = timelineEntries({ rows: arr(client.projection.visibleTurnItems), attempts: [], nodes: [] });
    expect(entries).toHaveLength(values.length);
    expect(entries.every(entry => entry.kind === 'work')).toBe(true);
  });
  test('fallback fold duration uses a trailing HTML entry creation time', () => {
    const client = fixture([user('Make a report'), item('tool', 'command_execution', { input: 'pwd', output: '/repo', exitCode: 0 }),
      answer('Done'), page('page', { startedAt: at(11), updatedAt: at(20) })],
      { runs: [{ id: 'r1', status: 'completed', startedAt: null, completedAt: at(20) }] });
    expect(transcriptRows(client, true).find(row => row.kind === 'work')?.title).toBe('Worked for 11s');
    expect(transcriptRows(client, true).at(-1)?.kind).toBe('html-render');
  });
  test('explicit run timing wins over the trailing HTML entry creation time', () => {
    const client = fixture([user('Make a report'), item('tool', 'command_execution', { input: 'pwd', output: '/repo', exitCode: 0 }),
      answer('Done'), page('page', { startedAt: at(11), updatedAt: at(20) })],
      { runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(20) }] });
    expect(transcriptRows(client, true).find(row => row.kind === 'work')?.title).toBe('Worked for 20s');
    expect(transcriptRows(client, true).at(-1)?.kind).toBe('html-render');
  });
});

// Golden tokens captured from pinned @shikijs/{core,engine-javascript,langs,themes}@4.2.0.
// Mobile uses github-light-default/github-dark-default; expected color case is normalized.
// Captured under Node24.13.1; Bun1.2.22 produces invalid comment tokens and is not this repo's pinned runtime.
const shikiFixtures = [{"language":"typescript","code":"const answer: number = 42; // hello","dark":false,"expected":[{"text":"const","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"answer","color":"#0550ae","weight":400,"slant":"normal"},{"text":":","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"number","color":"#0550ae","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"=","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"42","color":"#0550ae","weight":400,"slant":"normal"},{"text":"; ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"// hello","color":"#6e7781","weight":400,"slant":"normal"}]},{"language":"typescript","code":"const answer: number = 42; // hello","dark":true,"expected":[{"text":"const","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"answer","color":"#79c0ff","weight":400,"slant":"normal"},{"text":":","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"number","color":"#79c0ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"=","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"42","color":"#79c0ff","weight":400,"slant":"normal"},{"text":"; ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"// hello","color":"#8b949e","weight":400,"slant":"normal"}]},{"language":"json","code":"{\"enabled\": true, \"name\": \"T3\"}","dark":false,"expected":[{"text":"{","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"enabled\"","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"true","color":"#0550ae","weight":400,"slant":"normal"},{"text":", ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"name\"","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"T3\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":"}","color":"#1f2328","weight":400,"slant":"normal"}]},{"language":"json","code":"{\"enabled\": true, \"name\": \"T3\"}","dark":true,"expected":[{"text":"{","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"enabled\"","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"true","color":"#79c0ff","weight":400,"slant":"normal"},{"text":", ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"name\"","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"T3\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"}","color":"#e6edf3","weight":400,"slant":"normal"}]},{"language":"bash","code":"echo \"$HOME\" # hello","dark":false,"expected":[{"text":"echo","color":"#0550ae","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":"$HOME","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"# hello","color":"#6e7781","weight":400,"slant":"normal"}]},{"language":"bash","code":"echo \"$HOME\" # hello","dark":true,"expected":[{"text":"echo","color":"#79c0ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"$HOME","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"# hello","color":"#8b949e","weight":400,"slant":"normal"}]},{"language":"yaml","code":"name: mobile\nenabled: true","dark":false,"expected":[{"text":"name","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"mobile","color":"#0a3069","weight":400,"slant":"normal"},{"text":"\n","color":"#1f2328","weight":400,"slant":"normal"},{"text":"enabled","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"true","color":"#0550ae","weight":400,"slant":"normal"}]},{"language":"yaml","code":"name: mobile\nenabled: true","dark":true,"expected":[{"text":"name","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"mobile","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"\n","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"enabled","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"true","color":"#79c0ff","weight":400,"slant":"normal"}]}];
for (const fixture of shikiFixtures) test(`mobile Shiki ${fixture.language} ${fixture.dark ? 'dark' : 'light'} oracle`, () => {
  const tokens: { text: string; color: string; weight: number; slant: string }[] = [];
  for (const token of mobileCodeTokens(fixture.code, fixture.language, fixture.dark)) {
    const style = { text: token.text, color: token.color.toLowerCase(), weight: token.weight, slant: token.slant }, last = tokens.at(-1);
    if (last && last.color === style.color && last.weight === style.weight && last.slant === style.slant) last.text += style.text;
    else tokens.push(style);
  }
  expect(tokens).toEqual(fixture.expected);
});

test('queued composer projects dedicated text and never offers ordinary Stop', () => {
  const client = fixture([], { runs: [{ id: 'active', status: 'running' }] });
  client.local.drafts[client.draftKey] = 'ordinary draft';
  const edit: MobileQueuedEditSession = { owner: 'edit', session: 'unique', draftKey: 'env:t1~queued-edit~queued',
    origin: client.origin, environmentId: 'env', threadId: 't1', projectId: 'p1', generation: client.generation,
    revision: 1, runId: 'queued', messageId: 'queued-message', text: 'replacement', attachments: [], existingAttachments: [], saving: false };
  queuedEditState(client).sessions.set(edit.owner, edit); queuedEditState(client).active.set('env:t1', edit.owner);
  expect(mobileThreadComposer(client)).toMatchObject({ editing: true, draft: 'replacement', showStop: false, canSend: true, sendSymbol: 'checkmark', canCancel: true });
  queuedEditState(client).sessions.set(edit.owner, { ...edit, saving: true });
  expect(mobileThreadComposer(client)).toMatchObject({ saving: true, canSend: false, canCancel: false, blockedReason: 'Saving…' });
  queuedEditState(client).sessions.set(edit.owner, edit); client.modelId = 'unavailable';
  arr(client.config.providers)[0]!.driver = 'antigravity';
  expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, modelUnavailable: true });
  expect(client.draft).toBe('ordinary draft');
});


test('mobile unavailable notice follows the pinned Antigravity driver, including configured instances absent from catalog', () => {
  const selection = { instanceId: 'custom-instance', model: 'retained-model' };
  const provider = { instanceId: 'custom-instance', driver: 'antigravity', enabled: true, installed: true, auth: { status: 'authenticated' }, models: [{ slug: 'retained-model' }] };
  expect(mobileModelSelectionUnavailable(null, selection)).toBe(false);
  expect(mobileModelSelectionUnavailable({}, null)).toBe(false);
  expect(mobileModelSelectionUnavailable({ providers: [provider] }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [provider] }, selection)).toBe(true);
  for (const patch of [{ enabled: false }, { installed: false }, { auth: { status: 'unauthenticated' } }, { availability: 'unavailable' }, { models: [] }]) {
    expect(mobileModelSelectionUnavailable({ providers: [{ ...provider, ...patch }] }, selection)).toBe(true);
    expect(mobileModelSelectionReady({ providers: [{ ...provider, ...patch }] }, selection)).toBe(false);
  }
  expect(mobileModelSelectionUnavailable({ providers: [], settings: { providerInstances: { 'custom-instance': { driver: 'antigravity' } } } }, selection)).toBe(true);
  expect(mobileModelSelectionUnavailable({ providers: [], settings: { providerInstances: { 'custom-instance': { driver: 'codex' } } } }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [provider] }, { ...selection, model: '' })).toBe(false);
  // Source's notice does not test status:error. The transport still refuses it.
  expect(mobileModelSelectionUnavailable({ providers: [{ ...provider, status: 'error' }] }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [{ ...provider, status: 'error' }] }, selection)).toBe(false);
});
