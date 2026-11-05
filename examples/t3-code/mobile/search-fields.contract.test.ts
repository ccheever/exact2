import { expect, test } from 'bun:test';
import { appText } from '../tests/support/app-files';

test('mobile filter fields use native search traits', () => {
  const route = appText('mobile/route-content.contract');
  const scheduled = appText('mobile/settings-scheduled.contract');
  for (const [source, placeholder] of [
    [route, 'Search models'],
    [route, 'Search branches'],
    [scheduled, 'Search branches'],
  ] as const) {
    const start = source.indexOf(`placeholder="${placeholder}"`);
    expect(start).toBeGreaterThanOrEqual(0);
    const lineStart = source.lastIndexOf('\n', start) + 1;
    const lineEnd = source.indexOf('\n', start);
    const line = source.slice(lineStart, lineEnd < 0 ? source.length : lineEnd);
    expect(line).toContain('input type="search"');
    expect(line).toContain('autocapitalize="none" autocomplete="off" spellcheck=false');
  }
});
