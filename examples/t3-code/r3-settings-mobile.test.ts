// Lane r3-settings: General → About → Mobile app (upstream f1dcd93931 NightlyMobileBeta.tsx).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { encodeQr, qrCells } from './settings-qr';
import { aboutRows, rememberDelivery } from './settings-a-about';
import { CLIENT_VERSION } from './connections';
import { ANDROID_BETA_GROUP_URL, ANDROID_PLAY_TESTING_URL, IOS_TESTFLIGHT_URL, isNightlyVersion, mobileBetaCommand, mobileBetaMarks, showMobileBeta } from './settings-mobile-beta';

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
  test('the Android links are version 4 (33 modules)', () => {
    expect(encodeQr(ANDROID_BETA_GROUP_URL, 'M').length).toBe(33);
    expect(encodeQr(ANDROID_PLAY_TESTING_URL, 'M').length).toBe(33);
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
  test('IS_NIGHTLY_BUILD reads the first prerelease identifier', () => {
    expect(isNightlyVersion('0.0.46-nightly.20261004.1')).toBe(true);
    expect(isNightlyVersion('0.0.45')).toBe(false);
    expect(isNightlyVersion('0.0.46-beta.1')).toBe(false);
    expect(showMobileBeta('0.0.45', 'nightly')).toBe(true);
    expect(showMobileBeta('0.0.45', 'latest')).toBe(false);
  });
  test('About lists Mobile app after the version rows: this client is a Nightly build', () => {
    const client = {} as T3Client;
    rememberDelivery(client, 'embedded', false);
    expect(isNightlyVersion(CLIENT_VERSION)).toBe(true);
    expect(aboutRows(client, CLIENT_VERSION).map(row => row.title)).toEqual(['Version', 'Update track', 'Mobile app']);
    expect(aboutRows(client, CLIENT_VERSION)[0]!.value).toBe('0.0.46-nightly.20261004.1');
    rememberDelivery(client, 'nightly/macos-arm64', false);
    const mobile = aboutRows(client, CLIENT_VERSION).at(-1)!;
    expect(mobile).toMatchObject({ id: 'nightly-mobile-beta', kind: 'mobile-beta', description: 'Nightly needs the beta app. The App Store and Google Play versions cannot connect.' });
    expect(mobile.qr.map(mark => [mark.id, mark.caption, mark.copied])).toEqual([['ios', '', 0], ['android-group', '1. Join the group', 0], ['android-play', '2. Become a tester', 0]]);
  });
  test('Copy link writes only a beta link; each copy replays Copied', async () => {
    const copied: string[] = [];
    const client = { restAccess: () => ({ call: async (request: { op: string; text: string }) => { copied.push(request.text); return { copied: true }; } }) } as unknown as T3Client;
    const native = { available: true } as unknown as Native;
    expect(mobileBetaMarks(client).map(mark => mark.copied)).toEqual([0, 0, 0]);
    await mobileBetaCommand(client, native, 'copy', IOS_TESTFLIGHT_URL);
    expect(copied).toEqual([IOS_TESTFLIGHT_URL]);
    expect(mobileBetaMarks(client).map(mark => mark.copied)).toEqual([1, 0, 0]);
    await mobileBetaCommand(client, native, 'copy', IOS_TESTFLIGHT_URL);
    await mobileBetaCommand(client, native, 'copy', ANDROID_PLAY_TESTING_URL);
    expect(mobileBetaMarks(client).map(mark => mark.copied)).toEqual([2, 0, 1]);
    await expect(mobileBetaCommand(client, native, 'copy', 'https://example.com')).rejects.toThrow('Unsupported mobile app action.');
    expect(copied.length).toBe(3);
  });
});
