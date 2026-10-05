// Local notifications in the browser (rules/DEFERRED.md, 2026-10-04): the
// Notification API, once the runner's rule (runner/src/notify.rs; the JS
// target's commands.js) has admitted one, loaded on the first. Permission is
// asked the first time, inside the press's activation. A notification shows
// now, or at `showTrigger` while this page is open: the web has no trigger
// that outlives the page (the Notification Triggers draft was abandoned), a
// declared deviation from the native hosts, which hand it to the system. A
// tag replaces the older one, shown or waiting; `close(tag)` takes either
// away. The outcome is a journal line, as `share`'s is.
const shown = new Map(), waiting = new Map();
function close(tag) {
  clearTimeout(waiting.get(tag)); waiting.delete(tag);
  shown.get(tag)?.close(); shown.delete(tag);
}
function post({ title, body, tag }, log) {
  let n;
  // A browser that shows them only from a service worker throws here (Chrome on Android).
  try { n = new Notification(title, { ...(body != null && { body }), ...(tag != null && { tag }) }); }
  catch (e) { return log(`showNotification: refused: ${e?.name ?? e}`); }
  if (tag != null) { shown.set(tag, n); n.onclose = () => { if (shown.get(tag) === n) shown.delete(tag); }; }
  log('showNotification: shown');
}
function show(notice, log) {
  if (typeof Notification !== 'function') return log('showNotification: refused: unavailable');
  const asked = Notification.permission === 'default' ? Notification.requestPermission() : Promise.resolve(Notification.permission);
  return asked.then(permission => {
    if (permission !== 'granted') return log(`showNotification: refused: ${permission}`);
    if (notice.tag != null) close(notice.tag);
    const at = notice.showTrigger;
    if (at == null || at <= Date.now()) return post(notice, log);
    // Re-armed past setTimeout's 2^31 ms ceiling.
    const wait = () => {
      const left = at - Date.now();
      if (left <= 0) { if (notice.tag != null) waiting.delete(notice.tag); return post(notice, log); }
      const timer = setTimeout(wait, Math.min(left, 2 ** 31 - 1));
      if (notice.tag != null) waiting.set(notice.tag, timer);
    };
    wait();
    log('showNotification: scheduled while this page is open');
  }, e => log(`showNotification: refused: ${e?.name ?? e}`));
}
(globalThis.exact ??= {}).notifications = { show, close };
