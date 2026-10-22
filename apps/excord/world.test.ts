import { expect, test } from 'bun:test';
import { answer } from './app';
import type { Args, Result, Source, Storage, Store } from './app.contract.d.ts';
import { createWorld, WORLD_NOW } from './world';
import type { Message } from './world';
import { createRenderer, renderMessage } from './render';
import { generate, photos, emojiNames } from './fixture';
import { decodePng } from '../../scripts/png.mjs';

const call = <S extends Source>(source: S, args: Args<S>): Promise<Result<S>> =>
  Promise.resolve(answer(source, args, {} as Store, {} as Storage));
const ids = (messages: { id: string }[]) => messages.map(m => m.id);
function sample(w = createWorld(), patch: Partial<Message> = {}): Message {
  return { id: '00000000001000000001', channel_id: 'general', author: w.me(), content: 'hello',
    timestamp: WORLD_NOW, edited_timestamp: null, attachments: [], embeds: [], reactions: [],
    type: 0, state: 'sent', revision: 0, ...patch };
}

test('deterministic general generation covers 400 days and reports its cost', () => {
  const start = performance.now();
  const fixture = generate('general');
  console.log(`general: ${fixture.messages.length} messages generated in ${(performance.now() - start).toFixed(2)} ms (Bun)`);
  const all = fixture.messages;
  expect(all).toHaveLength(10000);
  expect(all[9999].timestamp).toBe(WORLD_NOW - 300000);
  expect(all[9999].timestamp - all[0].timestamp).toBe(400 * 86400000);
  expect(all.every((m, i) => /^\d{19}$/.test(m.id) && (!i || m.id > all[i - 1].id))).toBe(true);
  expect(generate('general')).toEqual(fixture);
  expect(all.some(m => m.message_reference && m.id > m.message_reference.message_id)).toBe(true);
  expect(new Set(all.map(m => Math.floor(m.timestamp / 86400000))).size).toBeGreaterThan(200);
});

test('history pages are chronological and exclude before/after anchors', () => {
  const w = createWorld(), all = w.latest('general', 20000).messages;
  expect(w.latest('general', 200)).toEqual({ messages: all.slice(-200), hasBefore: true, hasAfter: false });
  expect(w.around('general', all[5000].id, 200).messages).toEqual(all.slice(4900, 5100));
  expect(w.around('general', all[0].id, 200).messages).toEqual(all.slice(0, 100));
  expect(w.around('general', '9999999999999999999', 200).messages).toEqual(all.slice(-101));
  expect(w.before('general', all[5000].id, 200).messages).toEqual(all.slice(4800, 5000));
  expect(w.after('general', all[5000].id, 200).messages).toEqual(all.slice(5001, 5201));
  expect(w.previous('general', all[5000].id)).toBe(all[4999]);
  expect(w.previous('general', all[0].id)).toBeUndefined();
  const deleted = all[5000].id;
  w.inject('general', 'delete-above', all[5003].id, WORLD_NOW);
  expect(w.around('general', deleted, 200).messages[100].id).toBe(all[5001].id);
});

test('source windows pin, overlap while paging, echo inputs and have only their declared fields', async () => {
  const empty = await call('loadWindow', ['', '', 0]);
  expect(empty).toEqual({ channel: '', anchor: '', rows: [], hasBefore: false, hasAfter: false, firstId: '', lastId: '', pin: '' });
  const latest = await call('loadWindow', ['general', '', 0]);
  expect(latest.rows).toHaveLength(200);
  expect(latest.pin).toBe(latest.rows[100].id);
  const pinned = await call('loadWindow', ['general', latest.pin, 999]);
  expect(pinned.rows).toEqual(latest.rows);
  expect(pinned.rows[0]).toBe(latest.rows[0]);
  expect(pinned.anchor).toBe(latest.pin);
  const older = await call('loadWindow', ['general', latest.firstId, 0]);
  expect(ids(older.rows.slice(100))).toEqual(ids(latest.rows.slice(0, 100)));
  const newer = await call('loadWindow', ['general', older.lastId, 0]);
  expect(ids(newer.rows.slice(0, 101))).toEqual(ids(older.rows.slice(99)));
  expect(newer.rows.length).toBeLessThanOrEqual(200);
  expect(older.hasBefore && older.hasAfter).toBe(true);
  expect(latest.hasAfter).toBe(false);
  const short = await call('loadWindow', ['announcements', '', 0]);
  expect((await call('loadWindow', ['announcements', short.pin, 0])).rows).toEqual(short.rows);
  expect(Object.keys(latest).sort()).toEqual(['anchor', 'channel', 'firstId', 'hasAfter', 'hasBefore', 'lastId', 'pin', 'rows']);
  expect(Object.keys(latest.rows[0]).sort()).toEqual(['at', 'author', 'authorId', 'blocks', 'bot', 'edited', 'embeds', 'grouped', 'id',
    'initials', 'jumbo', 'key', 'media', 'mentionsMe', 'mine', 'plain', 'prevAt', 'reactions', 'reply', 'state', 'system', 'tint']);
  expect(await call('sendMessage', ['general', '  ', '', WORLD_NOW])).toEqual({ ok: false, error: 'A message needs some text.' });
  expect(await call('noteJump', ['general', latest.firstId])).toEqual({ ok: true, error: '' });
});

test('grouping stops at exactly seven minutes, replies, system rows and failures', () => {
  const w = createWorld(), previous = sample(w, { timestamp: WORLD_NOW - 7 * 60000 });
  expect(renderMessage(sample(w, { timestamp: WORLD_NOW - 1 }), previous, w).grouped).toBe(true);
  expect(renderMessage(sample(w), previous, w).grouped).toBe(false);
  const close = { ...previous, timestamp: WORLD_NOW - 1000 };
  for (const patch of [{ type: 19 }, { type: 7 }, { message_reference: { message_id: previous.id } }, { author: w.user('101')! }]) {
    expect(renderMessage(sample(w, patch), close, w).grouped).toBe(false);
  }
  expect(renderMessage(sample(w), { ...close, state: 'failed' }, w).grouped).toBe(false);
  expect(renderMessage(sample(w), { ...close, type: 19 }, w).grouped).toBe(false);
  expect(renderMessage(sample(w), undefined, w).prevAt).toBe(0);
});

test('send, failure, retry, confirmation, read state and deletion preserve local identity', () => {
  const w = createWorld(), render = createRenderer(w);
  w.markRead('general');
  const m = w.send('general', 'first', '', WORLD_NOW), key = m.id;
  const next = w.send('general', 'second', '', WORLD_NOW);
  expect(render(m).state).toBe('pending');
  w.markRead('general');
  w.inject('general', 'fail-next', '', WORLD_NOW);
  w.settle('general');
  expect(m.state).toBe('failed');
  expect(next.state).toBe('sent');
  expect(w.channelList()[0].unread).toBe(0);
  w.retry('general', key);
  expect(m.state).toBe('pending');
  w.settle('general');
  expect(render(m).key).toBe(key);
  expect(m.id).not.toBe(key);
  expect(render(m).state).toBe('sent');
  expect(w.latest('general', 2).messages).toEqual([m, next]);
  expect(w.message('general', key)).toBe(m);
  w.edit('general', m.id, 'changed');
  expect(render(m).edited).toBe(true);
  expect(render(m).plain).toBe('changed');
  w.react('general', m.id, '👍');
  expect(render(m).reactions[0]).toMatchObject({ count: 1, me: true });
  w.react('general', m.id, '👍');
  expect(render(m).reactions).toEqual([]);
  w.react('general', m.id, '501');
  expect(render(m).reactions[0]).toMatchObject({ id: '501', src: 'assets/emoji/ferris.png' });
  w.remove('general', key);
  expect(w.message('general', m.id)).toBeUndefined();
  expect(w.channelList()[0].pending).toBe(0);
  const someoneElse = w.latest('general', 200).messages.find(m => m.author.id !== 'me')!;
  expect(() => w.remove('general', someoneElse.id)).toThrow('Only your own');
});

test('above-viewport events target exactly three rows before near and invalidate cached dependents', () => {
  for (const what of ['edit-above', 'delete-above', 'react-above', 'embed-above']) {
    const w = createWorld(), render = createRenderer(w), rows = w.latest('general', 100).messages;
    const target = rows[47], near = rows[50], old = render(target), version = w.channelList()[0].version;
    w.inject('general', what, near.nonce ?? near.id, WORLD_NOW);
    expect(w.channelList()[0].version).toBe(version + 1);
    expect(render(near, rows[49]).id).toBe(near.id);
    if (what === 'delete-above') expect(w.message('general', target.id)).toBeUndefined();
    else {
      const changed = render(target);
      expect(changed).not.toBe(old);
      if (what === 'edit-above') { expect(changed.plain.split('\n').length).toBeGreaterThan(4); expect(changed.edited).toBe(true); }
      if (what === 'react-above') expect(changed.reactions.length).toBe(old.reactions.length + 1);
      if (what === 'embed-above') expect(changed.embeds[changed.embeds.length - 1].image[0].fitWidth).toBe(320);
    }
  }
  const w = createWorld(), render = createRenderer(w);
  const first = w.send('help', 'reference', '', WORLD_NOW);
  const reply = w.send('help', 'reply', first.id, WORLD_NOW + 1);
  expect(render(reply).reply[0].preview).toBe('reference');
  w.edit('help', first.id, 'new reference');
  expect(render(reply).reply[0].preview).toBe('new reference');
  w.remove('help', first.id);
  expect(render(reply).reply[0].missing).toBe(true);
  const tail = w.latest('help', 20).messages;
  w.inject('help', 'edit-above', 'unknown', WORLD_NOW);
  expect(w.message('help', tail[tail.length - 11].id)?.edited_timestamp).toBe(WORLD_NOW);
});

test('unread boundaries, mention counts, typing and burst arrival', () => {
  const w = createWorld(), channels = w.channelList();
  expect(channels.map(c => c.total)).toEqual([10000, 1200, 300, 60]);
  expect(channels.map(c => c.unread)).toEqual([37, 260, 0, 0]);
  expect(channels[0].mentions).toBe(1);
  expect(w.latest('help', 200).messages.some(m => m.id === channels[1].firstUnread)).toBe(false);
  expect(w.around('help', channels[1].firstUnread, 200).messages[100].timestamp).toBe(channels[1].firstUnreadAt);
  w.markRead('general');
  expect(w.channelList()[0]).toMatchObject({ unread: 0, mentions: 0, firstUnread: '', firstUnreadAt: 0 });
  w.inject('general', 'typing', '', WORLD_NOW);
  expect(w.channelList()[0].typing).toBe('Ada is typing…');
  w.inject('general', 'typing', '', WORLD_NOW);
  expect(w.channelList()[0].typing).toBe('Ada and Grace are typing…');
  w.inject('general', 'message', '', WORLD_NOW);
  expect(w.channelList()[0].typing).toBe('Grace is typing…');
  w.inject('general', 'burst', '', WORLD_NOW + 1000);
  expect(w.channelList()[0]).toMatchObject({ unread: 7, typing: '' });
  w.inject('general', 'typing', '', WORLD_NOW);
  w.inject('general', 'typing-stop', '', WORLD_NOW);
  expect(w.channelList()[0].typing).toBe('');
});

test('Markdown becomes typed blocks, marked runs, resolved mentions and timestamp formats', () => {
  const w = createWorld();
  const content = [
    '# Title', '## Second', '### Third', '-# A quiet note',
    '**bold** *italic* __underline__ ~~strike~~ ||secret **bold secret**|| and ||other||',
    '`inline` [**label**](https://example.com) https://example.com/plain',
    '<@me> <@101> <#help> <@&201> @everyone @here <:ferris:501> <:unknown:999>',
    'line one\nline two', '', '> quoted **text**', '',
    '- first\n  - nested\n\n  later paragraph\n- second', '',
    '3. third\n4. fourth', '', '```rust\nlet n = 2;\n```',
    ['t', 'T', 'd', 'D', 'R', 'f', 'F'].map(s => `<t:1792692000:${s}>`).join(' '),
    '', '>>> long quote\nsecond quoted line',
  ].join('\n');
  const row = renderMessage(sample(w, { content, edited_timestamp: WORLD_NOW + 1 }), undefined, w);
  const runs = row.blocks.flatMap(b => b.runs);
  expect(row.blocks.filter(b => b.kind === 'heading').map(b => b.level)).toEqual([1, 2, 3]);
  expect(row.blocks.some(b => b.kind === 'subtext')).toBe(true);
  expect(row.blocks.find(b => b.kind === 'code')).toMatchObject({ lang: 'rust', text: 'let n = 2;', runs: [] });
  expect(row.blocks.filter(b => b.kind === 'item').map(b => [b.marker, b.level])).toEqual([['•', 0], ['•', 1], ['', 0], ['•', 0], ['3.', 0], ['4.', 0]]);
  expect(row.blocks.filter(b => b.quote).length).toBe(2);
  for (const mark of ['bold', 'italic', 'underline', 'strike'] as const) expect(runs.some(r => r[mark])).toBe(true);
  const secrets = runs.filter(r => r.text.includes('secret'));
  expect(secrets[0].spoiler).toBe(secrets[1].spoiler);
  expect(runs.find(r => r.text === 'other')?.spoiler).not.toBe(secrets[0].spoiler);
  expect(runs.find(r => r.text === 'label')).toMatchObject({ kind: 'link', bold: true, href: 'https://example.com' });
  expect(runs.find(r => r.kind === 'code')?.text).toBe('inline');
  expect(runs.find(r => r.self)).toMatchObject({ kind: 'user', text: '@Charlie' });
  expect(row.plain).toContain('@Ada #help @Maintainers @everyone @here :ferris:');
  expect(row.mentionsMe).toBe(true);
  expect(runs.filter(r => r.kind === 'emoji').map(r => r.href)).toEqual(['assets/emoji/ferris.png', '']);
  expect(runs.filter(r => r.kind === 'time').map(r => r.format)).toEqual(['time-short', 'time-medium', 'date-short', 'date-long', 'relative', 'date-long', 'time-short', 'date-full', 'time-short']);
  expect(runs.filter(r => r.kind === 'time').every(r => r.at === 1792692000000)).toBe(true);
  expect(row.plain).toContain('line one\nline two');
  expect(runs[runs.length - 1]).toMatchObject({ kind: 'edited', text: ' (edited)' });
  expect(new Set(runs.map(r => r.id)).size).toBe(runs.length);
});

test('jumbo emoji, media reservation, embeds, reactions, join rows and missing replies', () => {
  const w = createWorld();
  const row = renderMessage(sample(w, { content: '🎉 <:ferris:501>',
    attachments: [
      { id: 'large', url: 'a', filename: 'large.png', width: 1200, height: 600, spoiler: true },
      { id: 'small', url: 'b', filename: 'small.png', width: 64, height: 64, spoiler: false },
      { id: 'unknown', url: 'c', filename: 'unknown.png', spoiler: false },
    ], embeds: [{ type: 'rich', color: 0x0123ab, title: 'Title', description: '**description**',
      image: { url: 'image', width: 800, height: 1600 }, thumbnail: { url: 'thumb', width: 32, height: 32 } }],
    message_reference: { message_id: 'deleted' },
  }), undefined, w);
  expect(row.jumbo).toBe(true);
  expect(row.blocks.map(b => b.kind)).toEqual(['emoji', 'paragraph']);
  expect(row.blocks[0].runs[0].href).toBe('assets/emoji/ferris.png');
  expect(row.media.map(m => [m.fitWidth, m.fitHeight])).toEqual([[320, 160], [64, 64], [0, 0]]);
  expect(row.embeds[0].color).toBe('#0123ab');
  expect(row.embeds[0].image[0]).toMatchObject({ fitWidth: 160, fitHeight: 320 });
  expect(row.embeds[0].description[0].runs[0].bold).toBe(true);
  expect(row.reply[0].missing).toBe(true);
  expect(renderMessage(sample(w, { type: 7, author: w.user('101')! }), undefined, w).system).toBe('Ada joined the server.');
  expect(renderMessage(sample(w, { content: '`<@me>`' }), undefined, w).mentionsMe).toBe(false);
});

test('bundled PNGs decode at their real sizes, stay small and include deliberate metadata mismatches', async () => {
  for (const p of photos) {
    const bytes = await Bun.file(new URL(`./assets/media/${p.name}.png`, import.meta.url)).bytes();
    const decoded = decodePng(Buffer.from(bytes));
    expect([decoded.width, decoded.height]).toEqual([p.width, p.height]);
    expect(bytes.length).toBeLessThan(30000);
  }
  for (const name of emojiNames) {
    const decoded = decodePng(Buffer.from(await Bun.file(new URL(`./assets/emoji/${name}.png`, import.meta.url)).bytes()));
    expect([decoded.width, decoded.height]).toEqual([64, 64]);
  }
  const attachments = generate('random').messages.flatMap(m => m.attachments);
  const mismatched = attachments.filter(a => a.width && a.width !== photos.find(p => a.filename === `${p.name}.png`)!.width);
  expect(mismatched.length / attachments.length).toBeCloseTo(0.2, 1);
  expect(attachments.filter(a => !a.width).length / attachments.length).toBeCloseTo(0.1, 1);
});
