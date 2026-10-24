// A page whose build is kept no more (host/web/generations.mjs: its generation reaped, so a script or chunk under
// `/.exact/gen/<id>/` fails) loads again into the build in place: once, so a bad build cannot loop (a session's key,
// cleared after the page has run a while). A drive's page is never given it (rt.js `mount`, under the agent's clock):
// a drive never moves builds, and its carrier ends it with the generation named.
export function reloadOnGone(win = globalThis) {
  if (typeof win.addEventListener !== 'function' || !win.location) return false;
  const KEY = 'exact.generation-reload', gone = (url) => typeof url === 'string' && url.includes('/.exact/gen/');
  const again = () => {
    try { if (win.sessionStorage.getItem(KEY)) return; win.sessionStorage.setItem(KEY, '1'); } catch { return; }
    win.location.reload();
  };
  win.addEventListener('unhandledrejection', (e) => { const m = String(e.reason?.message ?? e.reason ?? ''); if (/dynamically imported module|module script failed/i.test(m) && gone(m)) again(); });
  win.addEventListener('error', (e) => { const t = e.target; if (t && t !== win && gone(t.src ?? t.href)) again(); }, true);
  win.setTimeout?.(() => { try { win.sessionStorage.removeItem(KEY); } catch { /* none */ } }, 10_000);
  return true;
}
