// The host commands the JS target carries beyond rt.js's own, each as the
// wasm web host runs it (host/web/glue.js, navigation.js), so a command the
// compiler admits (contract/types/src/checks.rs `HOST_COMMANDS`) is one this
// runtime carries (files diary F5: `selectText` was refused here).
import { popoverCommand, setFieldSelection, setFieldRangeText, appRootFontSize, liveById, declaredScheme } from "./navigation.js";
import "./prompt-glue.js"; // a prompt's Return (host/web/prompt-glue.js)
import "./alert-message.js"; // a grouped alert's message box (host/web/alert-message.js)
import "./menu-rows.js"; // a menu row's popover opens from the menu's invoker (host/web/menu-rows.js, LLP 1021)
globalThis.exact?.menuRows?.();
// A development page may hold a fact over the app's own (LLP 1122 T7, theme2's F2 ruling: a copy the design tool pins):
// the app's choice kept as asked, with how to apply it, and applied only when the fact is not held.
const asked = (fact, value, apply) => { const x = globalThis.exact; if (!x) return apply(); (x.asked ??= {})[fact] = { value, apply }; if (!x.held?.[fact]) apply(); else x.onAsked?.(); };
// `agentMode`: rt.js's, whether the agent drives the page.
export const commands = (say, agentMode = () => false) => ({
  // The field's own method, by its id (x2apps codeedit #2); commands run
  // once the commit's tree is in place, so a value set beside it is there.
  removeAllRanges: () => getSelection()?.removeAllRanges(),
  setSelectionRange: (...args) => setFieldSelection(liveById(args[0]), args, say),
  setRangeText: (...args) => setFieldRangeText(liveById(args[0]), args, say),
  // Focus, then the field's whole text selected, as `select()` does
  // (navigation.js `runFocusCommands`).
  selectText: id => {
    const e = liveById(id);
    if (typeof e?.select !== "function") return say(`selectText "${id}" refused: ${e ? "not a text editor" : "no live node with that id"}`);
    e.focus();
    if (document.activeElement === e) e.select();
  },
  // An absolute http, https, mailto or tel URL in a new browsing context,
  // the one scheme allowlist every navigation takes (navigation.js `navigableURL`),
  // or one of a scheme app.json's `open_url.schemes` declares (`exact.openSchemes`),
  // which the browser asks about before it hands it to the app registered for it.
  openURL: u => {
    let to = null, declared = false;
    try { const url = new URL(u); declared = declaredScheme(url.protocol); if (declared || ["http:", "https:", "mailto:", "tel:"].includes(url.protocol)) to = url.href; } catch {}
    if (to && declared && agentMode()) say(`openURL ${to.slice(0, to.indexOf(":"))}: ${to.slice(0, 200)} (journaled, not opened under the agent)`);
    else if (to) open(to, "_blank", "noopener,noreferrer");
    // The app's own settings page (LLP 1133.002 D11): a browser has none to open.
    else if (u === "app-settings:") say("openURL refused: app-settings: has no page on the web; a site's permissions are the browser's own (its address bar, its settings)");
    else say(`openURL refused: ${JSON.stringify(String(u).slice(0, 80))} is not an absolute http, https, mailto or tel URL, nor of a scheme app.json's open_url.schemes declares`);
  },
  // The inverse of `message=`: text into the named surface, every post in
  // order, held until the GPU glue loads and drains `pendingPosts`
  // (gpu-glue.js); 64 a surface, as every host bounds them.
  postMessage: (text, name, kind) => {
    const live = [...document.querySelectorAll('#exact-root *')].filter(e => !e.closest('[data-exiting]'));
    if (kind === "iframe") { const captured = live.filter(e => e.localName === 'iframe' && e.id === name).map(e => [e, e.exactFrameChannel]); import('./frame-messages.js').then(m => m.postFrameMessage(text, name, null, say, captured)); return; }
    if (live.some(e => e.localName === 'iframe' && e.id === name)) return say(`postMessage: canvas name collides with an iframe id "${name}"`);
    const x = globalThis.exact ??= {}, at = x.now?.() ?? performance.now();
    if (x.gpu) x.gpu.post(name, text, at);
    else if ((x.pendingPosts ??= []).filter(p => p.name === name).length >= 64) say(`postMessage: dropped: 64 posts already wait for surface "${name}"`);
    else x.pendingPosts.push({ name, text, at, generation: 0 });
  },
  // `:root { font-size }` (LLP 1069.000 D3): the stylesheet's `rem` rows
  // follow it; checked here, where no runner checked it first.
  // (A size the app may not set is no choice: the last one stands, and the call refuses as ever.)
  setRootFontSize: v => v === "medium" || (typeof v === "number" && Math.fround(v) > 0 && Number.isFinite(Math.fround(v))) || (typeof v === "string" && /^[+-]?(\d+|\d*\.\d+)(e[+-]?\d+)?%$/i.test(v) && ((h) => Math.fround(parseFloat(v) / 100 * h) > 0 && Number.isFinite(Math.fround(parseFloat(v) / 100 * h)))(parseFloat(getComputedStyle(document.documentElement).fontSize) || 16)) ? asked('rootFontSize', v, () => appRootFontSize(v, say)) : appRootFontSize(v, say),
  // A recording of frames is the Apple hosts'; the browser's performance panel records the page.
  startTrace_EXPERIMENTAL: () => say("startTrace_EXPERIMENTAL refused: the web host does not record traces; use the browser's performance panel"),
  stopTrace_EXPERIMENTAL: () => say("stopTrace_EXPERIMENTAL refused: the web host does not record traces"),
  // The window's app modes (LLP 1121 D5, D7): modes.js's, which an app with
  // axes loads; any other refuses it, as the runner does.
  setMode: (...args) => globalThis.exact?.modes ? globalThis.exact.modes.command(args) : say(`setMode(${args.map(x => JSON.stringify(x)).join(", ")}) refused: an axis of the app's design and one of its values`),
  // The page's own reload: every web page has one, where a native host
  // needs its dev menu's. `reload(id, ignoreCache)` is a frame's.
  reload: (...args) => args.length && args[0] != null ? frame(say, "reload", args) : location.reload(),
  // A frame's navigation commands (LLP 1020.002 D3): frame-navigation.js's,
  // the subset a page has over its frame.
  navigate: (...args) => frame(say, "navigate", args),
  traverseHistory: (...args) => frame(say, "traverseHistory", args),
  stopLoading: (...args) => frame(say, "stopLoading", args),
  // A picture of a frame's guest (LLP 1020.002 D7): refused on the web, answered on its `capture=`.
  captureScreenshot: (...args) => frame(say, "captureScreenshot", args),
  relaunch: () => location.reload(),
  // `window.close()` (studio diary R17): a browser closes only a window a
  // script opened, and says so in its console otherwise.
  // `close(id)` is a dialog's (LLP 1101.001 P5): HTMLDialogElement's.
  close: id => id == null ? window.close() : modal(id, say, "close"),
  // `showModal(id)` from an action (LLP 1115 D6): the dialog's own, or an
  // alertdialog popover's showPopover, as the native hosts present either.
  showModal: id => modal(id, say, "showModal"),
  showPopover: (id, source) => popoverCommand("showPopover", id, source, say),
  hidePopover: id => popoverCommand("hidePopover", id, null, say),
  togglePopover: (id, source) => popoverCommand("togglePopover", id, source, say),
  // A JS build links no update store (facts.js `exactDelivery`): the page
  // loaded the newest root, and nothing is ever staged.
  deliveryCheck: () => say("delivery: no update store on the web; the page loaded the newest root"),
  deliveryActivate: () => say("delivery: nothing is staged"),
  // A media element's, by HTML's method names (podcast F8, F18): media.js
  // queues them for the web host's media glue (media-glue.js `run`).
  fastSeek: (id, seconds) => media(say, id, "fastSeek", seconds),
  load: id => media(say, id, "load"),
});
function frame(say, name, args) {
  import("./frame-navigation.js").then(m => m.command(name, args, say));
}

function media(say, id, name, seconds) {
  const e = liveById(id);
  if (!e?.$media) return say(`${name} "${id}" refused: ${e ? "not a video or audio" : "no live node with that id"}`);
  e.$media.command(name, seconds);
}

function modal(id, say, method) {
  const e = liveById(id);
  if (e instanceof HTMLDialogElement) { if (method === "close") e.close(); else if (!e.open) e.showModal(); else say(`showModal "${id}" ignored: dialog is already open`); return; }
  if (e?.popover != null && e.getAttribute("role") === "alertdialog") { if (method === "close") e.hidePopover(); else if (!e.matches(":popover-open")) e.showPopover(); return; }
  if (method === "showModal") say(`showModal "${id}" refused: no dialog or alertdialog popover has that id`);
}
