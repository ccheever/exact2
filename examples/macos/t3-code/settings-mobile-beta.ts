// General → About → Mobile app (lane r3-settings; upstream f1dcd93931
// NightlyMobileBeta.tsx NightlyMobileBetaRow). Orchestrator V2 ships to Nightly
// first and the store apps cannot connect to it, so a Nightly build (or a desktop
// whose update track is Nightly) links the beta apps: TestFlight for iPhone, and the
// Google group then the Play testing page for Android, each as a QR code
// (QRCodeSvg size 128, level M, marginSize 1) with a Copy link button.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { ClientError } from './protocol';
import { coreRow, type CoreRow } from './settings-core';
import { qrCells, type QrCell } from './settings-qr';

export const IOS_TESTFLIGHT_URL = 'https://testflight.apple.com/join/XgaxaRtd';
export const ANDROID_BETA_GROUP_URL = 'https://groups.google.com/g/t3-code-v2-beta';
export const ANDROID_PLAY_TESTING_URL = 'https://play.google.com/apps/testing/com.t3tools.t3code';
export const MOBILE_BETA_ROW = 'nightly-mobile-beta';

/** `copied` counts the link's copies: each new one replays the view's two-second "Copied" (useCopyToClipboard's timeout). */
export type QrMark = { id: string; url: string; label: string; caption: string; copied: number; cells: QrCell[] };
const LINKS: [string, string, string, string][] = [
  ['ios', IOS_TESTFLIGHT_URL, 'TestFlight beta link', ''],
  ['android-group', ANDROID_BETA_GROUP_URL, 'Android beta group link', '1. Join the group'],
  ['android-play', ANDROID_PLAY_TESTING_URL, 'Google Play beta link', '2. Become a tester'],
];
let cells: QrCell[][] | null = null;
// Data sources have no clock (Date.now() is unavailable there): the two seconds are the view's animation.
const copies = new WeakMap<object, Map<string, number>>();
/** The three codes (encoded once per process; they never change) with each link's copy count. */
export function mobileBetaMarks(owner: object): QrMark[] {
  cells ??= LINKS.map(([, url]) => qrCells(url, 128, 1, 'M'));
  const copied = copies.get(owner);
  return LINKS.map(([id, url, label, caption], index) => ({ id, url, label, caption, copied: copied?.get(url) ?? 0, cells: cells![index]! }));
}

/** IS_NIGHTLY_BUILD: the version's first prerelease identifier is `nightly` (parseSemver). */
export function isNightlyVersion(version: string): boolean {
  const match = /^v?\d+\.\d+\.\d+-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)(?:\+[0-9A-Za-z.-]+)?$/.exec(version.trim());
  return match?.[1]?.split('.')[0] === 'nightly';
}
/** AboutVersionSection showNightlyMobileBeta: a Nightly build, or Nightly picked as the desktop update track. */
export const showMobileBeta = (version: string, channel: string) => isNightlyVersion(version) || channel === 'nightly';

export function mobileBetaRow(owner: object): CoreRow {
  return coreRow(MOBILE_BETA_ROW, 'Mobile app', 'Nightly needs the beta app. The App Store and Google Play versions cannot connect.', 'mobile-beta', { qr: mobileBetaMarks(owner), width: 0 });
}

/** BetaLinkQr's Copy link: only the three beta links are ever written to the clipboard. */
export async function mobileBetaCommand(client: T3Client, native: Native, part: string, value: string): Promise<string> {
  if (part !== 'copy' || !LINKS.some(([, url]) => url === value)) throw new ClientError('Unsupported mobile app action.');
  if (!native?.available) throw new ClientError('Copying links needs the macOS app.');
  const reply = await client.restAccess(native).call({ op: 'copyText', text: value });
  if (reply.copied === false || reply.ok === false) throw new ClientError('Could not copy the link.');
  const copied = copies.get(client) ?? new Map<string, number>();
  copied.set(value, (copied.get(value) ?? 0) + 1);
  copies.set(client, copied);
  return '';
}
