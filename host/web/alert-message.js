// A grouped alert's message box (`alertPart`, contract/lower/src/alert_parts.rs) starts at its top each time the alert
// opens, and is a Tab stop while it overflows so a keyboard can scroll it (Chrome and Firefox make a scroller one
// themselves; WebKit does not, bug 277290). The box is inert through the dialog's focusing steps, so the alert opens on
// its field or a button, as the native alerts do: neither a link in the message nor the box (which Chrome would focus
// while it overflows) takes the first focus. Both web targets load this beside prompt-glue.js, the wasm page after its
// first paint and a page rendered ahead when it activates, which opening a grouped alert does (capture.js,
// document-glue.js): an alert may already be open.

const MESSAGE = ':scope > [data-alertpart="message"]';
const open = new WeakMap(); // a dialog → what ends the watch on its message

addEventListener('beforetoggle', e => {
  const dialog = e.target;
  if (dialog?.localName !== 'dialog' || dialog.getAttribute('role') !== 'alertdialog') return;
  unwatch(dialog);
  if (e.newState !== 'open') return;
  const held = dialog.querySelector(MESSAGE);
  held?.removeAttribute('tabindex');
  if (held) held.inert = true; // out of the dialog's focusing steps, which follow this event (above)
  const free = () => { if (held) held.inert = false; };
  // After the opening, before its first paint (a microtask would run before the focusing steps of a `commandfor`
  // opening, which dispatches this event first). The box is the one the alert has then.
  const frame = requestAnimationFrame(() => {
    free();
    const message = dialog.querySelector(MESSAGE);
    if (open.get(dialog)?.frame === frame && message && current(dialog, message)) watch(dialog, message, true);
  });
  open.set(dialog, { frame, end: () => { cancelAnimationFrame(frame); free(); } });
}, true);
// An alert open before this loaded, whose focus Chrome may have put on the box: the focus moves where the opening
// would put it now, an `autofocus` element first, else the first that takes it (HTML's dialog focusing steps)
for (const dialog of globalThis.document?.querySelectorAll('dialog[open][role="alertdialog"]') ?? []) {
  const message = dialog.querySelector(MESSAGE);
  if (!message) continue;
  if (document.activeElement === message) {
    for (const el of [...dialog.querySelectorAll('[autofocus]'), ...dialog.querySelectorAll('*')]) {
      if (message.contains(el)) continue;
      el.focus();
      if (document.activeElement === el) break;
    }
  }
  watch(dialog, message, false);
}

// Whether `message` is still the open alert's, in the page
function current(dialog, message) {
  return dialog.open && dialog.isConnected && dialog.querySelector(MESSAGE) === message;
}

// Keep its stop current while it is open (at its top when it just opened): a message whose text or look changes, a
// look its ancestors pass down (their style, class, modes, language), a window that resizes, and the alert or its box
// leaving the page.
function watch(dialog, message, opened) {
  if (opened) {
    message.scrollTop = 0;
    message.scrollLeft = 0;
  }
  const sizes = new ResizeObserver(() => sync());
  const observe = () => {
    sizes.disconnect();
    sizes.observe(message);
    for (const child of message.children) sizes.observe(child);
  };
  const edits = new MutationObserver(() => {
    observe();
    sync();
  });
  const sync = () => {
    if (!current(dialog, message)) {
      unwatch(dialog);
      // A box the page replaced while the alert stays open is watched in its place
      const next = dialog.open && dialog.isConnected && dialog.querySelector(MESSAGE);
      if (next) watch(dialog, next, false);
      return;
    }
    tabStop(message);
  };
  // A text's look can make it overflow with no box changing size. Inside the alert only the attributes that set a look
  // are heard (this module writes the box's `tabindex`); on its ancestors any attribute, and their children, which is
  // how the alert's removal is heard when an empty box has no size for it to change.
  edits.observe(dialog, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['class', 'style', 'data-x-modes', 'lang', 'dir'] });
  for (let up = dialog.parentElement; up; up = up.parentElement) edits.observe(up, { attributes: true, childList: true });
  observe();
  open.set(dialog, { end: () => { sizes.disconnect(); edits.disconnect(); } });
  sync();
}

function tabStop(message) {
  if (message.scrollHeight > message.clientHeight || message.scrollWidth > message.clientWidth) {
    message.tabIndex = 0;
  } else if (document.activeElement === message) {
    // Out of the Tab order, but the focus stays until the person moves it
    message.tabIndex = -1;
  } else {
    message.removeAttribute('tabindex');
  }
}

function unwatch(dialog) {
  open.get(dialog)?.end();
  open.delete(dialog);
}
