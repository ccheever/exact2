// Where Chromium's PDF viewer puts the pages (file-preview-host/src/pdf/browser-pdf-layout.ts): the page boxes and
// their shadows, two page view's pairs, the zoom presets and fits, and the properties dialog's page and file sizes.
import { describe, expect, test } from 'bun:test';
import { join } from 'node:path';

const layout = await import(join(import.meta.dir, '../../file-preview-host/src/pdf/browser-pdf-layout.ts')) as typeof import('../../file-preview-host/src/pdf/browser-pdf-layout');
const { fileSizeText, fittingZoom, initialZoom, mostVisiblePage, pageSizeText, pdfLayout, stepZoom } = layout;
const letter = { width: 816, height: 1056 };

describe("Chromium's PDF layout", () => {
  test('stacks the pages inside their shadows with a separator between', () => {
    const { pages, width, height } = pdfLayout([letter, letter], false);
    expect([width, height]).toEqual([826, 2 * (1056 + 10) + 4]);
    expect(pages).toEqual([{ x: 5, y: 3, width: 816, height: 1056 }, { x: 5, y: 1066 + 4 + 3, width: 816, height: 1056 }]);
  });

  test('lays two page view in pairs either side of the middle, a lone last page on the left', () => {
    const { pages, width } = pdfLayout([letter, letter, letter], true);
    expect(width).toBe(2 * (816 + 6));
    expect(pages.map(page => [page.x, page.y])).toEqual([[5, 3], [823, 3], [5, 1066 + 4 + 3]]);
  });

  test('opens at 100 % or less, steps through the presets, and fits the width or the page', () => {
    const doc = pdfLayout([letter], false);
    expect(initialZoom(doc, { width: 1200, height: 800 })).toBe(1);
    expect(initialZoom(doc, { width: 413, height: 800 })).toBe(0.5);
    expect([stepZoom(1, 1), stepZoom(1, -1), stepZoom(0.73, 1), stepZoom(0.73, -1), stepZoom(5, 1), stepZoom(0.25, -1)]).toEqual([1.1, 0.9, 0.75, 2 / 3, 5, 0.25]);
    expect(fittingZoom(doc, 0, { width: 826, height: 533 }, 'width')).toBe(1);
    expect(fittingZoom(doc, 0, { width: 826, height: 533 }, 'page')).toBe(0.5);
  });

  test('names the page that shows the most of itself', () => {
    const doc = pdfLayout([letter, letter, letter], false);
    expect([mostVisiblePage(doc, 1, 0, 800), mostVisiblePage(doc, 1, 900, 800), mostVisiblePage(doc, 1, 1100, 800)]).toEqual([0, 1, 1]);
  });

  test("writes page and file sizes as the properties dialog does", () => {
    expect([pageSizeText({ width: 612, height: 792 }, false), pageSizeText({ width: 842, height: 595 }, true), pageSizeText({ width: 500, height: 500 }, false)])
      .toEqual(['8.50 × 11.00 in (portrait)', '297 × 210 mm (landscape)', '6.94 × 6.94 in (square)']);
    expect([fileSizeText(594), fileSizeText(57958), fileSizeText(3 * 1024 * 1024)]).toEqual(['594 B', '56.6 KB', '3.0 MB']);
  });
});
