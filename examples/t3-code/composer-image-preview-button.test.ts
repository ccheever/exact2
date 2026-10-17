// The composer shelf's image thumbnails against T3 Code 1e2ecbd975 (MIT, LICENSE-T3), ChatComposer.tsx's
// expandedComposerImages strip: each frame holds a "Preview <name>" button (the thumbnail, cursor-zoom-in) that opens
// the expanded preview over the draft's images, then the "Remove <name>" button; the strip wraps (flex-wrap) unless it
// holds a SnapShot frame, when it scrolls sideways.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chipPopoverLocal } from './composer-chip-popover';
import { composerImagePreview, composerImageStep } from './composer-chip-details';

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
