// The host commands the JS target carries beyond rt.js's own, each as the
// wasm web host runs it (host/web/glue.js, navigation.js), so a command the
// compiler admits (contract/types/src/checks.rs `HOST_COMMANDS`) is one this
// runtime carries (files diary F5: `selectText` was refused here).
// Notifications posted under the agent, where none reaches the system: the
// agent's `state.notifications` (agent.js), as the runner keeps them.
export const notices = [];
const untag = tag => { for (let i = notices.length; i--;) if (notices[i].tag === tag) notices.splice(i, 1); };
const notifyGlue = () => import("./notify-glue.js").then(() => globalThis.exact.notifications);
// `agent()` and `grants()`: rt.js's clock and the data module's grant text.
export const commands = (say, { agent, grants }) => ({
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
  // A JS build links no update store (facts.js `exactDelivery`): the page
  // loaded the newest root, and nothing is ever staged.
  deliveryCheck: () => say("delivery: no update store on the web; the page loaded the newest root"),
  deliveryActivate: () => say("delivery: nothing is staged"),
  // Local notifications by the Notification API's names, the runner's rule
  // (runner/src/notify.rs): the data and `device.notifications` checked,
  // listed under the agent (a tag replacing its older one), else posted by
  // the web host's notify-glue.js. The outcome is a journal line.
  showNotification: (title, body, tag, showTrigger) => {
    const refuse = why => say(`showNotification: refused: ${why}`);
    if (typeof title !== "string" || !title) return refuse("needs a title=");
    if (showTrigger != null && !(Number.isFinite(showTrigger) && showTrigger >= 0)) return refuse("showTrigger= is a time in epoch milliseconds");
    if (!/^\s*device\.notifications\s/m.test(grants() ?? "")) return refuse("the grants name no device.notifications");
    const notice = { title, body: body ?? null, tag: tag ?? null, showTrigger: showTrigger ?? null };
    if (!agent()) return notifyGlue().then(n => n.show(notice, say));
    say(`showNotification: listed${tag == null ? "" : ` (${tag})`}`);
    if (tag != null) untag(tag);
    notices.push(notice);
  },
  closeNotification: tag => agent() ? untag(tag) : notifyGlue().then(n => n.close(tag)),
});
