// The host commands the JS target carries beyond rt.js's own, each as the
// wasm web host runs it (host/web/glue.js, navigation.js), so a command the
// compiler admits (contract/types/src/checks.rs `HOST_COMMANDS`) is one this
// runtime carries (files diary F5: `selectText` was refused here).
export const commands = say => ({
  // Focus, then the field's whole text selected, as `select()` does
  // (navigation.js `runFocusCommands`).
  selectText: id => {
    const e = document.getElementById(id);
    if (typeof e?.select !== "function") return say(`selectText "${id}" refused: ${e ? "not a text editor" : "no live node with that id"}`);
    e.focus();
    if (document.activeElement === e) e.select();
  },
  // An absolute http, https, mailto or tel URL in a new browsing context,
  // the one scheme allowlist every navigation takes (navigation.js `navigableURL`).
  openURL: u => {
    let to = null;
    try { const url = new URL(u); if (["http:", "https:", "mailto:", "tel:"].includes(url.protocol)) to = url.href; } catch {}
    if (to) open(to, "_blank", "noopener,noreferrer");
    else say(`openURL refused: ${JSON.stringify(String(u).slice(0, 80))} is not an absolute http, https, mailto or tel URL`);
  },
  // The inverse of `message=`: text into the named surface, every post in
  // order, held until the GPU glue loads and drains `pendingPosts`
  // (gpu-glue.js); 64 a surface, as every host bounds them.
  postMessage: (text, name) => {
    const x = globalThis.exact ??= {}, at = x.now?.() ?? performance.now();
    if (x.gpu) x.gpu.post(name, text, at);
    else if ((x.pendingPosts ??= []).filter(p => p.name === name).length >= 64) say(`postMessage: dropped: 64 posts already wait for surface "${name}"`);
    else x.pendingPosts.push({ name, text, at, generation: 0 });
  },
  // The page's own reload: every web page has one, where a native host
  // needs its dev menu's.
  reload: () => location.reload(),
  // `window.close()` (studio diary R17): a browser closes only a window a
  // script opened, and says so in its console otherwise.
  close: () => window.close(),
  // A JS build links no update store (facts.js `exactDelivery`): the page
  // loaded the newest root, and nothing is ever staged.
  deliveryCheck: () => say("delivery: no update store on the web; the page loaded the newest root"),
  deliveryActivate: () => say("delivery: nothing is staged"),
  // A media element's, by HTML's method names (podcast F8, F18): media.js
  // queues them for the web host's media glue (media-glue.js `run`).
  fastSeek: (id, seconds) => media(say, id, "fastSeek", seconds),
  load: id => media(say, id, "load"),
});
function media(say, id, name, seconds) {
  const e = document.getElementById(id);
  if (!e?.$media) return say(`${name} "${id}" refused: ${e ? "not a video or audio" : "no live node with that id"}`);
  e.$media.command(name, seconds);
}
