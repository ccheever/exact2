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

/** HTML's built-in invoker commands. */
const COMMANDS = ["show-popover", "hide-popover", "toggle-popover", "show-modal", "close", "request-close"];

/** What `row` invokes, as HTML resolves it: the `commandfor` element when there is one (its `command` as written), else `popovertarget`'s, with what to do. */
function invoked(row) {
  const commanded = document.getElementById(row.getAttribute("commandfor") || "");
  if (commanded) {
    const command = row.getAttribute("command") ?? "", lower = command.toLowerCase();
    const action = { "show-popover": "show", "hide-popover": "hide", "toggle-popover": "toggle" }[lower];
    // A command HTML doesn't know (not a built-in, not a custom `--` one) does nothing, with no event.
    const known = command.startsWith("--") ? command : COMMANDS.includes(lower) ? lower : null;
    return known === null ? null : { target: commanded, command: known, action: action && commanded.hasAttribute("popover") ? action : null };
  }
  const target = document.getElementById(row.getAttribute("popovertarget") || "");
  const action = (row.getAttribute("popovertargetaction") ?? "").toLowerCase();
  return target?.hasAttribute("popover") ? { target, command: null, action: ["show", "hide"].includes(action) ? action : "toggle" } : null;
}

/** What opened `menu`, past any submenu: the button or node the outermost menu came from. */
function opener(menu) {
  let source = popoverSource(menu);
  for (let seen = new Set([menu]), outer; source && (outer = source.closest('[popover][role="menu"]')) && !seen.has(outer); seen.add(outer)) source = popoverSource(outer);
  return source;
}

/** Does what `row` invokes now, as HTML would, but opens a popover that is no menu from its menu's own invoker. */
function act(row, menu) {
  const now = row.isConnected && !row.disabled && row.closest('[popover][role="menu"]') === menu ? invoked(row) : null;
  if (!now) return;
  const { target, command, action } = now;
  try {
    // A command's event first, which its listeners may cancel (HTML's `command` event).
    if (command !== null && typeof CommandEvent === "function" && !target.dispatchEvent(new CommandEvent("command", { command, source: row, cancelable: true }))) return;
    if (!action) {
      // A dialog's commands, as HTML has them; a custom one is its event alone.
      if (!(target instanceof HTMLDialogElement)) return;
      if (command === "show-modal" && !target.open && target.isConnected) target.showModal();
      else if (command === "close" && target.open) target.close(row.hasAttribute("value") ? row.value : undefined);
      else if (command === "request-close" && target.open) target.requestClose?.(row.hasAttribute("value") ? row.value : undefined);
      return;
    }
    const open = target.matches(":popover-open");
    if (action === "hide" || action === "toggle" && open) { if (open) target.hidePopover(); return; }
    if (open) return;
    const nested = target === menu || target.getAttribute("role") === "menu"; // a submenu, nested as HTML nests it
    const source = nested ? row : opener(menu);
    target.showPopover({ source: source?.isConnected ? source : undefined });
  } catch {}
}

/** Installs the rule on the document, once. */
function menuRows() {
  if (globalThis.exactMenuRows) return;
  globalThis.exactMenuRows = true;
  addEventListener("click", e => {
    const row = e.target.closest?.("button[popovertarget], button[commandfor]");
    const menu = row?.closest('[popover][role="menu"]');
    if (!menu) return;
    // HTML would act on what the row names before its own press; this acts after it, on what it names
    // then (a press may retarget it), doing every command as HTML would: a dialog's too.
    e.preventDefault();
    setTimeout(() => act(row, menu));
  }, true);
}

(globalThis.exact ??= {}).menuRows = menuRows;
