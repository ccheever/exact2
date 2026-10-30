// LLP 1021, exact2's menu rule (theme2 2026-10-21 19:47, 22:05): a popover a
// row of a menu (`role="menu"`) opens, other than the menu itself or a
// submenu (itself a menu), is opened as if by the menu's own invoker
// (`showPopover({ source })`): HTML would nest it in the menu, and the menu
// closing as its row is chosen would take it along. The menu's invoker is
// what opened it: the button, or the node a context menu opened on, as the
// menu's `beforetoggle` said (navigation.js `popoverSource`). A popover
// written inside the menu stays nested by where it is (HTML), whatever its
// source. It also marks the button that opened a popover while the popover
// is open (below). Loaded after the first paint (glue.js), the JS target at
// boot.
import { popoverSource } from "./navigation.js";

/** HTML's popover commands, and what each does. */
const POPOVER_COMMANDS = new Map([["show-popover", "show"], ["hide-popover", "hide"], ["toggle-popover", "toggle"]]);

/** The popover `row` invokes, as HTML resolves it, with what to do: by `commandfor` when it names an
 * element (a popover command on an HTML element; its `command` lowercased), else by `popovertarget`. None
 * for anything else (a dialog's command, a custom one, a button of a form that isn't `type="button"`),
 * which keeps HTML's own activation. Whether the target is a popover is read when it's acted on. */
function invoked(row) {
  if (row.form && row.getAttribute("type")?.toLowerCase() !== "button") return null;
  const commanded = document.getElementById(row.getAttribute("commandfor") || "");
  if (commanded) {
    const command = (row.getAttribute("command") ?? "").toLowerCase();
    const action = POPOVER_COMMANDS.get(command);
    return action && commanded instanceof HTMLElement ? { target: commanded, command, action } : null;
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

/** Does what `row` invokes now (a popover's command, as at the click if it names none now), as HTML would,
 * but opens a popover that is no menu from its menu's own invoker. */
function act(row, menu, clicked) {
  if (!row.isConnected || row.matches(":disabled") || row.closest('[popover][role="menu"]') !== menu) return;
  const { target, command, action } = invoked(row) ?? clicked;
  try {
    // A command's event first, which its listeners may cancel (HTML's `command` event); whether the
    // target is a popover, and there, is read after it.
    if (command !== null && typeof CommandEvent === "function" && !target.dispatchEvent(new CommandEvent("command", { command, source: row, cancelable: true }))) return;
    if (!target.isConnected || !target.hasAttribute("popover")) return;
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
    if (globalThis.exact?.strayClick?.(e)) return; // the input glue swallows it: a lift off the target the touch began on
    const row = e.target.closest?.("button[popovertarget], button[commandfor]");
    const menu = row?.closest('[popover][role="menu"]');
    // Only a row that opens a popover at the click: every other keeps HTML's own activation (a press
    // retargeting a row between a popover and anything else acts on what it named at the click).
    // And one that would close its popover (a hide, a toggle of an open one) nests nothing: HTML's too.
    const clicked = menu ? invoked(row) : null;
    if (!clicked || clicked.action === "hide" || clicked.action === "toggle" && clicked.target.matches(":popover-open")) return;
    // HTML would act on what the row names before its own press; this acts after it, on the popover it names then.
    e.preventDefault();
    setTimeout(() => act(row, menu, clicked));
  }, true);
  // A button stays pressed while the popover it opened is open (neutral.css reads the mark as it reads a press); not
  // for a context menu, which the Mac opens on a button without highlighting it. Each popover keeps the button it
  // marked: one hidden and shown again from another button in one task fires a single `toggle`, open to open. A button
  // two open popovers marked keeps the mark until both close.
  const marked = new Map();
  const unmark = pop => {
    const source = marked.get(pop);
    marked.delete(pop);
    if (source && ![...marked.values()].includes(source)) source.removeAttribute("data-exact-open");
  };
  // A popover removed while open closes with no `toggle` (HTML's removing steps hide it without events), so while any
  // button is marked each frame looks for its popover gone.
  const sweep = () => {
    for (const pop of marked.keys()) if (!pop.isConnected || !pop.matches(":popover-open")) unmark(pop);
    if (marked.size) requestAnimationFrame(sweep);
  };
  addEventListener("toggle", e => {
    const pop = e.target, source = popoverSource(pop);
    unmark(pop);
    if (e.newState !== "open" || !source?.matches("button") || pop.matches('[style*="--exact-context-x"]')) return;
    source.setAttribute("data-exact-open", "");
    if (!marked.size) requestAnimationFrame(sweep);
    marked.set(pop, source);
  }, true);
}

(globalThis.exact ??= {}).menuRows = menuRows;
