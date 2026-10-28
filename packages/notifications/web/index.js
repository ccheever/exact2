// @exact/notifications on the web (LLP 1133.004 D1, D7, §8): the
// Notification API in the page, the Badging API, and the permission. The
// page runtime (host/web-js/modules.js) calls it outside the agent; under
// the agent the runner's substitute answers and this is never loaded.
//
// - A post while the permission is `default` asks first (Q4); a browser
//   that shows notifications only from a service worker throws at the
//   constructor (Chrome on Android), and the post refuses with its error.
// - `showTrigger` is held by a timer while the page is open: the web has no
//   trigger that outlives the page (a declared deviation, §8).
// - A click is bound through the host's `activation`, so only a trusted one
//   raises `notificationclick` (or opens `navigate`); the notification is
//   then closed, as Apple removes one its person opened (D2).
// - The state is live: `PermissionStatus`'s change, and a look again when
//   the page comes back to the foreground.

const STATES = { default: 'prompt', granted: 'granted', denied: 'denied' };

export function implementation({ changed, activation, log = () => {} }) {
  const shown = new Map(), waiting = new Map();
  const has = typeof Notification === 'function';
  const status = () => ({
    available: has,
    state: has ? STATES[Notification.permission] ?? 'unknown' : 'unknown',
    badge: typeof navigator === 'object' && 'setAppBadge' in navigator,
    apple: null,
  });
  let said = has ? Notification.permission : null;
  const again = () => { if (has && Notification.permission !== said) { said = Notification.permission; changed('notifications.notificationStatus'); } };
  if (typeof addEventListener === 'function') addEventListener('visibilitychange', again);
  navigator.permissions?.query({ name: 'notifications' }).then(p => { p.onchange = again; }, () => {});
  const close = tag => {
    clearTimeout(waiting.get(tag)); waiting.delete(tag);
    shown.get(tag)?.close(); shown.delete(tag);
  };
  const post = ({ title, body = null, tag = null, silent = null, data = null, navigate = null }) => {
    let n;
    try { n = new Notification(title, { ...(body != null && { body }), ...(tag != null && { tag }), ...(silent != null && { silent }), ...(data != null && { data }) }); }
    catch (e) { return { error: e?.name ?? 'NotSupportedError', message: String(e?.message ?? e) }; }
    activation(n, 'click', 'notificationclick', [tag ?? '', { tag: tag ?? '', data, title, body }], { navigate });
    n.addEventListener('click', e => { if (e.isTrusted) { e.preventDefault(); n.close(); } });
    if (tag != null) { shown.set(tag, n); n.onclose = () => { if (shown.get(tag) === n) shown.delete(tag); }; }
    log('showNotification: shown');
    return null;
  };
  return {
    async notificationStatus() { return status(); },
    async requestNotificationPermission() {
      if (has && Notification.permission === 'default') await Notification.requestPermission().catch(() => {});
      again();
      return status();
    },
    async showNotification(options) {
      if (!has) return { error: 'NotSupportedError' };
      const permission = Notification.permission === 'default' ? await Notification.requestPermission().catch(() => 'default') : Notification.permission;
      again();
      if (permission !== 'granted') return { error: 'NotAllowedError' };
      if (options.tag != null) close(options.tag);
      const at = options.showTrigger;
      if (at == null || at <= Date.now()) return post(options);
      // Re-armed past setTimeout's 2^31 ms ceiling.
      const wait = () => {
        const left = at - Date.now();
        if (left <= 0) { if (options.tag != null) waiting.delete(options.tag); return post(options); }
        const timer = setTimeout(wait, Math.min(left, 2 ** 31 - 1));
        if (options.tag != null) waiting.set(options.tag, timer);
      };
      wait();
      log('showNotification: scheduled while this page is open');
      return null;
    },
    async closeNotification({ tag }) { close(tag); return null; },
    // The Badging API (D1): best effort; a browser that lacks it refuses.
    async setAppBadge({ contents = null }) {
      if (!('setAppBadge' in navigator)) return { error: 'NotSupportedError' };
      try { await (contents == null ? navigator.setAppBadge() : navigator.setAppBadge(contents)); return null; }
      catch (e) { return { error: e?.name ?? 'NotAllowedError' }; }
    },
    async clearAppBadge() {
      if (!('clearAppBadge' in navigator)) return { error: 'NotSupportedError' };
      try { await navigator.clearAppBadge(); return null; } catch (e) { return { error: e?.name ?? 'NotAllowedError' }; }
    },
    release() {},
  };
}
