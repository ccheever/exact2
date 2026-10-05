// NightlyMobileBetaNotice (MIT reference, see LICENSE-T3: apps/web/src/
// components/NightlyMobileBeta.tsx, upstream f1dcd93): a one-time Nightly toast
// pointing to the beta mobile app. Any close (the corner orb, Dismiss, or the
// action) hides it for good on this client; the action opens Settings ›
// General at the "Mobile app" row (`#nightly-mobile-beta`).
import type { T3Client } from './client';
import { pushToast } from './toast';
import { shellPrefs } from './shell-prefs';

/** This client is the Nightly build: its sidebar carries the Nightly artwork and it pairs with Nightly servers. */
export const IS_NIGHTLY_BUILD = true;
export const NIGHTLY_ROW_ID = 'nightly-mobile-beta';
/** The row's first control (settings-mobile-beta.contract): Settings scrolls it to the middle and focuses it, standing in for `#nightly-mobile-beta`. */
export const NIGHTLY_ROW_TARGET = 'mobile-beta-ios';
export const NIGHTLY_NOTICE = {
  title: 'Nightly needs the beta mobile app',
  description: 'Nightly uses the new orchestrator. The App Store and Google Play versions of T3 Code cannot connect to it.',
};

// Guards against a second toast for one window (the reference's module-level noticeShown).
const shown = new WeakSet<object>();

/**
 * Shown once the saved preferences are read (the reference reads its storage
 * flag synchronously on mount), unless dismissed before. Returns the toast id, or 0.
 */
export function nightlyMobileBetaNotice(client: T3Client, preferencesLoaded: boolean, nightly = IS_NIGHTLY_BUILD): number {
  if (!nightly || !preferencesLoaded || shown.has(client)) return 0;
  shown.add(client);
  const prefs = shellPrefs(client);
  if (prefs.nightlyNoticeDismissed) return 0;
  return pushToast(client, {
    kind: 'info', title: NIGHTLY_NOTICE.title, description: NIGHTLY_NOTICE.description, timeoutMs: 0,
    leading: 'smartphone:foreground', stacked: true, hideCopy: true, closeOnAction: true,
    action: { label: 'Get the beta app', op: 'ui:settings', id: 'general', value: NIGHTLY_ROW_TARGET },
    secondary: { label: 'Dismiss', op: 'dismiss' }, secondaryVariant: 'ghost',
    onClose: () => { shellPrefs(client).nightlyNoticeDismissed = true; },
  });
}
