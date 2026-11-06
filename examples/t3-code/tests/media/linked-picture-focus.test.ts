// A linked picture's two Tab stops (MarkdownMediaBody, LinkedPicture), measured against the reference at 1280 × 840:
// the link's tooltip stands on the `<a>`'s inline box, and each stop shows Chromium's focus ring for a keyboard's focus
// (FocusRing), with AppKit's taken off (`outline="none"`).
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const body = contractComponent('src/media/media-markdown.contract', 'MarkdownMediaBody');
const picture = contractComponent('src/media/media-markdown.contract', 'LinkedPicture');

function scope(keyboardLast: boolean) {
  const tips: { tip: Record<string, unknown>; on: boolean }[] = [];
  // The reference's "HTML image layout" line: an 80 × 32 picture whose `<a>` box is 16.5px tall, 6.19px below its top.
  const frames: Record<string, { x: number; y: number; width: number; height: number }> = {
    'media-1': { x: 309, y: 375.76, width: 80, height: 32 },
    'media-1-link': { x: 309, y: 375.76, width: 80, height: 32 },
  };
  return {
    tips,
    env: {
      keyboardLast, rem: 16, link: 'https://example.com/docs', nodeId: 'media-1', tip: '',
      frame: (id: string) => frames[id]!,
      HoverTip: (fields: Record<string, unknown>) => fields,
      hoverTipAt: (tip: Record<string, unknown>, _role: string, on: boolean) => { tips.push({ tip, on }); },
    },
  };
}

describe("a linked picture's link stop", () => {
  test("its tooltip's trigger is the anchor's inline box, as the reference's getBoundingClientRect is", () => {
    const { tips, env } = scope(true);
    const state = { ring: '' };
    body.call('linkTip', state, env, [true]);
    expect(tips).toHaveLength(1);
    const tip = tips[0]!.tip;
    expect([tip.x, tip.width, tip.height, tip.side, tip.text]).toEqual([309, 80, 16.5, 'top', 'https://example.com/docs']);
    expect(tip.y as number).toBeCloseTo(381.95, 1);
    expect(state.ring).toBe('link');
  });
  test('a press focuses it without a tooltip or a ring; the focus leaving takes the ring', () => {
    const { tips, env } = scope(false);
    const state = { ring: '' };
    body.call('linkTip', state, env, [true]);
    expect([tips.length, state.ring]).toEqual([0, '']);
    const left = { ring: 'link' };
    body.call('linkTip', left, scope(true).env, [false]);
    expect(left.ring).toBe('');
  });
});

describe("a linked picture's focus rings", () => {
  test('the picture stop rings the picture, and the link stop the link', () => {
    const state = { ring: '' };
    body.call('pictureTip', state, scope(true).env, [true]);
    expect(state.ring).toBe('picture');
    // The picture's blur after the link took the focus leaves the link's ring.
    const moved = { ring: 'link' };
    body.call('pictureTip', moved, scope(true).env, [false]);
    expect(moved.ring).toBe('link');
    const shown = { kind: 'image', brokenUrl: '', url: 'u', link: 'https://example.com/docs', httpLink: () => true, imageWidth: 80, imageHeight: 32, knownWidth: '' };
    expect(body.drawn('FocusRing(inset=0, radius=4)', { ring: 'link' }, shown)).toBe(true);
    expect(body.drawn('FocusRing(inset=0, radius=4)', { ring: 'picture' }, shown)).toBe(false);
    expect(picture.drawn('FocusRing(inset=1, radius=6)', {}, { ringed: true })).toBe(true);
    expect(picture.drawn('FocusRing(inset=1, radius=6)', {}, { ringed: false })).toBe(false);
  });
});
