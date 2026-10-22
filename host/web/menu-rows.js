// LLP 1021, exact2's menu rule (theme2 2026-10-21 19:47, 22:05): a popover a
// row of a menu (`role="menu"`) opens, other than the menu itself or a
// submenu (itself a menu), is opened as if by the menu's own invoker
// (`showPopover({ source })`): HTML would nest it in the menu, and the menu
// closing as its row is chosen would take it along. The menu's invoker is
// what opened it: the button, or the node a context menu opened on, as the
// menu's `beforetoggle` said (navigation.js `popoverSource`). A popover
// written inside the menu stays nested by where it is (HTML), whatever its
// source. Loaded after the first paint (glue.js), the JS target at boot.
import { popoverSource } from "./navigation.js";

/** What `row` invokes, as HTML resolves it: `commandfor` before `popovertarget`; null for a hide or no popover. */
function invoked(row) {
  const commanded = row.getAttribute("commandfor");
  const target = document.getElementById(commanded ?? row.getAttribute("popovertarget") ?? "");
  const action = commanded != null ? (row.getAttribute("command") ?? "").toLowerCase() : (row.getAttribute("popovertargetaction") ?? "toggle").toLowerCase();
  const opens = commanded != null ? ["show-popover", "toggle-popover"].includes(action) : action !== "hide";
  return opens && target?.hasAttribute("popover") ? target : null;
}

/** Installs the rule on the document, once. */
function menuRows() {
  if (globalThis.exactMenuRows) return;
  globalThis.exactMenuRows = true;
  addEventListener("click", e => {
    const row = e.target.closest?.("button[popovertarget], button[commandfor]");
    const menu = row?.closest('[popover][role="menu"]'), target = row && invoked(row);
    if (!menu || !target || target === menu || target.getAttribute("role") === "menu") return;
    e.preventDefault(); // HTML's invoker would nest it in the menu
    const source = popoverSource(menu);
    // After the row's own press, which this click still delivers: what the row opens then, if it still can.
    setTimeout(() => {
      const now = row.isConnected && !row.disabled && row.closest('[popover][role="menu"]') === menu ? invoked(row) : null;
      if (!now || now.matches(":popover-open")) return;
      try { now.showPopover({ source: now === menu || now.getAttribute("role") === "menu" ? row : source?.isConnected ? source : undefined }); } catch {}
    });
  }, true);
}

(globalThis.exact ??= {}).menuRows = menuRows;
