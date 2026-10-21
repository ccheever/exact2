// ComposerBannerStack NoticeText at T3 Code 74541bab17 (#19810): a cut-off notice title, such as a long Running command,
// shows in full from an info button.
import { describe, expect, test } from 'bun:test';
import { noticeTitleDetails } from '../../src/composer/composer-presentation';
import { probeKey } from '../../src/composer/composer-menus';

const notice = (title: string, extra: Partial<Parameters<typeof noticeTitleDetails>[1]> = {}) =>
  ({ id: 'background', title, tip: '', description: '', redact: '', segments: [], machines: [], details: false, ...extra });
const presentation = (title: string, natural: number, content: number) => ({ anchors: { [probeKey(title, 12, 500)]: [0, natural], 'notice-content:background': [28, content] } });

describe('notice title details', () => {
  const title = 'Running: bun run dev --filter web --port 16930 --host 127.0.0.1';
  test('a title wider than its content row is cut off; one that fits is not', () => {
    expect(noticeTitleDetails(presentation(title, 420, 300), notice(title))).toBe(true);
    expect(noticeTitleDetails(presentation(title, 280, 300), notice(title))).toBe(false);
  });
  test('the description, and its details button while it shows, start after it', () => {
    expect(noticeTitleDetails(presentation(title, 290, 300), notice(title, { description: 'running for 3m' }))).toBe(false);
    expect(noticeTitleDetails(presentation(title, 290, 300), notice(title, { description: 'running for 3m', details: true }))).toBe(true);
    // At a larger interface font the probe's width scales with it.
    expect(noticeTitleDetails(presentation(title, 280, 300), notice(title), 18)).toBe(true);
  });
  test('nothing before both have been laid out, and never for a title that has its own tooltip', () => {
    expect(noticeTitleDetails({ anchors: {} }, notice(title))).toBe(false);
    expect(noticeTitleDetails(presentation(title, 420, 300), notice(title, { tip: 'Server 1.2 → 1.3' }))).toBe(false);
  });
});
