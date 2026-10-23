// General → About → Mobile app, ported from T3 Code 9c161462c9 MobileAppSettingsRow.tsx (MIT, LICENSE-T3).
import { coreRow, type CoreRow } from './settings-core';
import { qrCells, type QrCell } from './settings-qr';

export const APP_STORE_URL = 'https://apps.apple.com/us/app/t3-code-remote-claude-more/id6787819824';
export const GOOGLE_PLAY_URL = 'https://play.google.com/store/apps/details?id=com.t3tools.t3code';
export type QrMark = { id: string; url: string; label: string; caption: string; cells: QrCell[] };
let marks: QrMark[] | null = null;
export function mobileAppRow(): CoreRow {
  marks ??= [
    { id: 'ios', url: APP_STORE_URL, label: 'iPhone and iPad', caption: 'App Store' },
    { id: 'android', url: GOOGLE_PLAY_URL, label: 'Android', caption: 'Google Play' },
  ].map(mark => ({ ...mark, cells: qrCells(mark.url, 128, 1, 'M') }));
  return coreRow('mobile-app', 'Mobile app', 'Connect to your server from your phone or tablet.', 'mobile-app', { qr: marks, width: 0 });
}
