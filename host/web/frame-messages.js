// Loaded only by an iframe post; capture ownership before importing this module.
export { commitGuestOrigin as commitFrameOrigin, guestMessageAuthorized as frameMessageAuthorized } from './navigation.js';
export function postFrameMessage(text, name, views, say, captured) {
  const live = [...(views?.values() ?? document.querySelectorAll('#exact-root *'))].filter(e => e.isConnected && !e.closest('[data-exiting]'));
  const matches = live.filter(e => e.localName === 'iframe' && e.id === name);
  if (matches.length !== 1 || live.some(e => e.exactSurfaceName === name)) {
    say(`postMessage: no unique live iframe with id "${name}"`);
    return;
  }
  const frame = matches[0], state = frame.exactFrameChannel;
  if (captured && !captured.some(([node, token]) => node === frame && token === state)) return;
  if (!state || state.revoked || !state.opaque && !state.origin) return;
  // Capture this document's Window now. There is no application queue that can
  // resolve the id again after navigation or replace it with a sibling frame.
  const target = frame.contentWindow;
  if (target) target.postMessage(text, state.opaque ? '*' : state.origin);
}
(globalThis.exact ??= {}).frameMessages = { postFrameMessage };
