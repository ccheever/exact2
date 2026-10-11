import { describe, expect, test } from 'bun:test';
import {
  contextId, contextLink, contextReferences, emptyText, entryIcon, expandCitations, fileLink, historyEntries, pathRows, promptLengthMessage,
  recallablePrompt, scoreQueryMatch, skillChipLabels, skillRows, slashRows, threadRows,
} from './composer-editor-menu';
import { composerEditorView, messageContext, threadContextRecords, withMessageContext } from './composer-editor';
import { T3Client } from './client';
import { Backend, storage } from './client-fixture';
import { arr } from './domain';

const codexCommands = [
  { name: 'compact', description: 'Summarize the conversation and reduce context usage' },
  { name: 'feedback', description: 'Send this thread and Codex logs to OpenAI' },
  { name: 'usage-limits', description: "Show this provider's usage limits" },
];
const slash = (overrides: object = {}) => slashRows({ query: '', atPromptStart: true, planModeUiEnabled: false, compactAvailable: true,
  driver: 'codex', slashCommands: codexCommands, skills: [], showSkillsInSlashMenu: true, ...overrides });

describe('slash command menu', () => {
  test('lists /model then the provider commands, as the served build does', () => {
    expect(slash().map(row => [row.label, row.description])).toEqual([
      ['/model', 'Switch response model for this thread'],
      ['/compact', 'Summarize the conversation and reduce context usage'],
      ['/feedback', 'Send this thread and Codex logs to OpenAI'],
      ['/usage-limits', "Show this provider's usage limits"],
    ]);
  });
  test('plan mode adds /plan and /default; compact needs an idle conversation', () => {
    expect(slash({ planModeUiEnabled: true, compactAvailable: false }).map(row => row.label)).toEqual(['/model', '/plan', '/default', '/feedback', '/usage-limits']);
  });
  test('provider commands only open the prompt', () => {
    expect(slash({ atPromptStart: false }).map(row => row.label)).toEqual(['/model']);
  });
  test('ranks exact > prefix > boundary > includes > fuzzy, descriptions lower', () => {
    expect(slash({ query: 'f' }).map(row => row.label)[0]).toBe('/feedback');
    expect(slash({ query: 'limits' }).map(row => row.label)).toEqual(['/usage-limits']);
    expect(slash({ query: 'mdl' }).map(row => row.label)).toEqual(['/model']);
    expect(slash({ query: 'summarize' }).map(row => row.label)).toEqual(['/compact']);
    expect(slash({ query: 'zzz' })).toEqual([]);
    expect(slash({ query: '/model' }).map(row => row.label)[0]).toBe('/model');
  });
  test('scoreQueryMatch tiers', () => {
    const tiers = { exactBase: 0, prefixBase: 2, boundaryBase: 4, includesBase: 6, fuzzyBase: 100, boundaryMarkers: ['-'] };
    expect(scoreQueryMatch({ value: 'model', query: 'model', ...tiers })).toBe(0);
    expect(scoreQueryMatch({ value: 'usage-limits', query: 'usage', ...tiers })).toBe(2 + 7);
    expect(scoreQueryMatch({ value: 'usage-limits', query: 'limits', ...tiers })).toBe(4 + 12 + 6);
    expect(scoreQueryMatch({ value: 'feedback', query: 'dba', ...tiers })).toBe(6 + 3 * 2 + 5);
  });
  test('skills join the menu with a /skill: prefix and a source badge', () => {
    const skills = [{ name: 'pdf-tools', description: 'Work with PDFs', scope: 'user', enabled: true, path: '/u/.codex/skills/pdf' }];
    const rows = slash({ skills });
    const skill = rows.find(row => row.type === 'skill')!;
    expect([skill.prefix, skill.label, skill.badge, skill.badgeIcon, skill.insert]).toEqual(['/skill:', 'Pdf Tools', 'Personal', 'user-round', '$pdf-tools ']);
    expect(slash({ skills, showSkillsInSlashMenu: false }).some(row => row.type === 'skill')).toBe(false);
  });
  test('provider commands insert /name and a space', () => {
    expect(slash().find(row => row.label === '/feedback')!.insert).toBe('/feedback ');
  });
});

describe('$ skill menu', () => {
  const skills = [
    { name: 'deploy', shortDescription: 'Ship it', scope: 'repo', enabled: true, path: '/r/.agents/skills/deploy' },
    { name: 'figma', description: 'Design', scope: 'system', enabled: true, path: '/s' },
    { name: 'plugin-x', scope: '', enabled: true, path: '/u/.codex/plugins/x/skill' },
    { name: 'off', enabled: false, path: '/x' },
  ];
  test('rows carry the skill badge with a Skill suffix and skip disabled skills', () => {
    expect(skillRows(skills, '', 'codex').map(row => [row.label, row.description, row.badge])).toEqual([
      ['Deploy', 'Ship it', 'Repo Skill'], ['Figma', 'Design', 'System Skill'], ['Plugin X', 'Run provider skill', 'App Skill']]);
    expect(skillRows(skills, 'fig', 'codex').map(row => row.label)).toEqual(['Figma']);
  });
  test('empty states', () => {
    expect(emptyText('skill', '', false)).toBe('No skills found. Try / to browse provider commands.');
    expect(emptyText('path', '', false)).toBe('No matching files or folders.');
    expect(emptyText('slash-command', 'x', false)).toBe('No matching command.');
    expect(emptyText('pull-request', '', false)).toBe('Pull requests are not available for this project.');
    expect(emptyText('pull-request', '12', true)).toBe('No pull request matches 12.');
    expect(emptyText('pull-request', '', true)).toBe('No pull requests found in this repository.');
    expect(emptyText('pull-request', '12', true, true)).toBe('Pull requests could not be read for this project.');
  });
});

describe('@ mention menu', () => {
  test('threads lead (query required), then entries with basename and directory', () => {
    const threads = [{ id: 'a', title: 'fixture complete', updatedAt: '2026-01-02' }, { id: 'b', title: 'fixture complete fork', updatedAt: '2026-01-03' }, { id: 'c', title: 'other' }];
    expect(threadRows(threads, 'b', 'fix').map(row => [row.label, row.description, row.icon])).toEqual([['fixture complete', 'Thread', 'messages-square']]);
    expect(threadRows(threads, '', '')).toEqual([]);
    const rows = pathRows([{ path: 'fixture-result.md', kind: 'file' }, { path: 'src/app/main.ts', kind: 'file' }, { path: 'src', kind: 'directory' }]);
    expect(rows.map(row => [row.label, row.description, row.icon, row.insert])).toEqual([
      ['fixture-result.md', '', 'pierre-markdown', '[fixture-result.md](fixture-result.md) '],
      ['main.ts', 'src/app', 'pierre-typescript', '[main.ts](src/app/main.ts) '],
      ['src', '', 'folder', '[src](src) ']]);
    expect(entryIcon('a.md', 'file')).toEqual({ icon: 'pierre-markdown', light: '#199f43', dark: '#5ecc71' });
  });
  test('file links escape labels and encode destinations', () => {
    expect(fileLink('docs/a b (1).md')).toBe('[a b (1).md](docs/a%20b%20%281%29.md)');
    expect(fileLink('x/[y].md')).toBe('[\\[y\\].md](x/%5By%5D.md)');
  });
});

describe('context references', () => {
  test('thread ids are kind-scoped and links canonical', () => {
    const id = contextId('thread', 'd50c777e-d1a0-4a23-89a0-314f676cf000');
    expect(id).toBe('thread_d50c777e-d1a0-4a23-89a0-314f676cf000');
    expect(contextLink('thread', id, 'fixture [complete]')).toBe(`[fixture complete](t3-context://v1/thread/${id})`);
    expect(contextId('review-comment', 'pull-request-finding:42')).toMatch(/^review-comment_pull-request-finding-42-[0-9a-f]{16}$/);
    expect(contextReferences(`see [A](t3-context://v1/thread/${id}) and [A](t3-context://v1/thread/${id})`)).toEqual([{ kind: 'thread', id, label: 'A' }]);
  });
  test('a send carries the thread records its chips reference', () => {
    const client = { environmentId: 'env', shell: { threads: [{ id: 't-1', title: 'fixture complete' }], projects: [] } } as unknown as T3Client;
    const text = `look at ${contextLink('thread', 'thread_t-1', 'fixture complete')} please`;
    expect(threadContextRecords(client, text)).toEqual([{ version: 1, kind: 'thread', contextId: 'thread_t-1', label: 'fixture complete',
      environmentId: 'env', threadId: 't-1', title: 'fixture complete' }]);
    expect(messageContext(client, 'plain')).toBeUndefined();
    expect(withMessageContext(client, { type: 'message.dispatch', text }, text).context).toEqual({ version: 1, records: threadContextRecords(client, text) });
    const launch = withMessageContext(client, { initialMessage: { text, attachments: [] } }, text);
    expect((launch.initialMessage as { context: unknown }).context).toEqual({ version: 1, records: threadContextRecords(client, text) });
  });
});

describe('prompt length and history', () => {
  test('the 120,000-character limit message', () => {
    expect(promptLengthMessage('x'.repeat(120_000))).toBe('');
    expect(promptLengthMessage('x'.repeat(120_001))).toBe('Prompt is 1 character over the 120,000-character limit. Shorten or split it before sending.');
    expect(promptLengthMessage('x'.repeat(121_234))).toBe('Prompt is 1,234 characters over the 120,000-character limit. Shorten or split it before sending.');
  });
  test('citations count expanded', () => {
    const href = 't3-citation://v1/env/thread/message?text=' + 'q'.repeat(10) + '&start=0&end=10&prefix=&suffix=';
    expect(expandCitations(`a [Assistant quote](${href}) b`)).toContain('<assistant_citations>');
  });
  test('history strips context chips and app sends, collapsing consecutive duplicates', () => {
    expect(recallablePrompt('fix it [T](t3-context://v1/thread/thread_x) now')).toBe('fix it now');
    expect(recallablePrompt('PLEASE IMPLEMENT THIS PLAN:\n# Plan')).toBe('');
    expect(historyEntries([
      { id: '1', kind: 'user', body: 'fixture complete' }, { id: '2', kind: 'assistant', body: 'ok' },
      { id: '3', kind: 'user', body: 'again' }, { id: '4', kind: 'user', body: 'again' },
    ])).toEqual([{ id: '1', prompt: 'fixture complete' }, { id: '4', prompt: 'again' }]);
  });
});

describe('# pull request menu', () => {
  const { pullRequestRows, pullRequestRecord, repositorySelector } = require('./composer-editor-menu') as typeof import('./composer-editor-menu');
  const entries = [
    { projectId: 'p', repository: 'acme/app', number: 12, title: 'Fix login', url: 'https://x/12', headBranch: 'fix', baseBranch: 'main', state: 'open', isDraft: false },
    { projectId: 'p', repository: 'acme/app', number: 120, title: 'Draft work', url: 'https://x/120', headBranch: 'd', baseBranch: 'main', state: 'open', isDraft: true },
    { projectId: 'p', repository: 'acme/app', number: 7, title: 'Merged thing', url: 'https://x/7', headBranch: 'm', baseBranch: 'main', state: 'merged', isDraft: false },
    { projectId: 'q', repository: 'acme/app', number: 1, title: 'Other project', url: 'u', headBranch: 'h', baseBranch: 'b', state: 'closed', isDraft: false },
  ];
  test('rows filter by project and repository, the exact number first', () => {
    expect(pullRequestRows(entries, 'p', 'acme/app', '12').map(row => [row.label, row.description, row.icon])).toEqual([
      ['#12', 'Fix login', 'git-pull-request'], ['#120', 'Draft work', 'git-pull-request-draft']]);
    expect(pullRequestRows(entries, 'p', 'acme/app', 'merged').map(row => row.label)).toEqual(['#7']);
    expect(pullRequestRows(entries, 'p', 'acme/app', '').map(row => row.label)).toEqual(['#12', '#120', '#7']);
    expect(repositorySelector({ owner: 'acme', name: 'app' })).toBe('acme/app');
  });
  test('numbers match anywhere in the number, newest first after the exact one; text ranks by score', () => {
    const dated = [
      { projectId: 'p', repository: 'acme/app', number: 21, title: 'Old', updatedAt: '2026-01-01T00:00:00Z', headBranch: 'a', host: 'github.com' },
      { projectId: 'p', repository: 'acme/app', number: 2, title: 'Exact', updatedAt: '2025-01-01T00:00:00Z', headBranch: 'b', host: 'github.com' },
      { projectId: 'p', repository: 'acme/app', number: 32, title: 'Login page', updatedAt: '2026-06-01T00:00:00Z', headBranch: 'login-fix', host: 'github.com' },
      { projectId: 'p', repository: 'acme/app', number: 40, title: 'Body only match', updatedAt: '2026-07-01T00:00:00Z', headBranch: 'c', host: 'github.com' },
    ];
    expect(pullRequestRows(dated, 'p', 'acme/app', '2').map(row => row.label)).toEqual(['#2', '#32', '#21']);
    expect(pullRequestRows(dated, 'p', 'acme/app', 'login').map(row => row.label)).toEqual(['#32']);
    // A host that searched for us keeps rows whose match the row does not show, ranked last.
    expect(pullRequestRows(dated, 'p', 'acme/app', 'login', [{ host: 'github.com', searchesOnHost: true }]).map(row => row.label)).toEqual(['#32', '#40', '#21', '#2']);
  });
  test('a picked pull request becomes a review-comment record with PR metadata', () => {
    const record = pullRequestRecord({ number: 12, title: 'Fix  login', url: 'https://x/12', headBranch: 'fix', baseBranch: 'main', state: 'open', isDraft: false });
    expect(record.contextId).toMatch(/^review-comment_pr-reference-12-[0-9a-f]{16}$/);
    expect([record.label, record.filePath, record.rangeLabel]).toEqual(['#12', 'PR #12', 'Fix login']);
    expect(record.pullRequest).toEqual({ number: 12, title: 'Fix login', url: 'https://x/12', headBranch: 'fix', baseBranch: 'main', state: 'open', isDraft: false });
  });
});

// ComposerPromptEditorTiptap's skillLabelFor: a `$name` chip reads as the selected provider's skill of that
// name (formatProviderSkillDisplayName: its display name), for the workspace the composer is in.
describe('skill chip labels', () => {
  const skills = [
    { name: 'imagegen', displayName: 'Image Gen', scope: 'system', enabled: true },
    { name: 'openai-docs', displayName: '  OpenAI Docs ', scope: 'system', enabled: true },
    { name: 'frontend-design', scope: 'user', enabled: false },
    { name: 'imagegen', displayName: 'Second Image Gen', scope: 'user', enabled: true },
  ];
  test('each name reads as its first skill: the display name, trimmed, else the title-cased name', () => {
    expect(skillChipLabels(skills)).toEqual({ imagegen: 'Image Gen', 'openai-docs': 'OpenAI Docs', 'frontend-design': 'Frontend Design' });
    expect(skillChipLabels([])).toEqual({});
    // Exact names, as the reference's `find`: a chip `$imagegen` does not take `ImageGen`'s label.
    expect(skillChipLabels([{ name: 'ImageGen', displayName: 'Other' }])).toEqual({ ImageGen: 'Other' });
  });
  test("the editor is synced with the selected provider's labels, the workspace's own list first", async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    const codex = arr(native.config.providers)[0]!;
    codex.skills = skills;
    codex.workspaceSnapshots = [{ cwd: '/elsewhere', skills: [{ name: 'imagegen', displayName: 'Elsewhere' }] }];
    await client.refresh(native, disk.files);
    await client.command('select-thread', 't1', '', 0, native, disk.files);
    await client.refresh(native, disk.files);
    const synced = async () => { await composerEditorView(client, native); return native.calls.findLast(call => call.op === 'editorSync')!.skills; };
    expect(await synced()).toEqual({ imagegen: 'Image Gen', 'openai-docs': 'OpenAI Docs', 'frontend-design': 'Frontend Design' });
    // The thread's workspace (the project root, /repo) has its own discovered list: it wins.
    native.emit('config', { type: 'providerStatuses', payload: { providers: [{ ...codex, workspaceSnapshots: [{ cwd: '/repo', skills: [{ name: 'imagegen', displayName: 'Repo Image Gen' }] }] }] } });
    await client.refresh(native, disk.files);
    expect(await synced()).toEqual({ imagegen: 'Repo Image Gen' });
  });
});
