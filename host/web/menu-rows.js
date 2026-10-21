// LLP 1021, exact2's menu rule (theme2 2026-10-21 19:47, 22:05): a popover a
// row of a menu (`role="menu"`) opens, other than the menu itself or a
// submenu (itself a menu), is opened as if by the menu's own invoker
// (`showPopover({ source })`): HTML would nest it in the menu, and the menu
// closing as its row is chosen would take it along. The menu's invoker is
// the button that opened it, or the node a context menu opened on
// (navigation.js `contextAt` notes it as the popover's `exactOpener`).
// Loaded after paint (glue.js), as nothing opens a menu before then.

/** Installs the rule on the document, once. */
function menuRows() {
  if (globalThis.exactMenuRows) return;
  globalThis.exactMenuRows = true;
  addEventListener("click", e => {
    const row = e.target.closest?.("button[popovertarget]");
    if (!row || /^hide$/i.test(row.getAttribute("popovertargetaction") ?? "")) return;
    const target = document.getElementById(row.getAttribute("popovertarget"));
    if (!target?.hasAttribute("popover")) return;
    const menu = row.closest('[popover][role="menu"]');
    // Each invoker is noted as its popover's opener, for a row of it to open from.
    if (!menu || menu === target || target.getAttribute("role") === "menu") return void (target.exactOpener = row);
    e.preventDefault(); // HTML's invoker would nest it in the menu
    const source = menu.exactOpener?.isConnected ? menu.exactOpener : undefined;
    target.exactOpener = source;
    // After the row's own press, which this click still delivers.
    setTimeout(() => { if (target.isConnected && !target.matches(":popover-open")) try { target.showPopover({ source }); } catch {} });
  }, true);
}

(globalThis.exact ??= {}).menuRows = menuRows;
