// General → About → Mobile app (T3 Code 9c161462c9 MobileAppSettingsRow.tsx).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { encodeQr, qrCells } from '../../src/settings/settings-qr';
import { aboutRows, rememberDelivery } from '../../src/settings/settings-a-about';
import { CLIENT_VERSION } from '../../src/connections/connections';
import { APP_STORE_URL, GOOGLE_PLAY_URL, mobileAppRow } from '../../src/settings/settings-mobile-beta';

const IOS_TESTFLIGHT_URL = 'https://testflight.apple.com/join/XgaxaRtd';
// Module rows of the reference's QrCode.encodeText(IOS_TESTFLIGHT_URL, MEDIUM) (version 3, mask 2),
// as packages/shared/src/qrCode.ts at f90b77d809 draws it ('#' dark).
const TESTFLIGHT_TOP = ['#######..#..#...#####.#######', '#.....#..##....##.....#.....#', '#.###.#.##.#..#.#.#...#.###.#'];

describe('beta links as QR codes', () => {
  test('the TestFlight link encodes as the reference symbol (version 3, 29 modules)', () => {
    const modules = encodeQr(IOS_TESTFLIGHT_URL, 'M');
    expect(modules.length).toBe(29);
    expect(modules.slice(0, 3).map(row => row.map(dark => dark ? '#' : '.').join(''))).toEqual(TESTFLIGHT_TOP);
    // Finder patterns at three corners, timing row between them.
    expect(modules[6]!.slice(8, 21).map(dark => dark ? 1 : 0).join('')).toBe('1010101010101');
  });
  test('cells are dark runs inside a 128-point square with one light module of margin', () => {
    const cells = qrCells(IOS_TESTFLIGHT_URL, 128, 1, 'M'), unit = 128 / 31;
    expect(cells.length).toBeGreaterThan(29);
    for (const cell of cells) {
      expect(cell.x).toBeGreaterThanOrEqual(Math.floor(unit));
      expect(cell.x + cell.w).toBeLessThanOrEqual(128 - Math.floor(unit) + 0.5);
      expect(cell.w).toBeGreaterThan(0);
      expect(cell.h).toBeGreaterThan(0);
      expect(cell.x * 2 % 1).toBe(0);
    }
    // The top-left finder's first row is one run seven modules wide.
    expect(cells[0]).toEqual({ x: 4, y: 4, w: 29, h: 4.5 });
  });
});

describe('Mobile app row', () => {
  test('both stable and nightly show store links after the version rows', () => {
    const client = {} as T3Client;
    for (const stream of ['embedded', 'nightly/macos-arm64', 'latest/macos-arm64']) {
      rememberDelivery(client, stream, false);
      const rows = aboutRows(client, '0.0.45');
      expect(rows.map(row => row.title)).toEqual(['Version', 'Update track', 'Mobile app']);
      expect(aboutRows(client, CLIENT_VERSION)[0]!.value).toBe('0.1.6');
      expect(rows.at(-1)).toMatchObject({ id: 'mobile-app', kind: 'mobile-app', description: 'Connect to your server from your phone or tablet.' });
    }
  });
  test('the QR symbols encode exactly the store links their Open buttons use', () => {
    const row = mobileAppRow();
    expect(row.qr.map(mark => [mark.id, mark.label, mark.caption, mark.url])).toEqual([
      ['ios', 'iPhone and iPad', 'App Store', APP_STORE_URL], ['android', 'Android', 'Google Play', GOOGLE_PLAY_URL],
    ]);
    for (const mark of row.qr) expect(mark.cells).toEqual(qrCells(mark.url, 128, 1, 'M'));
  });
});
