import { afterEach, describe, expect, test } from 'bun:test';
import { opened, connected, running } from '../support/composer-controls-fixture';
import { arr, initialShell, obj, type Obj } from '../../src/client/domain';
import { resolveDispatchMode, isCompacting } from '../../src/composer/composer-controls';
import { presentBackgroundWork, primaryAction } from '../../src/composer/composer-controls-view';
import { draftContext, setEnvMode } from '../../src/composer/composer-controls-branch';
import { newThreadEnvMode, refreshNewThreadContext, resolveNewThreadEnvMode } from '../../src/composer/new-thread-context';
import { pullRequestRows, contextLink, contextId } from '../../src/composer/composer-editor-menu';
import { openThreadLink, projectPendingUserInputAnswer, formatThreadLink, parseThreadLinkHref, relabelThreadLinks } from '../../src/timeline/thread-links';
import { renderCodexInlineDirectivesAsMarkdown as directives } from '../../src/highlight/codex-directives.js';
import { importDroppedThread, pendingThreadImports, threadTranscriptAttachment } from '../../src/composer/composer-thread-import';
import { composerFileAttachments, draftFiles, stageTextAttachment, foldedText } from '../../src/composer/composer-editor-files';
import { threadContextRecords } from '../../src/composer/composer-editor';
import { fleet } from '../../src/connections/settings-b-fleet';
import { markdownThreadGlyphs } from '../../src/timeline/markdown-thread-glyphs';

afterEach(() => { fleet.entries.clear(); });

describe('server-owned setup and compaction', () => {
  for (const status of ['preparing', 'starting']) test(`a follow-up during ${status} queues without launching another workspace`, async () => {
    const { client, native, command } = await opened();
    running(client); obj(arr(client.projection.runs)[0]).status = status;
    obj(client.projection.thread).worktreePath = '/repo/.worktrees/retained';
    client.local.clientSettings.followUpBehavior = 'steer';
    expect(primaryAction(client, 'connecting')).toMatchObject({ sendLabel: 'Queue message', sendStatus: '', sendTooltip: 'Queue message' });
    await command('send', 'pointer:meta', 'Continue after setup');
    expect(client.error).toBe('');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't1', dispatchMode: { type: 'queue_after_active' } });
    expect(native.committed.some(command => command.method === 'orchestration.launchThread')).toBe(false);
    expect(obj(client.projection.thread).worktreePath).toBe('/repo/.worktrees/retained');
  });
  test('every modifier queues during /compact until its completed compaction item arrives', async () => {
    const { client, native, command } = await opened(); running(client);
    const runId = arr(client.projection.runs)[0]!.id;
    client.projection.visibleTurnItems = [{ item: { type: 'user_message', text: '/compact', attachments: [], runId } }];
    client.local.clientSettings.followUpBehavior = 'steer';
    expect(isCompacting(client)).toBe(true);
    for (const alternate of [true, false]) expect(resolveDispatchMode(true, 'steer', alternate, false)).toBe('queue');
    expect(primaryAction(client, 'running')).toMatchObject({ sendLabel: 'Queue message', sendTooltip: 'Queue message' });
    await command('send', 'pointer:meta', 'Follow-up');
    expect(native.committed.at(-1)).toMatchObject({ dispatchMode: { type: 'queue_after_active' } });
    client.projection.visibleTurnItems = [...arr(client.projection.visibleTurnItems), { item: { type: 'compaction', runId, status: 'completed' } }];
    expect(isCompacting(client)).toBe(false);
  });
  test('an automatic compaction running in the active run queues follow-ups too; a failed /compact settles it (#15631)', async () => {
    const { client } = await opened(); running(client);
    const runId = arr(client.projection.runs)[0]!.id;
    client.projection.visibleTurnItems = [{ item: { type: 'user_message', text: 'go on', attachments: [], runId } }, { item: { type: 'compaction', runId, status: 'running' } }];
    expect(isCompacting(client)).toBe(true);
    client.projection.visibleTurnItems = [{ item: { type: 'user_message', text: '/compact', attachments: [], runId } }, { item: { type: 'compaction', runId, status: 'failed' } }];
    expect(isCompacting(client)).toBe(false);
  });
  test('a named monitor shows its descriptive label once', () => {
    expect(presentBackgroundWork([{ kind: 'monitor', description: 'Waiting for checks', awaited: true }])?.title).toBe('Waiting for checks');
    expect(presentBackgroundWork([{ kind: 'monitor', description: 'Watch the dev server' }])?.title).toBe('Running: Watch the dev server');
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Review' }])?.title).toBe('Waiting on subagent Review');
  });
});

describe('first thread workspace', () => {
  const input = { configured: 'worktree', source: 'environment', workspaceRoot: '/projects/new', newProjectsRoot: '/projects', projectHasThreads: false };
  test('only a direct child of the new-project root gets a local first thread', () => {
    expect(resolveNewThreadEnvMode(input)).toBe('local');
    for (const extra of [{ projectHasThreads: true }, { source: 'project' }, { source: 't3.json' }, { workspaceRoot: '/projects/nested/new' }, { workspaceRoot: '/projects' }, { workspaceRoot: '/projectss/new' }]) expect(resolveNewThreadEnvMode({ ...input, ...extra })).toBe('worktree');
    expect(resolveNewThreadEnvMode({ ...input, workspaceRoot: 'C:/Projects/new/', newProjectsRoot: 'c:\\projects\\' })).toBe('local');
  });
  test('environment, project, file and an explicit draft choice keep their precedence', async () => {
    const { client, native } = await connected();
    client.shell.projects[0]!.workspaceRoot = '/projects/new'; client.shell.threads = [];
    client.config.newProjectsRoot = '/projects'; client.config.settings = { defaultThreadEnvMode: 'worktree' };
    expect(newThreadEnvMode(client)).toBe('local');
    client.config.settings = { defaultThreadEnvMode: 'worktree', projectSettingsOverrides: { p1: { defaultThreadEnvMode: 'worktree' } } };
    expect(newThreadEnvMode(client)).toBe('worktree');
    client.config.settings = { defaultThreadEnvMode: null };
    const original = native.later.bind(native);
    native.later = async request => obj(request).method === 'projects.readFile' ? native.ok({ contents: '{"defaultThreadEnvMode":"worktree"}' }) : original(request);
    await refreshNewThreadContext(client, native);
    expect(newThreadEnvMode(client)).toBe('worktree');
    setEnvMode(client, 'local'); expect(draftContext(client).envMode).toBe('local');
  });
});

describe('linked pull request suggestions', () => {
  const entry = { projectId: 'p', repository: 'owner/repo', number: 24, title: 'Current', updatedAt: '2026-01-03', url: 'https://github.com/owner/repo/pull/24' };
  const link = { repository: 'owner/other', number: 4, source: 'agent', linkedAt: '2026-01-01', url: 'https://github.com/owner/other/pull/4', snapshot: { title: 'Linked', state: 'open', updatedAt: '2026-01-02' } };
  test('exact matches precede linked results, then recent results, with repository disambiguation', () => {
    const rows = pullRequestRows([entry, { ...entry, number: 4, title: 'Exact' }], 'p', 'owner/repo', '4', [], [link]);
    expect(rows.map(row => [row.label, row.description])).toEqual([['#4', 'owner/other · Linked'], ['#4', 'Exact'], ['#24', 'Current']]);
    expect(pullRequestRows([entry], 'p', 'owner/repo', '', [], [{ ...link, repository: 'OWNER/REPO', number: 24 }])).toHaveLength(1);
    expect(pullRequestRows([], 'p', 'owner/repo', '', [], [{ ...link, snapshot: null }, { ...link, source: 'stack-dismissed' }])).toEqual([]);
    expect(pullRequestRows([], 'p', 'owner/repo', 'Linked', [], [link]).map(row => row.label)).toEqual(['#4']);
  });
  test('a text query puts the matching linked pull requests first, before and after the search returns (#26269)', () => {
    const fix = { ...link, repository: 'owner/repo', number: 7, snapshot: { title: 'Fix login', state: 'open', updatedAt: '2026-01-02' } };
    const other = { ...link, repository: 'owner/repo', number: 8, snapshot: { title: 'Other', state: 'open', updatedAt: '2026-01-02' } };
    expect(pullRequestRows([], 'p', 'owner/repo', 'fix', [], [fix, other]).map(row => row.label)).toEqual(['#7']);
    const searched = [{ ...entry, number: 9, title: 'Search hit', updatedAt: '2026-01-05' }, { ...entry, number: 8, title: 'Other', updatedAt: '2026-01-03' }];
    const hosts = [{ host: '', searchesOnHost: true }];
    expect(pullRequestRows(searched, 'p', 'owner/repo', 'fix', hosts, [fix, other]).map(row => row.label)).toEqual(['#7', '#8', '#9']);
  });
});

describe('thread links and pending answers', () => {
  test('literal percent escapes win; otherwise an encoded id resolves, including renames', () => {
    const threads = [{ id: 'mcp:1', title: 'Renamed' }, { id: 'literal%3A1', title: 'Literal' }];
    const markdown = '[Old](t3-thread://v1/mcp%3A1) [Old](t3-thread://v1/literal%3A1)';
    expect(relabelThreadLinks(markdown, threads)).toBe('[Renamed](t3-thread://v1/mcp:1) [Literal](t3-thread://v1/literal%3A1)');
    expect(relabelThreadLinks('`[Old](t3-thread://v1/mcp:1)`\n\n```md\n[Old](t3-thread://v1/mcp:1)\n```', threads)).toContain('[Old]');
    expect(relabelThreadLinks('[Missing](t3-thread://v1/gone)', threads)).toBe('[Missing](t3-thread://v1/gone)');
    expect(parseThreadLinkHref('t3-thread://v1/thread:mcp%3A1')).toEqual({ environmentId: null, threadId: 'thread:mcp%3A1' });
    expect(parseThreadLinkHref('t3-thread://v1/ ')).toBeNull();
    expect(formatThreadLink({ environmentId: null, threadId: 't1' }, 'A [title]\\\n')).toBe('[A title](t3-thread://v1/t1)');
  });
  // threadLinks.ts at v0.1.5 (#26239).
  test('a v2 link names its environment; without a label it is an autolink', () => {
    expect(parseThreadLinkHref('t3-thread://v2/env-1/thread:a%3A1')).toEqual({ environmentId: 'env-1', threadId: 'thread:a%3A1' });
    for (const bad of ['t3-thread://v2/env-1', 't3-thread://v2//t1', 't3-thread://v2/env-1/ ', 't3-thread://v3/env/t1']) expect(parseThreadLinkHref(bad)).toBeNull();
    expect(formatThreadLink({ environmentId: 'env-1', threadId: 't1' }, ' Fix CI ')).toBe('[Fix CI](t3-thread://v2/env-1/t1)');
    expect(formatThreadLink({ environmentId: 'env-1', threadId: 't1' })).toBe('<t3-thread://v2/env-1/t1>');
    expect(formatThreadLink({ environmentId: null, threadId: 't1' }, ' ')).toBe('<t3-thread://v1/t1>');
  });
  test('relabeling writes the current title only for a thread of the message\'s environment', () => {
    const threads = [{ id: 't1', title: 'Current' }];
    expect(relabelThreadLinks('[Old](t3-thread://v2/env-a/t1) [Old](t3-thread://v2/env-b/t1)', threads, 'env-a'))
      .toBe('[Current](t3-thread://v2/env-a/t1) [Old](t3-thread://v2/env-b/t1)');
    // An autolink reads the title too; one that stays keeps its URL as its text, as CommonMark draws it.
    expect(relabelThreadLinks('<t3-thread://v2/env-a/t1> <t3-thread://v2/env-b/t2> `<t3-thread://v1/t1>`', threads, 'env-a'))
      .toBe('[Current](t3-thread://v2/env-a/t1) [t3-thread://v2/env-b/t2](t3-thread://v2/env-b/t2) `<t3-thread://v1/t1>`');
  });
  test('an answer carries actual thread ids while unknown context remains written', () => {
    const chip = contextLink('thread', contextId('thread', 't1'), 'Review');
    const text = `${chip} ${chip} [unknown](t3-context://v1/thread/missing)`;
    expect(projectPendingUserInputAnswer(text, [{ contextId: contextId('thread', 't1'), threadId: 't1' }])).toBe('[Review](t3-thread://v1/t1) [Review](t3-thread://v1/t1) [unknown](t3-context://v1/thread/missing)');
    // A record names its thread's environment, so the link does (pendingUserInput.ts, #26239).
    expect(projectPendingUserInputAnswer(chip, [{ contextId: contextId('thread', 't1'), environmentId: 'env-1', threadId: 't1' }])).toBe('[Review](t3-thread://v2/env-1/t1)');
    expect(projectPendingUserInputAnswer(['Workspace'], [])).toEqual(['Workspace']);
  });
  test('a thread absent from the shell still opens through its bounded snapshot', async () => {
    const { client, native } = await opened();
    client.shell.threads = client.shell.threads.filter(thread => thread.id !== 't2');
    await openThreadLink(client, native, 't2');
    expect(client.threadId).toBe('t2'); expect(obj(client.projection.thread).id).toBe('t2');
    expect(client.threadLive).toBe(true); expect(client.error).toBe('');
  });
  test('thread links lead with their project favicon and leave unavailable threads on the fallback glyph', async () => {
    const { client } = await opened(); client.shell.projects[0]!.projectIcon = { kind: 'emoji', emoji: '🧪' };
    expect(markdownThreadGlyphs(client, '[Thread](t3-thread://v1/t1) [Missing](t3-thread://v1/gone)')).toMatchObject([
      { href: 't3-thread://v1/t1', title: 'Example', glyph: { kind: 'emoji', emoji: '🧪' } },
    ]);
  });
  test('kind-scoped ids with punctuation resolve back to the actual thread id', async () => {
    const { client } = await opened();
    client.shell.threads.push({ id: 'thread:mcp%3A1', title: 'Delegated' });
    const text = contextLink('thread', contextId('thread', 'thread:mcp%3A1'), 'Delegated');
    const records = threadContextRecords(client, text);
    expect(records[0]!.threadId).toBe('thread:mcp%3A1');
    expect(projectPendingUserInputAnswer(text, records)).toBe(`[Delegated](t3-thread://v2/${client.environmentId}/thread:mcp%3A1)`);
  });
});

describe('Codex inline directives', () => {
  test('keeps the visible follow-up label and drops only its hidden prompt', () => {
    expect(directives('Done.\n\n- :codex-followup[Prepare **print** version]{prompt="Prepare the document."}')).toBe('Done.\n\n- Prepare **print** version');
    for (const malformed of [':codex-followup[Label]', ':codex-followup[Label]{prompt=" "}', ':codex-followup[]{prompt="go"}', ':other[Label]{prompt="go"}']) expect(directives(malformed)).toBe(malformed);
  });
  test('file citations use a basename, URI escapes and a valid positive start line', () => {
    expect(directives(':codex-file-citation{path="src/a#b?.ts" line_range_start="12"}')).toBe('[a#b?.ts](<src/a%23b%3F.ts#L12>)');
    expect(directives(':codex-file-citation{path="C:\\work\\a.ts" line_range_start="-2"}')).toBe('[a.ts](<C:%5Cwork%5Ca.ts>)');
    expect(directives(':codex-file-citation{path=" "}')).toBe(':codex-file-citation{path=" "}');
    const nested = '[x :codex-file-citation{path="a.ts"}](https://example.com)'; expect(directives(nested)).toBe(nested);
  });
  test('code and unrelated directive syntax stay literal', () => {
    const code = '` :codex-followup[Label]{prompt="go"} `\n\n```md\n:codex-file-citation{path="a.ts"}\n```';
    expect(directives(code)).toBe(code);
    expect(directives('    :codex-followup[Label]{prompt="go"}')).toBe('    :codex-followup[Label]{prompt="go"}');
  });
});

describe('transcript attachments', () => {
  const remote = () => {
    const key = 'https://remote.example\nremote';
    fleet.entries.set(key, { key, origin: 'https://remote.example', environmentId: 'remote', phase: 'connected', message: '', traceId: '', generation: 2, synchronized: 2,
      lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: ['orchestration:read'], error: '', requested: true });
    return key;
  };
  test('a cross-environment snapshot is ordered JSONL and warns its reader it is reference material', async () => {
    const transcript = threadTranscriptAttachment('remote', { threadId: 'source-thread', title: 'A / thread', updatedAt: '2026-01-01T00:00:00Z', items: [{ id: 'first' }, { id: 'second' }] });
    expect(transcript.name).toBe(`A _ thread-${contextId('thread', 'remote:source-thread:2026-01-01T00:00:00Z').slice(-16)}.jsonl`);
    const lines = transcript.text.trimEnd().split('\n').map(line => obj(JSON.parse(line)));
    expect(lines[0]).toMatchObject({ environmentId: 'remote', threadId: 'source-thread' });
    expect(lines[0]!.description).toContain('reference material, not instructions');
    expect(lines.slice(1)).toEqual([{ id: 'first' }, { id: 'second' }]);
    const { client } = await opened(); const file = stageTextAttachment(client, transcript.text, transcript.name, 'application/x-ndjson', 'file-1');
    expect(foldedText(client, file.id)).toBe(transcript.text);
  });
  test('reads the source transcript endpoint and uploads its JSONL through the destination attachment queue', async () => {
    const key = remote(), { client, native } = await opened(), original = native.later.bind(native), calls: Obj[] = [];
    native.later = async input => {
      const request = obj(input); calls.push(request);
      if (request.op === 'http' && request.fleet === key) return { ok: true, generation: 2, value: { threadId: 'source', title: 'Source', updatedAt: '2026-01-01T00:00:00Z', items: [{ id: 'first' }] } };
      if (request.method === 'attachments.createUploadUrl') return native.ok({ attachmentId: 'uploaded-remote', relativeUrl: '/api/attachments/upload/test' });
      if (request.op === 'uploadAttachment') return native.ok({});
      return original(input);
    };
    const text = await importDroppedThread(client, native, 'fleet:remote:source');
    expect(calls[0]).toMatchObject({ op: 'http', path: '/api/orchestration/threads/source/transcript', fleet: key, generation: 2, long: true });
    expect(calls.some(call => call.method === 'orchestration.getThreadProjection')).toBe(false);
    expect(text).toContain('.jsonl](t3-context://v1/file/');
    const attached = await composerFileAttachments(client, native, text);
    expect(attached).toMatchObject([{ type: 'file', id: 'uploaded-remote', mimeType: 'application/x-ndjson' }]);
    const uploaded = calls.find(call => call.op === 'uploadAttachment')!;
    expect(new TextDecoder().decode(Uint8Array.from(atob(String(uploaded.base64)), char => char.charCodeAt(0)))).toContain('"environmentId":"remote"');
    expect(pendingThreadImports(client)).toBe(0);
  });
  test('a late source response cannot attach itself to a different draft', async () => {
    remote(); const { client, native } = await opened(), original = native.later.bind(native);
    let finish: (value: unknown) => void = () => {};
    native.later = async input => obj(input).op === 'http' ? new Promise(resolve => { finish = resolve; }) : original(input);
    const imported = importDroppedThread(client, native, 'fleet:remote:source');
    expect(pendingThreadImports(client)).toBe(1);
    client.threadId = 't2';
    finish({ ok: true, generation: 2, value: { threadId: 'source', title: 'Source', updatedAt: '2026-01-01', items: [] } });
    expect(await imported).toBe('');
    expect(draftFiles(client.local)).toEqual([]);
    expect(pendingThreadImports(client)).toBe(0);
  });
  test('the attachment limit reserves a slot before issuing the read', async () => {
    remote(); const { client, native } = await opened();
    client.local.snapshotDrafts[client.draftKey] = Array.from({ length: 100 }, (_, index) => ({ id: `image-${index}` }));
    await expect(importDroppedThread(client, native, 'fleet:remote:source')).rejects.toThrow('up to 100 files');
    expect(pendingThreadImports(client)).toBe(0);
  });
  test('questions share import reservations and cannot attach a 101st file', async () => {
    remote(); const { client, native } = await opened(), original = native.later.bind(native);
    client.projection.runtimeRequests = [{ id: 'r', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }];
    client.projection.turnItems = [{ id: 'request', type: 'user_input_request', requestId: 'r', questions: [{ id: 'q1', question: 'One' }, { id: 'q2', question: 'Two' }] }];
    client.answers.r = { selected: [], custom: '', index: 0 };
    for (let index = 0; index < 99; index++) stageTextAttachment(client, 'x', `file-${index}`, 'text/plain', `file-${index}`);
    const finishes: ((value: unknown) => void)[] = [];
    native.later = async input => obj(input).op === 'http' ? new Promise(resolve => finishes.push(resolve)) : original(input);
    const first = importDroppedThread(client, native, 'fleet:remote:first');
    obj(client.answers.r).index = 1;
    const second = importDroppedThread(client, native, 'fleet:remote:second').then(value => ({ value, error: '' }), error => ({ value: '', error: String(error) }));
    obj(client.answers.r).index = 0;
    finishes[0]!({ ok: true, generation: 2, value: { threadId: 'first', updatedAt: '2026-01-01', items: [] } });
    expect(await first).toContain('.jsonl](t3-context://v1/file/');
    obj(client.answers.r).index = 1;
    finishes[1]?.({ ok: true, generation: 2, value: { threadId: 'second', updatedAt: '2026-01-01', items: [] } });
    const refused = await second;
    expect(draftFiles(client.local)).toHaveLength(100);
    expect(refused.error).toContain('up to 100 files');
    expect(finishes).toHaveLength(1);
    expect(pendingThreadImports(client)).toBe(0);
  });
  test('a source response rechecks capacity filled while its read was pending', async () => {
    remote(); const { client, native } = await opened(), original = native.later.bind(native);
    for (let index = 0; index < 99; index++) stageTextAttachment(client, 'x', `file-${index}`, 'text/plain', `file-${index}`);
    let finish: (value: unknown) => void = () => {};
    native.later = async input => obj(input).op === 'http' ? new Promise(resolve => { finish = resolve; }) : original(input);
    const imported = importDroppedThread(client, native, 'fleet:remote:late');
    stageTextAttachment(client, 'x', 'late-file', 'text/plain', 'late-file');
    finish({ ok: true, generation: 2, value: { threadId: 'late', updatedAt: '2026-01-01', items: [] } });
    await expect(imported).rejects.toThrow('up to 100 files');
    expect(draftFiles(client.local)).toHaveLength(100);
    expect(pendingThreadImports(client)).toBe(0);
  });
});
