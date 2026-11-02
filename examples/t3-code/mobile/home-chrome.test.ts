import { describe, expect, test } from 'bun:test';
import { decodeHomeChromeEvent } from './home-chrome';

describe('native Home chrome events', () => {
  test('accepts workspace tab selections and rejects unknown event kinds', () => {
    expect(decodeHomeChromeEvent('{"kind":"tab","value":"settings"}')).toEqual({ kind: 'tab', value: 'settings' });
    expect(decodeHomeChromeEvent('{"kind":"tab","value":"unknown"}')).toEqual({ kind: 'tab', value: 'unknown' });
    expect(decodeHomeChromeEvent('{"kind":"tabs","value":"settings"}')).toBeNull();
  });
});
