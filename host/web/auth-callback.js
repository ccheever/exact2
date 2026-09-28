// The auth callback page's script (LLP 1069.006 D4), served at
// /.exact/auth/callback.js: hand this page's URL (the code, state and iss)
// back to the app on a same-origin BroadcastChannel — the channel that
// survives a provider's Cross-Origin-Opener-Policy — and to window.opener
// with the app's own origin as the target when it is still there; then
// drop the code from the address and close. It never prints the URL.
(() => {
  const message = { type: 'exact-auth', url: location.href };
  try { const channel = new BroadcastChannel('exact-auth'); channel.postMessage(message); channel.close(); } catch {}
  try { if (window.opener) window.opener.postMessage(message, location.origin); } catch {}
  history.replaceState(null, '', location.pathname);
  window.close();
})();
