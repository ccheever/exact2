import { describe, expect, test } from 'bun:test';
import {
  contextId, contextLink, contextLinks, contextReferences, emptyText, entryIcon, expandCitations, fileLink, historyEntries, pathRows, promptLengthMessage,
  recallablePrompt, replaceContextReferences, scoreQueryMatch, skillChipLabels, skillRows, slashRows, stripContextReferences, threadRows,
} from '../../src/composer/composer-editor-menu';
import { composerChips } from '../../src/composer/composer-source-chips';
import { removeInlineContextReference } from '../../src/composer/composer-attachments';
import { foldedMessageSummary, plainContextReferences } from '../../src/timeline/user-message';
import { messageChips } from '../../src/timeline/message-chips';
import { withoutSoundReferences } from '../../src/timeline/timeline-attachments';
import { removeInlineContextReference as removeHandoffReference } from '../../src/pull-requests/pages-pr-handoffs-logic';
import { formatThreadLink, projectPendingUserInputAnswer } from '../../src/timeline/thread-links';
import { omitExpiredTerminalContexts } from '../../src/terminal/terminal-integrations';
import { composerEditorView, editorLocal, messageContext, threadContextRecords, withMessageContext } from '../../src/composer/composer-editor';
import { composerRichSource } from '../../src/composer/composer-rich-source';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { arr, type Obj } from '../../src/client/domain';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldObserved } from '../../src/composer/composer-field-bridge';

const codexCommands = [
  { name: 'compact', description: 'Summarize the conversation and reduce context usage' },
  { name: 'feedback', description: 'Send this thread and Codex logs to OpenAI' },
  { name: 'usage-limits', description: "Show this provider's usage limits" },
];
const slash = (overrides: object = {}) => slashRows({ query: '', atPromptStart: true, planModeUiEnabled: false, compactAvailable: true,
  driver: 'codex', slashCommands: codexCommands, skills: [], showSkillsInSlashMenu: true, ...overrides });

describe('command menus from the shared field', () => {
  async function opened() {
    const client = new T3Client(), native = new Backend(), disk = storage();
    await client.refresh(native, disk.files);
    await client.command('select-thread', 't1', '', 0, native, disk.files);
    await client.refresh(native, disk.files);
    let serial = 0;
    const observe = (value: string, start = value.length, end = start, composing = false) => {
      const owner = composerFieldOwner(client);
      composerFieldObserved(client, [{ generation: client.generation, snapshotOwner: client.snapshotOwner, owner,
        value, start, end, composing, live: true, focused: true, inputSerial: ++serial }, true], owner);
    };
    return { client, native, observe };
  }
  test('menus use accepted field text ahead of persisted draft and close for selections/composition', async () => {
    const { client, native, observe } = await opened();
    client.local.drafts[client.draftKey] = 'older saved prompt';
    observe('/mod');
    expect((await composerEditorView(client, native)).menu.rows.map(row => row.label)).toContain('/model');
    expect((await composerEditorView(client, null)).menu.rows.map(row => row.label)).toContain('/model');
    expect(client.draft).toBe('older saved prompt');
    observe('/mod', 1, 4);
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    observe('/mod', 4, 4, true);
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    observe('/mod');
    expect((await composerEditorView(client, native)).menu.open).toBe(true);
    client.threadId = 't2';
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
  });
  test('a suggestion from an older query is refused once the field holds a newer one (#18845)', async () => {
    const { client, native, observe } = await opened();
    const later = native.later.bind(native);
    let release = () => {};
    const held = new Promise<void>(resolve => { release = resolve; });
    let first = true;
    native.later = async (input: unknown) => {
      if ((input as Obj).op === 'editorPoke' && first) { first = false; await held; }
      return later(input);
    };
    observe('@Thre');
    const older = composerEditorView(client, native);
    await Bun.sleep(0);
    observe('@Thread');
    expect((await composerEditorView(client, native)).menu.rows.map(row => row.id)).toEqual(['thread:t2']);
    release();
    await older;
    await expect(editorLocal(client, native, 'pick', 'thread:t2', '')).rejects.toThrow('That suggestion is no longer available.');
    // Rows read for another workspace are refused too, though the query is the same.
    await composerEditorView(client, native);
    client.shell.projects[0]!.workspaceRoot = '/elsewhere';
    await expect(editorLocal(client, native, 'pick', 'thread:t2', '')).rejects.toThrow('That suggestion is no longer available.');
  });
  test('Escape keeps the same token dismissed until the caret leaves it', async () => {
    const { client, native, observe } = await opened();
    observe('/m');
    expect((await composerEditorView(client, native)).menu.open).toBe(true);
    await editorLocal(client, native, 'dismiss', '', '');
    observe('/mo');
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    observe('/mo ');
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    observe('/mod');
    expect((await composerEditorView(client, native)).menu.open).toBe(true);
  });
  test('rich code suppresses menus while plain mode treats the same source literally', async () => {
    const { client, native, observe } = await opened();
    const value = '```\n/mod\n```';
    observe(value, 8);
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    client.local.clientSettings.composerRichTextEnabled = false;
    expect((await composerEditorView(client, native)).menu.rows.map(row => row.label)).toContain('/model');
  });
  test('menus use live fence policy only for the exact observed field owner and source', async () => {
    const { client, native, observe } = await opened(), owner = composerFieldOwner(client), value = '```\n/mod';
    const project = (owner: string, value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project(owner, '\n/mod');
    project(owner, value, 'insertText');
    observe(value);
    expect((await composerEditorView(client, native)).menu.rows.map(row => row.label)).toContain('/model');
    project(owner, value + 'x', 'insertText');
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
    project('other', '\n/mod');
    project('other', value, 'insertText');
    expect((await composerEditorView(client, native)).menu.open).toBe(false);
  });
});

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
  // matchComposerThreadItems (bb7783e295): ⌘K's ranking, five at most, never the open, archived or subagent threads.
  test('threads rank as the palette ranks them, five at most, without the open, archived or subagent threads', () => {
    const now = Date.parse('2026-10-28T00:00:00Z'), day = 86_400_000;
    const at = (days: number) => new Date(now - days * day).toISOString();
    const threads = [
      { id: 'old', title: 'deploy notes', createdAt: at(30), latestUserAuthoredMessageAt: at(30) },
      { id: 'new', title: 'deploy notes v2', createdAt: at(1), latestUserAuthoredMessageAt: at(1) },
      { id: 'exact', title: 'deploy', createdAt: at(40), latestUserAuthoredMessageAt: at(40) },
      { id: 'open', title: 'deploy the open one', createdAt: at(0), latestUserAuthoredMessageAt: at(0) },
      { id: 'archived', title: 'deploy archive', createdAt: at(2), archivedAt: at(1) },
      { id: 'child', title: 'deploy subagent', createdAt: at(2), lineage: { relationshipToParent: 'subagent', parentThreadId: 'new' } },
      ...[1, 2, 3, 4].map(n => ({ id: `more-${n}`, title: `more deploy ${n}`, createdAt: at(50 + n) })),
    ];
    const ids = threadRows(threads, 'open', 'deploy', now).map(row => row.id);
    expect(ids).toHaveLength(5);
    expect(ids.every(id => !['thread:open', 'thread:archived', 'thread:child'].includes(id))).toBe(true);
    // The exact title leads; among equal matches the thread used last ranks first.
    expect(ids[0]).toBe('thread:exact');
    expect(ids.indexOf('thread:new')).toBeLessThan(ids.indexOf('thread:old'));
    // Typing is folded as the palette folds it.
    expect(threadRows(threads, 'open', '  DEPLOY V2 ', now)[0]?.id).toBe('thread:new');
  });

  test('threads lead (query required), then entries with basename and directory', () => {
    const now = Date.parse('2026-01-04T00:00:00Z');
    const threads = [{ id: 'a', title: 'fixture complete', updatedAt: '2026-01-02' }, { id: 'b', title: 'fixture complete fork', updatedAt: '2026-01-03' }, { id: 'c', title: 'other' }];
    expect(threadRows(threads, 'b', 'fix', now).map(row => [row.label, row.description, row.icon])).toEqual([['fixture complete', 'Thread', 'messages-square']]);
    expect(threadRows(threads, '', '', now)).toEqual([]);
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
    const client = { environmentId: 'env', snapshotDrafts: [], shell: { threads: [{ id: 't-1', title: 'fixture complete' }], projects: [] } } as unknown as T3Client;
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

// collectComposerContextReferences' expression, and upstream's recall with it (recallableComposerPrompt's chip loop at
// T3 Code 7c3b3b3605ef): what the one-pass reading must give.
const CONTEXT_LINK = /(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/[^\s)]{1,200})\)/g;
const parses = (href: string) => { const parts = href.slice(16).split('/'); return parts.length === 2 && /^[a-z][a-z0-9-]{0,39}$/.test(parts[0]!) && /^[a-z0-9_-]{1,128}$/i.test(parts[1]!); };
function upstreamRecallChips(text: string): string {
  let prompt = text;
  for (const match of [...text.matchAll(CONTEXT_LINK)].filter(each => parses(each[3]!)).reverse()) {
    let start = match.index!, end = start + match[0].length;
    if (prompt[end] === ' ') end += 1; else if (prompt[start - 1] === ' ') start -= 1;
    prompt = prompt.slice(0, start) + prompt.slice(end);
  }
  return prompt.trim();
}
/** Prompts of links, near-links and their pieces, the same each run. */
function corpus(count: number): string[] {
  let seed = 0x1151;
  const next = () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t ^= t + Math.imul(t ^ (t >>> 7), 61 | t); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32; };
  const pieces = ['!', '[', ']', '(', ')', ' ', ' ', '\n', 'a', 'Ab', 't3-context://v1/', 'image/img_1', 'file/f-2', 'x/y/z', 'Thread/T', '](t3-context://v1/',
    '[shot.png](t3-context://v1/image/img_1)', '![s](t3-context://v1/image/i)', '[A](t3-context://v1/thread/t_1)', ' [B](t3-context://v1/file/f)', '\t'];
  return Array.from({ length: count }, () => Array.from({ length: Math.floor(next() * 24) }, () => pieces[Math.floor(next() * pieces.length)]).join(''));
}

describe('context links in one pass', () => {
  const regexLinks = (text: string) => text.includes('](t3-context:') ? [...text.matchAll(CONTEXT_LINK)].map(match => ({ start: match.index!, end: match.index! + match[0].length,
    image: match[1] === '!', label: match[2]!, href: match[3]! })) : [];
  test('the same links as collectComposerContextReferences\' expression, its bounds included', () => {
    const label = (n: number) => 'l'.repeat(n), href = (n: number) => `image/${'i'.repeat(n - 6)}`;
    for (const text of [...corpus(4000), `[${label(512)}](t3-context://v1/image/i)`, `[${label(513)}](t3-context://v1/image/i)`,
      `![a](t3-context://v1/${href(200)})`, `![a](t3-context://v1/${href(201)})`, '!![a](t3-context://v1/image/i)', '[a\nb](t3-context://v1/image/i)',
      '[[a](t3-context://v1/image/i)', '[a](t3-context://v1/image/i x)', '[a](t3-context://v1/)', '[a](t3-context://v1/image/i\u00a0)']) {
      expect(contextLinks(text)).toEqual(regexLinks(text));
    }
  });
  test('recall, chip references and their replacement read as upstream\'s', () => {
    for (const text of corpus(4000)) {
      expect(recallablePrompt(text)).toBe(upstreamRecallChips(text.trim()));
      expect(stripContextReferences(text)).toBe(text.replace(CONTEXT_LINK, (source, _image, _label, href: string) => parses(href) ? '' : source));
      expect(replaceContextReferences(text, reference => `<${reference.kind}:${reference.label}>`))
        .toBe(text.replace(CONTEXT_LINK, (source, _image, label: string, href: string) => parses(href) ? `<${href.slice(16).split('/')[0]}:${label.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 200) || href.slice(16).split('/')[0]}>` : source));
    }
    // A reference the expression matches but whose href names no context stays (parseComposerContextHref).
    expect(recallablePrompt('see [a](t3-context://v1/x/y/z) now')).toBe('see [a](t3-context://v1/x/y/z) now');
  });
  // Each reader finds links as collectComposerContextReferences does (CONTEXT_LINK, a link's href running to its `)`),
  // then keeps its own rule for which links it takes.
  test('the other readers of context links take the links upstream\'s expression finds, each by its own rule', () => {
    const replaced = (text: string, take: (match: RegExpMatchArray) => string | null) =>
      text.replace(CONTEXT_LINK, (...args) => { const match = Object.assign([...args.slice(0, 4)], { index: args[4] }) as unknown as RegExpMatchArray; return take(match) ?? match[0]!; });
    const removed = (text: string, matches: RegExpMatchArray[]) => {
      let result = text, cursor = text.length;
      for (const match of [...matches].reverse()) {
        let start = match.index!, end = start + match[0].length;
        if (result[end] === ' ') end += 1; else if (result[start - 1] === ' ') start -= 1;
        result = `${result.slice(0, start)}${result.slice(end)}`; cursor = start;
      }
      return { result, cursor };
    };
    const kindOf = (href: string) => /^t3-context:\/\/v1\/([a-z][a-z0-9-]{0,39})\/[^\s)]{1,128}$/i.exec(href)?.[1];
    const target = (href: string) => parses(href) ? href.slice(16).split('/') as [string, string] : null;
    for (const text of corpus(3000)) {
      const matches = [...text.matchAll(CONTEXT_LINK)];
      expect(composerChips(text).filter(chip => chip.kind === 'context').map(chip => [chip.start, chip.end, chip.source]))
        .toEqual(matches.filter(match => parses(match[3]!)).map(match => [match.index, match.index! + match[0].length, match[0]]));
      expect(foldedMessageSummary('agent', text, '')).toBe(replaced(text, match => { const kind = kindOf(match[3]!); return kind === undefined ? null : match[2]!.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim() || kind; })
        .split(/\r?\n/).find(line => line.trim().length > 0)?.trim() ?? '');
      expect(plainContextReferences(text)).toBe(replaced(text, match => { const found = target(match[3]!); return found ? (match[2]!.trim() || found[1]).replace(/[\\`*_[\]()<>!|~#]/g, '\\$&') : null; }));
      for (const id of ['img_1', 'i', 't_1', 'f-2']) {
        const own = removed(text, matches.filter(match => target(match[3]!)?.[1] === id));
        expect(removeInlineContextReference(text, id)).toEqual(own.cursor === text.length && own.result === text ? { prompt: text, cursor: text.length }
          : own.cursor >= own.result.length ? { prompt: own.result.trimEnd(), cursor: own.result.trimEnd().length } : { prompt: own.result, cursor: own.cursor });
        const handoff = removed(text, matches.filter(match => match[3]!.split('/').pop() === id));
        expect(removeHandoffReference(text, id)).toBe(handoff.result === text && handoff.cursor === text.length ? text : handoff.cursor >= handoff.result.length ? handoff.result.trimEnd() : handoff.result);
      }
      const records = [{ contextId: 't_1', threadId: 'thread-1' }, { contextId: 'i', threadId: 'thread-i' }];
      expect(projectPendingUserInputAnswer(text, records)).toBe(replaced(text, match => {
        const found = target(match[3]!), record = found?.[0] === 'thread' ? records.find(record => record.contextId === found[1]) : undefined;
        return record ? formatThreadLink({ environmentId: null, threadId: record.threadId }, match[2]!) : null;
      }));
    }
  });
  // The expression took 100 to 125 ms here for each (500–600 ms a turn in the app); one pass takes under 5.
  test('a 256,000-character line of `![` takes each reader of context links a few milliseconds, with links after it too', () => {
    const line = '!['.repeat(128_000), timed = (run: () => unknown) => { const start = performance.now(); run(); return performance.now() - start; };
    const sound = { attachments: [{ type: 'file', id: 'att-s', name: 'memo.wav', mimeType: 'audio/wav' }], context: { records: [{ contextId: 'file_s', kind: 'file', attachmentId: 'att-s' }] } };
    for (const body of [line, `${line}](t3-context://v1/image/i) and [A](t3-context://v1/thread/t_1) [memo.wav](t3-context://v1/file/file_s)`]) {
      expect(timed(() => historyEntries([{ id: '1', kind: 'user', body }]))).toBeLessThan(30);
      expect(timed(() => contextReferences(body))).toBeLessThan(30);
      expect(timed(() => stripContextReferences(body))).toBeLessThan(30);
      expect(timed(() => composerChips(body))).toBeLessThan(30);
      expect(timed(() => foldedMessageSummary('agent', body, ''))).toBeLessThan(30);
      expect(timed(() => plainContextReferences(body))).toBeLessThan(30);
      expect(timed(() => removeInlineContextReference(body, 'i'))).toBeLessThan(30);
      expect(timed(() => messageChips({ text: body }, '/repo', []))).toBeLessThan(30);
      expect(timed(() => withoutSoundReferences(body, sound))).toBeLessThan(30);
      expect(timed(() => removeHandoffReference(body, 'i'))).toBeLessThan(30);
      expect(timed(() => projectPendingUserInputAnswer(body, [{ contextId: 't_1', threadId: 'thread-1' }]))).toBeLessThan(30);
      // A terminal chip whose output expired is cut from the message.
      expect(timed(() => omitExpiredTerminalContexts(new T3Client(), `${body} [Terminal 1](t3-context://v1/terminal/t_9)`, false))).toBeLessThan(30);
    }
    expect(recallablePrompt(line)).toBe(line);
  });
});

describe('# pull request menu', () => {
  const { pullRequestRows, pullRequestRecord, repositorySelector } = require('../../src/composer/composer-editor-menu') as typeof import('../../src/composer/composer-editor-menu');
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
    // A remote spells a space as `%20`, and `az repos pr list --repository` reads that literally.
    expect(repositorySelector({ provider: 'azure-devops', displayName: 'contoso/payments%20(eu)/_git/checkout%20(eu)', owner: 'contoso', name: 'checkout%20(eu)' })).toBe('checkout (eu)');
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
  test("the shared source uses selected workspace labels without sending a native stylist policy", async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    const codex = arr(native.config.providers)[0]!;
    codex.skills = skills;
    codex.workspaceSnapshots = [{ cwd: '/elsewhere', skills: [{ name: 'imagegen', displayName: 'Elsewhere' }] }];
    await client.refresh(native, disk.files);
    await client.command('select-thread', 't1', '', 0, native, disk.files);
    await client.refresh(native, disk.files);
    const synced = async () => {
      await composerEditorView(client, native);
      const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: composerFieldOwner(client), value: '$imagegen $openai-docs $frontend-design ' });
      return Object.fromEntries(view.chips.map(chip => [chip.name, chip.face.label]));
    };
    // Only a skill the provider offers users is a chip: the disabled `$frontend-design` stays text (composerSkillMeta).
    expect(await synced()).toEqual({ imagegen: 'Image Gen', 'openai-docs': 'OpenAI Docs' });
    // The thread's workspace (the project root, /repo) has its own discovered list: it wins, and its chips follow it.
    native.emit('config', { type: 'providerStatuses', payload: { providers: [{ ...codex, workspaceSnapshots: [{ cwd: '/repo', skills: [{ name: 'imagegen', displayName: 'Repo Image Gen' }] }] }] } });
    await client.refresh(native, disk.files);
    expect(await synced()).toEqual({ imagegen: 'Repo Image Gen' });
  });
});
