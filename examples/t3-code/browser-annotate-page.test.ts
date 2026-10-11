// browser-surface part 3 (capture): the annotation overlay's pure helpers (assets/browser-annotate.js, a classic
// script the module runs in the page; required here it exports them and touches no window or document).
// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/desktop/src/preview/AnnotationKeyboard.test.ts, its
// 2 tests under their own names. Clone rows (marked) cover PickPreload's geometry helpers the script carries.
import { describe, expect, it } from 'bun:test';

type Rect = { x: number; y: number; width: number; height: number };
type Point = { x: number; y: number };
type KeyEvent = { key: string; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; isComposing: boolean };
const page = require('./assets/browser-annotate.js') as {
  resolveAnnotationSubmission: (event: KeyEvent) => 'attach' | 'send' | null;
  normalizeRect: (startX: number, startY: number, endX: number, endY: number) => Rect;
  isUsableRect: (rect: Rect) => boolean;
  unionRects: (rects: Rect[], padding: number, viewport: { width: number; height: number }) => Rect | null;
  pathFromPoints: (points: Point[]) => string;
  strokeBounds: (points: Point[], width: number) => Rect;
  describeRawElement: (element: { tagName: string; id: string; className: unknown }) => string;
};
const { resolveAnnotationSubmission } = page;

const keyboardEvent = (overrides: Partial<KeyEvent> = {}): KeyEvent => ({
  key: 'Enter',
  metaKey: false,
  ctrlKey: false,
  shiftKey: false,
  isComposing: false,
  ...overrides,
});

describe('resolveAnnotationSubmission', () => {
  it('attaches on Enter and sends on Cmd/Ctrl+Enter', () => {
    expect(resolveAnnotationSubmission(keyboardEvent())).toBe('attach');
    expect(resolveAnnotationSubmission(keyboardEvent({ metaKey: true }))).toBe('send');
    expect(resolveAnnotationSubmission(keyboardEvent({ ctrlKey: true }))).toBe('send');
  });

  it('leaves Shift+Enter and composition events available for editing', () => {
    expect(resolveAnnotationSubmission(keyboardEvent({ shiftKey: true }))).toBeNull();
    expect(resolveAnnotationSubmission(keyboardEvent({ isComposing: true }))).toBeNull();
    expect(resolveAnnotationSubmission(keyboardEvent({ key: ' ' }))).toBeNull();
  });
});

describe('annotation geometry (clone)', () => {
  it('normalizes a drag in any direction and needs 3 pixels each way (clone)', () => {
    expect(page.normalizeRect(50, 40, 10, 100)).toEqual({ x: 10, y: 40, width: 40, height: 60 });
    expect(page.isUsableRect({ x: 0, y: 0, width: 3, height: 3 })).toBe(true);
    expect(page.isUsableRect({ x: 0, y: 0, width: 2, height: 30 })).toBe(false);
  });

  it('unions the targets with padding, clamped to the viewport (clone)', () => {
    const viewport = { width: 800, height: 600 };
    expect(page.unionRects([], 20, viewport)).toBeNull();
    expect(page.unionRects([{ x: 40, y: 60, width: 120, height: 40 }], 20, viewport)).toEqual({ x: 20, y: 40, width: 160, height: 80 });
    expect(page.unionRects([{ x: 5, y: 5, width: 10, height: 10 }, { x: 700, y: 550, width: 90, height: 40 }], 20, viewport))
      .toEqual({ x: 0, y: 0, width: 800, height: 600 });
    expect(page.unionRects([{ x: 10, y: 10, width: 10, height: 10 }], 0, viewport)).toEqual({ x: 10, y: 10, width: 10, height: 10 });
  });

  it('draws strokes as smoothed paths with padded bounds (clone)', () => {
    expect(page.pathFromPoints([])).toBe('');
    expect(page.pathFromPoints([{ x: 1, y: 2 }])).toBe('M 1 2 l 0.01 0.01');
    expect(page.pathFromPoints([{ x: 0, y: 0 }, { x: 10, y: 10 }, { x: 20, y: 0 }])).toBe('M 0 0 Q 10 10 15 5 L 20 0');
    expect(page.strokeBounds([{ x: 10, y: 10 }, { x: 20, y: 20 }], 4)).toEqual({ x: 3, y: 3, width: 24, height: 24 });
  });

  it('labels an element by its tag, id and first two classes (clone)', () => {
    expect(page.describeRawElement({ tagName: 'BUTTON', id: 'save', className: ' primary  big extra ' })).toBe('button#save.primary.big');
    expect(page.describeRawElement({ tagName: 'svg', id: '', className: { baseVal: 'icon' } })).toBe('svg');
  });
});
