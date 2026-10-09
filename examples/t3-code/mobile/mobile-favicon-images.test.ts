import { describe, expect, test } from 'bun:test';
import { mobileCacheClear, mobileCacheClearKind } from './mobile-client-cache';
import { mobileFaviconResourceKey, mobileFaviconRevision } from './mobile-favicon-cache';
import { MobileFaviconImages, type MobileFaviconImageDemand, type MobileFaviconImageItem } from './mobile-favicon-images';
import type { Native } from './shared/protocol';

const target = { environmentId: 'favicon-images', cwd: '/workspace', faviconPath: null };
const old = 'https://example.test/old/icon.png', fresh = 'https://example.test/new/icon.png';
const inline = 'data:image/png;base64,QQ==';
const demand = (mountId = 'a', url: string | null = old, scopeRevision = 'catalog:1'): MobileFaviconImageDemand =>
  ({ mountId, target, url, scopeRevision });
const event = (owner: MobileFaviconImages, item: MobileFaviconImageItem, type: 'load' | 'error' = 'load') =>
  owner.event(item.mountId, item.requestKey, item.url, type).accepted;
const successful: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: { removed: 0 } }; } };

describe('projected favicon image requests, pinned365aa87982 semantics', () => {
  test('remote images stay mounted while loading and accept only a matching event', () => {
    const owner = new MobileFaviconImages(), row = owner.reconcile([demand()]).items[0]!;
    expect(row).toMatchObject({ key: mobileFaviconResourceKey(target), url: old, loaded: false, failed: false });
    expect(JSON.parse(row.requestKey)[0]).toBe(mobileFaviconRevision(target, old));
    expect(owner.event('other', row.requestKey, old, 'load').accepted).toBe(false);
    expect(owner.event('a', 'other', old, 'load').accepted).toBe(false);
    expect(owner.event('a', row.requestKey, fresh, 'load').accepted).toBe(false);
    expect(event(owner, row)).toBe(true);
    expect(owner.snapshot().items[0]!.loaded).toBe(true);
  });

  test('latest begun URL wins and its removal restores the last surviving URL', () => {
    const owner = new MobileFaviconImages();
    const a = owner.reconcile([demand()]).items[0]!;
    const b = owner.reconcile([demand(), demand('b', fresh)]).items[1]!;
    expect(event(owner, a)).toBe(false);
    expect(event(owner, a, 'error')).toBe(false);
    expect(event(owner, b)).toBe(true);
    owner.reconcile([demand()]);
    expect(event(owner, a)).toBe(true);
    expect(event(owner, b)).toBe(false);
  });

  test('duplicate URL counts survive one cleanup; stable reorder does not begin again', () => {
    const owner = new MobileFaviconImages();
    const [a, b, c] = owner.reconcile([demand(), demand('b', fresh), demand('c', fresh)]).items;
    owner.reconcile([demand('c', fresh), demand(), demand('b', fresh)]);
    expect(event(owner, a!)).toBe(false);
    owner.reconcile([demand(), demand('b', fresh)]);
    expect(event(owner, b!)).toBe(true);
    expect(event(owner, c!)).toBe(false);
    owner.reconcile([demand()]);
    expect(event(owner, a!)).toBe(true);
    owner.reconcile([]); owner.reconcile([]);
    expect(event(owner, a!)).toBe(false);
  });

  test('beginning a duplicate URL promotes its order over another surviving URL', () => {
    const owner = new MobileFaviconImages();
    const [a, b, c] = owner.reconcile([demand(), demand('b', fresh), demand('c')]).items;
    expect(event(owner, b!)).toBe(false);
    expect(event(owner, a!)).toBe(true);
    owner.reconcile([demand('b', fresh), demand('c')]);
    expect(event(owner, c!)).toBe(true);
    owner.reconcile([demand('b', fresh)]);
    expect(event(owner, b!)).toBe(true);
  });

  test('token rotation preserves local loaded/error status; a new revision resets it', () => {
    const owner = new MobileFaviconImages(), a = owner.reconcile([demand()]).items[0]!;
    event(owner, a);
    const b = owner.reconcile([demand('a', fresh)]).items[0]!;
    expect(b.loaded).toBe(true);
    expect(b.requestKey).not.toBe(a.requestKey);
    expect(event(owner, a, 'error')).toBe(false);
    expect(event(owner, b, 'error')).toBe(true);
    const c = owner.reconcile([demand()]).items[0]!;
    expect(c).toMatchObject({ loaded: false, failed: true });
    expect(owner.reconcile([demand('a', 'https://example.test/new/new.png')]).items[0])
      .toMatchObject({ loaded: false, failed: false });
  });

  test('an error only changes its own component status and evicts remembered loading', () => {
    const owner = new MobileFaviconImages(), [a, b] = owner.reconcile([demand(), demand('b')]).items;
    event(owner, a!);
    expect(owner.snapshot().items[1]!.loaded).toBe(false);
    event(owner, b!, 'error');
    expect(owner.snapshot().items[0]!.loaded).toBe(true);
    owner.reconcile([]);
    expect(owner.reconcile([demand('c')]).items[0]!.loaded).toBe(false);
  });

  test('inline starts loaded only with an admitted request; missing and absent URLs are fallback', () => {
    const owner = new MobileFaviconImages(), row = owner.reconcile([demand('a', inline)]).items[0]!;
    expect(row.loaded).toBe(true);
    expect(JSON.parse(row.requestKey)[0]).toBe(mobileFaviconResourceKey(target));
    owner.reconcile([]);
    expect(event(owner, row)).toBe(false);
    for (const url of [null, '', 'https://example.test/project-favicon-missing?token=x']) {
      const fallback = owner.reconcile([demand('a', url)]).items[0]!;
      expect(fallback).toMatchObject({ loaded: false, failed: false, requestKey: '', url: '' });
      expect(event(owner, fallback)).toBe(false);
    }
  });

  test('route removal and return cannot accept an old same-URL callback', () => {
    const owner = new MobileFaviconImages(), a = owner.reconcile([demand()]).items[0]!;
    event(owner, a); owner.reconcile([]);
    const b = owner.reconcile([demand()]).items[0]!;
    expect(b.loaded).toBe(true);
    expect(b.requestKey).not.toBe(a.requestKey);
    expect(event(owner, a, 'error')).toBe(false);
    expect(event(owner, b)).toBe(true);
  });

  test('catalog changes retire active requests and loaded memory, including return to old home', () => {
    const owner = new MobileFaviconImages(), a = owner.reconcile([demand()]).items[0]!;
    event(owner, a);
    const replacement = owner.reconcile([demand('a', old, 'catalog:2')]).items[0]!;
    expect(replacement.loaded).toBe(false);
    expect(event(owner, a)).toBe(false);
    owner.reconcile([]);
    expect(owner.reconcile([demand()]).items[0]!.loaded).toBe(false);
  });

  test('clear admission retires callbacks before its pending native reply and next root evaluation', async () => {
    const owner = new MobileFaviconImages(), a = owner.reconcile([demand()]).items[0]!;
    event(owner, a);
    let finish!: (value: unknown) => void;
    const native: Native = { available: true, watch() {}, later() { return new Promise(resolve => { finish = resolve; }); } };
    const pending = mobileCacheClear(native, { environmentId: target.environmentId });
    expect(event(owner, a)).toBe(false);
    expect(owner.snapshot().items).toEqual([]);
    const b = owner.reconcile([demand()]).items[0]!;
    expect(b.loaded).toBe(false);
    expect(event(owner, a)).toBe(false);
    finish({ ok: true, generation: 0, value: { removed: 0 } });
    await pending;
  });

  test('other environment and kind clears preserve images; favicon and global clears retire them', async () => {
    const owner = new MobileFaviconImages(), a = owner.reconcile([demand()]).items[0]!;
    await mobileCacheClear(successful, { environmentId: 'another-environment' });
    await mobileCacheClearKind(successful, 'vcs-refs');
    expect(event(owner, a)).toBe(true);
    await mobileCacheClearKind(successful, 'project-favicon');
    expect(event(owner, a)).toBe(false);
    const b = owner.reconcile([demand()]).items[0]!;
    await mobileCacheClear(successful);
    expect(event(owner, b)).toBe(false);
  });

  test('loaded-key LRU holds 256 keys, promotes on load but not on display', () => {
    const owner = new MobileFaviconImages();
    const icon = (index: number) => demand('a', `https://example.test/icon-${index}.png`);
    for (let i = 0; i < 256; i++) event(owner, owner.reconcile([icon(i)]).items[0]!);
    event(owner, owner.reconcile([icon(0)]).items[0]!);
    expect(owner.reconcile([icon(1)]).items[0]!.loaded).toBe(true);
    event(owner, owner.reconcile([icon(256)]).items[0]!);
    expect(owner.reconcile([icon(1)]).items[0]!.loaded).toBe(false);
    expect(owner.reconcile([icon(0)]).items[0]!.loaded).toBe(true);
    expect(owner.reconcile([icon(2)]).items[0]!.loaded).toBe(true);
  });

  test('duplicate mount input is one final demand; snapshots cannot mutate retained state', () => {
    const owner = new MobileFaviconImages(), view = owner.reconcile([demand(), demand('a', fresh)]);
    expect(view.items).toHaveLength(1);
    expect(view.items[0]!.url).toBe(fresh);
    view.items[0]!.loaded = true;
    expect(owner.snapshot().items[0]!.loaded).toBe(false);
    expect(owner.reconcile([demand('a', fresh)]).revision).toBe(view.revision);
    const saved = demand('b');
    owner.reconcile([saved]); saved.url = fresh; saved.target = { ...target, cwd: '/changed' };
    expect(owner.snapshot().items[0]!.url).toBe(old);
  });
});
