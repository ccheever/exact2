// The composer shelf's image thumbnails against T3 Code 1e2ecbd975 (MIT, LICENSE-T3), ChatComposer.tsx's
// expandedComposerImages strip: each frame holds a "Preview <name>" button (the thumbnail, cursor-zoom-in) that opens
// the expanded preview over the draft's images, then the "Remove <name>" button; the strip wraps (flex-wrap) unless it
// holds a SnapShot frame, when it scrolls sideways.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chipPopoverLocal } from './composer-chip-popover';
import { composerImagePreview, composerImageStep } from './composer-chip-details';

const source = (name: string) => readFileSync(new URL(`./${name}`, import.meta.url), 'utf8');
const component = (text: string, name: string) => {
  const start = text.indexOf(`component ${name}\n`);
  const end = text.indexOf('\ncomponent ', start + 1);
  return text.slice(start, end < 0 ? undefined : end);
};
/** The accessible names a component's lines carry, in tree order (what VoiceOver walks). */
const labels = (text: string) => [...text.matchAll(/aria-label=`([^`]*)`/g)].map(match => match[1]);

describe('the shelf thumbnails are reachable, named and ordered as the reference\'s', () => {
  test('a picked image: Preview <name> then Remove <name>, and the frame itself is unnamed', () => {
    const editor = source('composer-editor.contract');
    const tile = component(editor, 'ImageDraftTile'), button = component(editor, 'ImagePreviewButton');
    expect(tile.indexOf('ImagePreviewButton(')).toBeGreaterThan(0);
    expect(tile.indexOf('ImagePreviewButton(')).toBeLessThan(tile.indexOf('aria-label=`Remove ${name}`'));
    expect(tile.split('\n').find(line => line.includes('testId=`image-draft-${id}`'))).not.toContain('aria-label');
    expect(labels(button)).toEqual(['Preview ${name}']);
    expect(button).toContain('press=command("editorlocal:chip-image-open", id, "", 0)');
    expect(button).toContain('cursor="zoom-in"');
  });

  test('a SnapShot frame: Preview first, then its accessibility data, then Remove', () => {
    const tile = component(source('snapshot.contract'), 'SnapshotTile');
    const order = ['aria-label=`Preview ${image.name}`', 'popovertarget=`snapshot-contents-${image.id}`', 'aria-label=`Remove ${image.name}`'].map(part => tile.indexOf(part));
    expect(order.every(index => index > 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    // Focus inside the frame shows its Remove, as group-focus-within does.
    expect(tile).toMatch(/press=command\("editorlocal:chip-image-open", image\.id, "", 0\) focus=setHover\(true\) blur=setHover\(false\)/);
  });

  test('the strip scrolls only with a SnapShot frame, so plain images are not behind a scroll area', () => {
    const strip = component(source('snapshot.contract'), 'SnapshotDrafts');
    expect(strip).toContain('derive snapshots = images.filter((image) => image.snapshot).length > 0');
    expect(strip).toContain('flex-wrap=(snapshots ? "nowrap" : "wrap") overflow-x=(snapshots ? "scroll" : "visible")');
    expect(strip).not.toContain('overflow-x="scroll"');
  });
});

describe('a Preview press opens the expanded preview', () => {
  const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as unknown as Native;
  const fake = () => ({ snapshotOwner: 'draft:t1', draftKey: 'draft:t1', local: { drafts: {} },
    snapshotDrafts: [{ id: 'img-1', name: 'one.png' }, { id: 'img-2', name: 'two.png' }] }) as unknown as T3Client;

  test('at the pressed image, stepping over the draft\'s images', async () => {
    const client = fake();
    expect(await chipPopoverLocal(client, native, 'chip-image-open', 'img-2', '')).toBe('');
    expect(composerImagePreview(client)).toEqual([{ open: true, id: 'img-2', name: 'two.png', position: '(2/2)', many: true, width: 0, height: 0 }]);
    composerImageStep(client, 'chip-image-next');
    expect(composerImagePreview(client)[0]?.id).toBe('img-1');
    composerImageStep(client, 'chip-image-close');
    expect(composerImagePreview(client)).toEqual([]);
  });

  test('an image no longer attached says so and opens nothing', async () => {
    const client = fake();
    await expect(chipPopoverLocal(client, native, 'chip-image-open', 'gone', '')).rejects.toThrow('That image is no longer attached.');
    expect(composerImagePreview(client)).toEqual([]);
  });
});
