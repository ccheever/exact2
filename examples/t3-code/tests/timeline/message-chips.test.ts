// Sent-message chips resolved against HEAD's message shapes
// (OrchestrationMessageContext records, ChatAttachment), file-link paths, and
// the Markdown settings the chat reads.
import { describe, expect, test } from 'bun:test';
import { attachmentSize, chippedAttachmentIds, diffSchemeOf, fileLinkTarget, markdownEnv, markdownLinks, messageChips, middleTruncate } from '../../src/timeline/message-chips';
import type { T3Client } from '../../src/client/client';
import { markdownImageHref, markdownLinkHref, NO_HREF } from '../../src/media/media-views';

const item = (text: string, records: object[] = [], attachments: object[] = []) => ({ type: 'user_message', text, context: { version: 1, records }, attachments });

describe('sent-message context chips', () => {
  test('a file record bound to its attachment shows name, size and the two-line tooltip; its file row is left out', () => {
    const chips = messageChips(item('see [notes.md](t3-context://v1/file/file_abc) ok',
      [{ version: 1, kind: 'file', contextId: 'file_abc', label: 'notes.md', attachmentId: 'att-1', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 812 }],
      [{ type: 'file', id: 'att-1', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 812 }]), '/w', [], 'm1');
    expect(chips).toEqual([{ id: 'chip-0', href: 't3-context://v1/file/file_abc', kind: 'file', label: 'notes.md', size: '1 KB', tip: 'notes.md\n1 KB', detail: '', icon: 'markdown', target: 'att-1', owner: 'm1', menu: [] }]);
    expect([...chippedAttachmentIds(chips)]).toEqual(['att-1']);
  });
  test('a sent SVG is an image chip with the picture as its thumbnail, named and tipped as the file it opens (#21355)', () => {
    const svg = (mimeType: string, name = 'logo.svg') => messageChips(item(`see [${name}](t3-context://v1/file/file_svg)`,
      [{ version: 1, kind: 'file', contextId: 'file_svg', label: name, attachmentId: 'att-s', name, mimeType, sizeBytes: 2048 }],
      [{ type: 'file', id: 'att-s', name, mimeType, sizeBytes: 2048 }]), '', [], 'm1')[0];
    expect(svg('image/svg+xml')).toMatchObject({ kind: 'image', label: 'logo.svg', size: '2 KB', tip: 'File attachment, logo.svg, 2 KB', detail: 'logo.svg\n2 KB', target: 'att-s' });
    // Its chip shows it, so it has no file row of its own.
    expect([...chippedAttachmentIds([svg('image/svg+xml')])]).toEqual(['att-s']);
    expect(svg('application/octet-stream')).toMatchObject({ kind: 'image' });
    // A definite type other than SVG keeps the name's extension from counting.
    expect(svg('text/plain')).toMatchObject({ kind: 'file' });
    expect(svg('', 'logo.png')).toMatchObject({ kind: 'file' });
  });
  test('a reference without its record, or whose attachment is gone, reads "This context is no longer available."', () => {
    const chips = messageChips(item('[deploy](t3-context://v1/skill/skill_deploy) and ![a.png](t3-context://v1/image/image_x)',
      [{ version: 1, kind: 'image', contextId: 'image_x', label: 'a.png', attachmentId: 'gone', name: 'a.png', mimeType: 'image/png', sizeBytes: 10 }]), '', []);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip])).toEqual([
      ['unresolved', 'deploy', 'This context is no longer available.'], ['unresolved', 'a.png', 'This context is no longer available.']]);
  });
  test('skill, mention, thread and image records resolve to their own presentation', () => {
    const chips = messageChips(item('[Deploy](t3-context://v1/skill/skill_d) [a.ts](t3-context://v1/mention/mention_a) [Old](t3-context://v1/thread/thread_t) ![shot.png](t3-context://v1/image/image_s) [x](t3-context://v1/thread/thread_u)', [
      { version: 1, kind: 'skill', contextId: 'skill_d', label: 'Deploy', name: 'deploy' },
      { version: 1, kind: 'mention', contextId: 'mention_a', label: 'a.ts', path: 'src/a.ts' },
      { version: 1, kind: 'thread', contextId: 'thread_t', label: 'Old', environmentId: 'e', threadId: 't1', title: 'Old' },
      { version: 1, kind: 'thread', contextId: 'thread_u', label: 'x', environmentId: 'e', threadId: 'missing', title: 'x' },
      { version: 1, kind: 'image', contextId: 'image_s', label: 'shot.png', attachmentId: 'img', name: 'shot.png', mimeType: 'image/png', sizeBytes: 2048 },
    ], [{ type: 'image', id: 'img', name: 'shot.png', mimeType: 'image/png', sizeBytes: 2048 }]), '', [{ id: 't1', title: 'Renamed thread' }], 'm');
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.target])).toEqual([
      ['skill', 'Deploy', '$deploy', ''], ['mention', 'a.ts', 'src/a.ts', ''], ['thread', 'Renamed thread', 'Open thread', 't1'],
      ['image', 'shot.png', 'Image attachment, shot.png, 2 KB', 'img'], ['thread', 'x', 'Thread no longer available', '']]);
  });
  test('a video file chip leads with the film glyph; popover kinds carry their details', () => {
    const chips = messageChips(item('[clip.mov](t3-context://v1/file/file_v) [out](t3-context://v1/terminal/terminal_1)', [
      { version: 1, kind: 'file', contextId: 'file_v', label: 'clip.mov', attachmentId: 'v', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 3 * 1024 * 1024 },
      { version: 1, kind: 'terminal', contextId: 'terminal_1', label: 'out', terminalId: 't', terminalLabel: 'Terminal 1', lineStart: 2, lineEnd: 4, text: 'ls\nok' },
    ], [{ type: 'file', id: 'v', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 3 * 1024 * 1024 }]), '', []);
    expect(chips.map(chip => [chip.kind, chip.size, chip.tip, chip.detail])).toEqual([['video', '3.0 MB', 'clip.mov\n3.0 MB', ''], ['terminal', 'Lines 2–4', 'Terminal 1', 'ls\nok']]);
  });
  test('workspace file links carry their full path; web links are not chips and an image is a media chip', () => {
    const chips = messageChips({ text: 'Open [a](src/a.ts#L3), [b](./docs/../b.md:4), [abs](/etc/hosts), [web](https://x.dev) and ![i](pic.png)' }, '/repo/root', []);
    expect(chips.filter(chip => chip.kind === 'media').map(chip => [chip.href, chip.tip])).toEqual([['t3-image:t3-file:pic.png', '/repo/root/pic.png']]);
    expect(chips.filter(chip => chip.kind !== 'media').map(chip => [chip.href, chip.tip])).toEqual([
      ['t3-file:src/a.ts#L3', '/repo/root/src/a.ts:3'], ['t3-file:./docs/../b.md:4', '/repo/root/b.md:4'], ['t3-file:/etc/hosts', '/etc/hosts']]);
  });
  // react-markdown's defaultUrlTransform empties `fixture.txt:3` (its "scheme"
  // is fixture.txt), so only the bare path is a chip; the same table as markdown_links.rs relative_name_line_links_lose_their_target.
  test('relative name:line destinations lose their target, so only the bare path is a file chip', () => {
    const chips = messageChips({ text: 'Review [parser](fixture.txt:3), [fixture.txt](fixture.txt), [fixture.txt:3](fixture.txt:3).' }, '/repo', []);
    expect(chips.map(chip => [chip.href, chip.tip])).toEqual([['t3-file:fixture.txt', '/repo/fixture.txt']]);
    expect(messageChips({ text: '[a](C:\\repo\\a.ts:3) [m](Makefile:12) [x](xmpp:me@x.dev) [t](tel:1) [f](file:///repo/a.ts)' }, '/repo', []).map(chip => chip.href))
      .toEqual(['t3-file:C:\\repo\\a.ts:3', 't3-file:file:///repo/a.ts']);
    const table: [string, string][] = [
      ['fixture.txt:3', NO_HREF], ['Makefile:12', NO_HREF], ['a.ts:12:4', NO_HREF], ['fixture.txt', 't3-file:fixture.txt'], ['src/a.ts:3', 't3-file:src/a.ts:3'],
      ['./fixture.txt:3', 't3-file:./fixture.txt:3'], ['/repo/fixture.txt:3', 't3-file:/repo/fixture.txt:3'], ['a.ts#L3', 't3-file:a.ts#L3'],
      ['C:\\repo\\a.ts:3', 't3-file:C:\\repo\\a.ts:3'], ['C:/repo/a.ts', 't3-file:C:/repo/a.ts'], ['file:///repo/a.ts', 't3-file:file:///repo/a.ts'],
      ['https://example.com/a:b', 'https://example.com/a:b'], ['mailto:me@example.com', 'mailto:me@example.com'], ['xmpp:me@example.com', 'xmpp:me@example.com'],
      ['tel:123', NO_HREF], ['javascript:alert(1)', NO_HREF], ['data:text/plain,hi', NO_HREF], ['ftp://example.com/a', NO_HREF], ['#section', '#section'],
      ['//example.com/a', '//example.com/a'], ['t3-context://v1/file/f1', 't3-context://v1/file/f1'],
    ];
    expect(table.map(([href]) => [href, markdownLinkHref(href)])).toEqual(table);
    expect([markdownImageHref('a.png:3'), markdownImageHref('shots/a.png'), markdownImageHref('data:image/png;base64,AA'), markdownImageHref('#shot')]).toEqual(['', 't3-file:shots/a.png', '', '']);
  });
  test('an assistant quote reads its quote cut at 64 characters; its popover holds the quote, and "Go to source" opens the cited thread', () => {
    const quote = 'q'.repeat(70);
    const chips = messageChips({ text: `see [Assistant quote](t3-citation://v1/env/thread-9/msg-2?text=${quote}&start=0&end=70&prefix=&suffix=)` }, '', []);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.size, chip.target, chip.detail])).toEqual([['citation', `${'q'.repeat(64)}…`, 'q'.repeat(70), '', 'thread-9', 'msg-2']]);
  });
  test('formatAttachmentSize, middle truncation and path resolution follow the reference helpers', () => {
    expect([attachmentSize(0), attachmentSize(1025), attachmentSize(1024 * 1024)]).toEqual(['1 KB', '2 KB', '1.0 MB']);
    expect(middleTruncate('a-very-long-attachment-name-that-goes-on-and-on.png')).toBe('a-very-long-attachmen…-on-and-on.png');
    expect(fileLinkTarget('../x.md', '/a/b')).toBe('/a/x.md');
  });
});

describe('markdown link scan (extractMarkdownLinkHrefs)', () => {
  test('angle and bare destinations, titles, nested brackets and images', () => {
    expect(markdownLinks('[a](<x y.md>) [b]( c.ts "t" ) [d[e](f.md) ![g](h.png) [i](j.md \'k\')')).toEqual([
      { image: false, label: 'a', href: 'x y.md' }, { image: false, label: 'b', href: 'c.ts' }, { image: false, label: 'd[e', href: 'f.md' },
      { image: true, label: 'g', href: 'h.png' }, { image: false, label: 'i', href: 'j.md' }]);
  });
  test('a label stays on one line; an unclosed title or an empty destination is no link', () => {
    expect(markdownLinks('[a\n[b](c.md) [d\ne](f.md)')).toEqual([{ image: false, label: 'b', href: 'c.md' }]);
    expect(markdownLinks('[a](b.md "open) [c]() [d](<e.md>')).toEqual([]);
  });
  test('unmatched labels and destinations scan in linear time (#18387)', () => {
    const started = performance.now();
    expect(markdownLinks('[a]('.repeat(20_000))).toEqual([]);
    expect(markdownLinks('[a](file.md) '.repeat(20_000))).toHaveLength(20_000);
    expect(markdownLinks(`[${'['.repeat(20_000)}](`)).toEqual([]);
    expect(markdownLinks(`[a]( ${' '.repeat(20_000)}x "`.repeat(20))).toEqual([]);
    expect(performance.now() - started).toBeLessThan(1000);
  });
});

describe('Markdown settings', () => {
  const client = (settings: object) => ({ local: { clientSettings: settings } }) as unknown as T3Client;
  test('code size clamps to 10–18, word wrap and the monospace family come from Settings → Appearance', () => {
    expect(markdownEnv(client({ fontSizeCode: 22, wordWrap: false, fontFamilyCode: 'Menlo' }))).toEqual({ codeFont: '"Menlo", ui-monospace', codeSize: 18, wrap: false, chips: [], runCommands: [], threadGlyphs: [], inks: [] });
    // The default is the reference's code font as its renderer draws it (settings-appearance.ts DEFAULT_CODE_FONT).
    expect(markdownEnv(client({ fontSizeCode: 13, wordWrap: true, fontFamilyCode: '' }))).toEqual({ codeFont: 'Menlo, Consolas, "Liberation Mono", monospace', codeSize: 13, wrap: true, chips: [], runCommands: [], threadGlyphs: [], inks: [] });
    expect(markdownEnv({} as T3Client).codeSize).toBe(13);
  });
  test('diff colors default to red-green', () => {
    expect([diffSchemeOf(client({ diffColorScheme: 'blue-orange' })), diffSchemeOf(client({})), diffSchemeOf({} as T3Client)]).toEqual(['blue-orange', 'red-green', 'red-green']);
  });
});

describe('subagent divider (TimelineSystemDivider)', () => {
  test('"Subagent of" and the parent title are separate parts, the bot glyph leads and the pill opens the parent', async () => {
    const { subagentLead } = await import('../../src/sidebar/sidebar-lineage');
    const client = { threadId: 'kid', shell: { threads: [{ id: 'mom', title: 'Parent work', lineage: {} },
      { id: 'kid', title: 'Explore', createdAt: '2026-10-04T10:00:00.000Z', lineage: { relationshipToParent: 'subagent', parentThreadId: 'mom' } }] } } as unknown as T3Client;
    expect(subagentLead(client)).toEqual([expect.objectContaining({ kind: 'fork', icon: 'bot', title: 'Subagent of', detail: 'Parent work', actionLabel: 'Open parent thread', targetId: 'mom' })]);
  });
});

describe('Pull Requests diff colors and Markdown settings', () => {
  test('the list and the detail carry Appearance → Diff colors and the code settings', async () => {
    const { pullRequestsPage } = await import('../../src/pull-requests/pages-prs');
    const { pullRequestDetail } = await import('../../src/pull-requests/pages-pr-detail');
    const client = { local: { clientSettings: { diffColorScheme: 'blue-orange', fontSizeCode: 15, wordWrap: false, fontFamilyCode: '' } },
      config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [], threads: [] }, projectGroups: () => [], ready: false, environmentId: 'e' } as unknown as T3Client;
    const list = await pullRequestsPage(client, null, { open: false, refresh: 0, now: 0, selected: '', query: '', typed: false });
    expect(list.diffScheme).toBe('blue-orange');
    const detail = await pullRequestDetail(client, null, { selected: '', refresh: 0, now: 0 });
    expect([detail.diffScheme, detail.md]).toEqual(['blue-orange', { codeFont: 'Menlo, Consolas, "Liberation Mono", monospace', codeSize: 15, wrap: false, chips: [], runCommands: [], threadGlyphs: [], inks: [] }]);
  });
});

// Ported providerSkills.test.ts names; the pipeline assertion is clone-authored.
describe('formatProviderSkillDisplayName', () => {
  test('prefers the provider display name', async () => {
    const { formatProviderSkillDisplayName } = await import('../../src/timeline/message-chips');
    expect(formatProviderSkillDisplayName({ name: 'review-follow-up', displayName: 'Review Follow-up' })).toBe('Review Follow-up');
  });
  test('falls back to a title-cased skill name', async () => {
    const { formatProviderSkillDisplayName } = await import('../../src/timeline/message-chips');
    expect(formatProviderSkillDisplayName({ name: 'review-follow-up' })).toBe('Review Follow Up');
  });
  test('Markdown skill candidates exclude prices, invalid names and repeated names', async () => {
    const { markdownSkills } = await import('../../src/timeline/message-chips');
    expect(markdownSkills(['5', '1k', '1e9', '5foo', 'review-follow-up', 'review-follow-up', '_invalid', 'with space', 'unknown!'].map(name => ({ name })))).toEqual([
      { name: '5foo', displayName: '5foo' }, { name: 'review-follow-up', displayName: 'Review Follow Up' },
    ]);
  });
});

// MarkdownThreadLink → ThreadContextChip at v0.1.5 (#22621, #21505, #26239).
describe('thread links are thread chips', () => {
  const { resolveThreadLink, parseThreadLinkHref } = require('../../src/timeline/thread-links') as typeof import('../../src/timeline/thread-links');
  const { EnvironmentFleet } = require('../../src/connections/settings-b-fleet') as typeof import('../../src/connections/settings-b-fleet');
  const { markdownThreadGlyphs } = require('../../src/timeline/markdown-thread-glyphs') as typeof import('../../src/timeline/markdown-thread-glyphs');
  const entry = (environmentId: string, threads: object[], projects: object[] = []) => ({ key: environmentId, origin: `http://${environmentId}`, environmentId, phase: 'connected' as const,
    message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: {}, shell: { projects, threads } as never, scopes: [], error: '', requested: false });
  const local = { environmentId: 'env-a', shell: { threads: [{ id: 'thread-1', title: '  Live title ', projectId: 'p1' }, { id: 'a b', title: 'Spaced', projectId: 'p1' }],
    projects: [{ id: 'p1', title: 'Clone', projectIcon: { kind: 'emoji', emoji: '🧪' } }] } } as unknown as T3Client;
  const link = (href: string) => parseThreadLinkHref(href)!;
  test('a link to a thread here reads its live title, never the written one, and opens it', () => {
    const chips = messageChips(item('See [Old label](t3-thread://v1/thread-1), [x](t3-thread://v1/a%20b) and [Written](t3-thread://v2/env-a/thread-1)'), '', [], 'm', {}, local);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.target, chip.detail])).toEqual([
      ['thread', 'Live title', 'Open thread', 'thread-1', 'link'], ['thread', 'Spaced', 'Open thread', 'a b', 'link'], ['thread', 'Live title', 'Open thread', 'thread-1', 'link']]);
  });
  test('a link that resolves to nothing is unresolved: its written label, or why, which the tip says', () => {
    const text = '[Gone](t3-thread://v1/missing) <t3-thread://v2/env-a/missing> <t3-thread://v2/env-z/t9> [Elsewhere](t3-thread://v2/env-z/t8)';
    const chips = messageChips(item(text), '', [], 'm', {}, local);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.target])).toEqual([
      ['unresolved', 'Gone', 'Thread no longer available', ''],
      ['unresolved', 'Thread no longer available', 'Thread no longer available', ''],
      ['unresolved', 'Thread on another machine', 'Thread on another machine', ''],
      ['unresolved', 'Elsewhere', 'Thread on another machine', '']]);
  });
  test('an autolink shows the current title', () => {
    expect(messageChips(item('Started <t3-thread://v2/env-a/thread-1>.'), '', [], 'm', {}, local).map(chip => [chip.kind, chip.label]))
      .toEqual([['thread', 'Live title']]);
  });
  test('a v1 link this environment lacks opens the one match elsewhere; a v2 link is never looked for elsewhere', () => {
    const source = new EnvironmentFleet();
    source.entries.set('b', entry('env-b', [{ id: 'remote-1', title: 'Remote', projectId: 'pb' }], [{ id: 'pb', title: 'Remote project' }]));
    expect(resolveThreadLink(local, link('t3-thread://v1/remote-1'), source)).toMatchObject({ id: 'fleet:env-b:remote-1', remote: true, project: { id: 'pb' } });
    expect(resolveThreadLink(local, link('t3-thread://v1/thread-1'), source)).toMatchObject({ id: 'thread-1', remote: false });
    expect(resolveThreadLink(local, link('t3-thread://v2/env-b/remote-1'), source)).toMatchObject({ id: 'fleet:env-b:remote-1', environmentId: 'env-b' });
    expect(resolveThreadLink(local, link('t3-thread://v2/env-a/remote-1'), source)).toBeNull();
    expect(resolveThreadLink(local, link('t3-thread://v2/env-c/remote-1'), source)).toBeNull();
    source.entries.set('c', entry('env-c', [{ id: 'remote-1', title: 'Twin' }]));
    expect(resolveThreadLink(local, link('t3-thread://v1/remote-1'), source)).toBeNull();
    expect(resolveThreadLink(local, link('t3-thread://v2/env-c/remote-1'), source)).toMatchObject({ id: 'fleet:env-c:remote-1' });
  });
  test('a thread chip leads with its project icon, a link\'s or a sent thread context\'s', () => {
    const context = { id: 'chip-0', href: 't3-context://v1/thread/thread_t', kind: 'thread', label: 'Live title', size: '', tip: 'Open thread', detail: '', icon: '', target: 'thread-1', owner: 'm' };
    const glyphs = markdownThreadGlyphs(local, 'See [x](t3-thread://v1/thread-1) and [y](t3-thread://v1/missing)', [context]);
    expect(glyphs.map(glyph => [glyph.href, glyph.title, glyph.glyph.kind, glyph.glyph.emoji])).toEqual([
      ['t3-thread://v1/thread-1', 'Clone', 'emoji', '🧪'], ['t3-context://v1/thread/thread_t', 'Clone', 'emoji', '🧪']]);
  });
});

describe('skill chips open their details (#25432)', () => {
  const client = (skills: object[]) => ({ providerId: 'codex', config: { providers: [{ instanceId: 'codex', skills }] }, projection: { thread: {} },
    shell: { projects: [], threads: [] }, projectId: '' }) as unknown as T3Client;
  const skills = [{ name: 'review-code', displayName: 'Review code', description: ' Reads the diff. ', path: '/skills/review-code/SKILL.md' }, { name: 'deploy', shortDescription: 'Ships it' }];
  test('a `$skill` in the prose and a skill record carry the description and the instructions path', () => {
    const chips = messageChips(item('Use $review-code then [Deploy](t3-context://v1/skill/skill_d), not $5', [{ version: 1, kind: 'skill', contextId: 'skill_d', label: 'Deploy', name: 'deploy' }]),
      '', [], 'm', {}, client(skills));
    expect(chips.map(chip => [chip.href, chip.kind, chip.label, chip.detail, chip.target])).toEqual([
      ['t3-context://v1/skill/skill_d', 'skill', 'Deploy', 'Ships it', ''],
      ['t3-skill:$review-code', 'skill', 'Review code', 'Reads the diff.', '/skills/review-code/SKILL.md']]);
  });
  test('an unknown skill has no chip of its own, where a known one beside it has', () => {
    expect(messageChips(item('Use $unknown, then $review-code'), '', [], 'm', {}, client(skills)).map(chip => [chip.href, chip.kind]))
      .toEqual([['t3-skill:$review-code', 'skill']]);
  });
  test('each skill chip occurrence opens its own popover: its id names the message, the block and the run (#25432)', async () => {
    // One chip per href, so the same skill in two list items is one chip, and each block numbers its runs from 0.
    const chips = messageChips(item('- $review-code a\n- $review-code b\n\n$deploy when green'), '', [], 'm', {}, client(skills));
    expect(chips.map(chip => [chip.id, chip.href])).toEqual([['chip-0', 't3-skill:$review-code'], ['chip-1', 't3-skill:$deploy']]);
    const { value } = await import('../support/contract-action-fixture');
    const { appText } = await import('../support/app-files');
    const call = /SkillChip\(label=chip\.label, description=chip\.detail, path=chip\.target, id=(`[^`]+`)/.exec(appText('markdown.contract'))![1]!;
    const ids = [['0', chips[0]!], ['1', chips[0]!], ['3', chips[1]!]].map(([blockId, chip]) => value(call, { chip, blockId, run: { id: '0' } }));
    expect(ids).toEqual(['m-0-0', 'm-1-0', 'm-3-0']);
    expect(appText('markdown.contract')).toContain('ResolvedChip(run=run, chip=chip, urls=urls, local=local, blockId=blockId, color=color)');
  });
});

describe('the chips and rows a context reference draws', () => {
  test('an attachment chip without a target is no button, an image chip included (FileChip disabled)', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const body = contractComponent('markdown.contract', 'AttachmentChipBody');
    const button = (kind: string, target: string) => body.drawn('button press=local(', body.state(), { chip: { kind, target }, run: { id: '0' } });
    expect([button('image', 'att'), button('image', ''), button('file', ''), button('video', 'v')]).toEqual([true, false, false, true]);
  });
  test('a chip picture that fails to load tells the data module, which then draws the file chip', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const { appText } = await import('../support/app-files');
    const thumb = contractComponent('timeline-attachments.contract', 'ChipThumb'), sent: unknown[][] = [];
    const url = 'http://127.0.0.1:1/api/assets/att-svg?sig=1';
    thumb.call('failed', thumb.state(), { target: 'att-svg', url, local: (...args: unknown[]) => sent.push(args) }, ['decode failed']);
    expect(sent).toEqual([['chip-preview-failed', 'att-svg', url]]);
    expect(appText('markdown.contract').match(/ChipThumb\(url=entry\.url, target=chip\.target, local=local\)/g)).toHaveLength(1);
  });
  test('a secret request draws its card and no divider under it', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const content = contractComponent('timeline.contract', 'TimelineContent');
    const divider = (kind: string) => content.drawn('Icon(name="compacted"', content.state(), { message: { kind } });
    expect([divider('secret'), divider('notice')]).toEqual([false, true]);
  });
});

describe('a sent sound (#21363)', () => {
  test('its context reference is an audio chip, which the prose draws nothing for: the sound plays in its audio chip', async () => {
    const chips = messageChips(item('Listen to [memo.mp3](t3-context://v1/file/file_m)',
      [{ version: 1, kind: 'file', contextId: 'file_m', label: 'memo.mp3', attachmentId: 'att-m', name: 'memo.mp3', mimeType: 'audio/mpeg', sizeBytes: 900 }],
      [{ type: 'file', id: 'att-m', name: 'memo.mp3', mimeType: 'audio/mpeg; codecs=mp3', sizeBytes: 900 }]), '', [], 'm');
    expect(chips.map(chip => [chip.kind, chip.label, chip.target])).toEqual([['audio', 'memo.mp3', 'att-m']]);
    const { contractComponent } = await import('../support/contract-action-fixture');
    const { contractFns } = await import('../support/contract-fn-fixture');
    const resolved = contractComponent('markdown.contract', 'ResolvedChip'), inks = { ...contractFns('src/timeline/markdown-chip-inks.contract'), chipInks: [] };
    const drawn = (kind: string) => resolved.drawn('AttachmentChipBody(run=run, chip=chip', resolved.state(), { ...inks, chip: { ...chips[0], kind }, run: { id: '0' }, color: '' });
    expect([drawn('audio'), drawn('file')]).toEqual([false, true]);
  });
  test('the timeline leaves the reference out of the prompt, so nothing draws for it (renderContextReference returns null)', async () => {
    const { withoutSoundReferences } = await import('../../src/timeline/timeline-attachments');
    const sound = { context: { records: [{ kind: 'file', contextId: 'file_m', attachmentId: 'att-m' }, { kind: 'file', contextId: 'file_n', attachmentId: 'att-n' }] },
      attachments: [{ type: 'file', id: 'att-m', name: 'memo.mp3', mimeType: 'audio/mpeg' }, { type: 'file', id: 'att-n', name: 'notes.md', mimeType: 'text/markdown' }] };
    expect(withoutSoundReferences('Listen to [memo.mp3](t3-context://v1/file/file_m) and read [notes.md](t3-context://v1/file/file_n).', sound))
      .toBe('Listen to  and read [notes.md](t3-context://v1/file/file_n).');
    expect(withoutSoundReferences('[memo.mp3](t3-context://v1/file/file_m)', sound)).toBe('');
  });
});

// Chips drop their pill (T3 Code v0.1.5 ContextChip, #25066, #26306, #26379): no border or fill; the glyph and the
// medium label take the kind's ink, its accent raised to 5:1 on the theme (shared/chip-accents.ts).
describe('sent chips, file links and citations without a pill', () => {
  const inkFns = async () => {
    const { contractFns } = await import('../support/contract-fn-fixture');
    return contractFns('src/timeline/markdown-chip-inks.contract') as unknown as Record<string, (...args: unknown[]) => string>;
  };
  test('a PDF, Word, spreadsheet, slides or EPUB name gives its document kind; any other file has none', async () => {
    const inks = await inkFns();
    expect(['a.PDF', 'b.docx', 'c.rtf', 'd.xlsx', 'e.csv', 'f.pptx', 'g.epub', 'h.ts', 'report.pdf · L3'].map(name => inks.docKind!(name)))
      .toEqual(['doc-pdf', 'doc-word', 'doc-word', 'doc-spreadsheet', 'doc-spreadsheet', 'doc-slides', 'doc-epub', '', 'doc-pdf']);
  });
  test('the default theme\'s inks in the contract are those contextChipInks solves for it', async () => {
    const { chipInks } = await import('../../src/composer/composer-chip-presentation');
    const { CONTEXT_CHIP_ACCENTS } = await import('../../src/shared/chip-accents');
    const inks = await inkFns();
    const solved = chipInks({ local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: {} } } as unknown as T3Client);
    for (const kind of [...Object.keys(CONTEXT_CHIP_ACCENTS), 'invalid']) expect([kind, inks.defaultChipInk!(kind)]).toEqual([kind, solved[kind]!]);
    expect(inks.defaultChipInk!('neutral')).toBe('light-dark(#18181b, #f5f5f5)');
  });
  test('a chip reads the Markdown\'s inks, the default theme\'s without them', async () => {
    const inks = await inkFns();
    const theirs = [{ kind: 'image', ink: 'light-dark(#000001, #fffffe)' }];
    expect(inks.inkOf!(theirs, 'image')).toBe('light-dark(#000001, #fffffe)');
    expect(inks.inkOf!(theirs, 'thread')).toBe(inks.defaultChipInk!('thread'));
    expect(inks.inkOf!([], 'image')).toBe('light-dark(#006592, #249fdc)'); // image chips are blue (#26306)
    expect([inks.chipAccent!('review', ''), inks.chipAccent!('pr', 'merged'), inks.chipAccent!('pr', ''), inks.chipAccent!('link', ''), inks.chipAccent!('unresolved', '')])
      .toEqual(['review-comment', 'pr-merged', 'pull-request', 'mention', 'neutral']);
    expect([inks.runAccent!('context-review-comment'), inks.runAccent!('context-other')]).toEqual(['review-comment', 'neutral']);
    // A neutral chip draws in its text's colour (currentColor), a bubble's or a custom theme's too.
    expect([inks.inkIn!(theirs, 'neutral', 'light-dark(#0a0a0a, #fafafa)'), inks.inkIn!(theirs, 'image', '#123456')]).toEqual(['light-dark(#0a0a0a, #fafafa)', 'light-dark(#000001, #fffffe)']);
  });
  test('markdownEnv carries the theme\'s inks; a sent message\'s bubble on the other side of the canvas takes its own text colour', async () => {
    const { markdownChipInks, markdownEnv } = await import('../../src/timeline/message-chips');
    const client = { local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: { fontFamilyCode: '' } } } as unknown as T3Client;
    expect(markdownEnv(client).inks).toContainEqual({ kind: 'citation', ink: 'light-dark(#1f5cba, #5695f8)' });
    // The default theme's bubble is on the canvas's side in both halves, so its chips keep their accents.
    expect(markdownChipInks(client, true)).toEqual(markdownChipInks(client));
    expect(markdownEnv({ local: {} } as unknown as T3Client).inks).toEqual([]);
  });
  test('no chip in the chat Markdown draws a border or a fill', async () => {
    const { appText } = await import('../support/app-files');
    for (const file of ['markdown.contract', 'skill-chip.contract']) {
      const text = appText(file);
      for (const name of ['ContextChipBox', 'FileChipFace', 'ResolvedChip', 'PrContextChip', 'CitationContextChip', 'TerminalContextChip', 'AttachmentChipBody', 'ChipFace', 'SkillChip']) {
        const at = text.indexOf(`component ${name}\n`);
        if (at < 0) continue;
        const end = text.indexOf('\ncomponent ', at + 1), body = text.slice(at, end < 0 ? undefined : end);
        const trigger = body.split('\n').filter(line => /testId=`(context|file)-chip-/.test(line));
        for (const line of trigger) expect([name, /border-width=1|background-color=`|1c"/.test(line)]).toEqual([name, false]);
      }
    }
    expect(appText('markdown.contract')).not.toContain('TintedImageChip');
  });
});
