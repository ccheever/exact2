// The viewers' shared chrome (MIT reference, see LICENSE-T3: apps/web/src/components/files/fileSurfaceChrome.tsx,
// components/ui/tooltip.tsx, spinner.tsx, select.tsx and input.tsx; T3 Code 7c3b3b3605ef), as DOM: FileSurfaceAction,
// FileSurfaceNotice, FileSurfaceLoading, FileSurfaceFailure, a Select's trigger and the compact search field. A
// Select's popup cannot leave this page's view, so the app draws it at the point the trigger names (`menu`).
import { ChevronDown, createElement, LoaderCircle } from "./icons";

export type IconNode = [tag: string, attrs: Record<string, string | number>][];

type Child = Node | string | null | undefined | false;

/** An element with its attributes (`class`, `style` and the rest as written) and children. */
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number | boolean | undefined | null> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) {
    if (value === undefined || value === null || value === false) continue;
    element.setAttribute(name, value === true ? "" : String(value));
  }
  for (const child of children) if (child) element.append(child);
  return element;
}

/** A Lucide icon at `size` CSS pixels. */
export function icon(node: IconNode, size = 14, className = ""): SVGElement {
  const svg = createElement(node, { width: size, height: size, "aria-hidden": "true" }) as SVGElement;
  if (className) svg.setAttribute("class", className);
  return svg;
}

/** The page's app (LLP 1020.001 D5), reached by `postMessage` only: on Apple the host forwards it to the window. */
export function post(message: Record<string, unknown> & { readonly type: string }): void {
  window.parent.postMessage(JSON.stringify(message), "/");
}

// ---- Tooltips: Base UI's 600 ms open delay, above the trigger unless the page has no room there.
let tip: HTMLDivElement | null = null;
let tipTimer: ReturnType<typeof setTimeout> | undefined;
let tipOwner: Element | null = null;
function tipElement(): HTMLDivElement {
  tip ??= document.body.appendChild(h("div", { class: "tooltip", role: "tooltip", "aria-hidden": "true" }));
  return tip;
}
export function hideTooltip(owner?: Element): void {
  if (owner && owner !== tipOwner) return;
  clearTimeout(tipTimer);
  tipOwner = null;
  tip?.classList.remove("shown");
}
function showTooltip(owner: HTMLElement, label: () => string): void {
  clearTimeout(tipTimer);
  tipOwner = owner;
  tipTimer = setTimeout(() => {
    if (tipOwner !== owner || !owner.isConnected) return;
    const card = tipElement();
    card.textContent = label();
    const box = owner.getBoundingClientRect();
    card.style.left = "0px";
    card.style.top = "0px";
    card.classList.add("shown");
    const width = card.offsetWidth, height = card.offsetHeight;
    const above = box.top - 4 - height >= 0;
    card.style.top = `${above ? box.top - 4 - height : box.bottom + 4}px`;
    card.style.left = `${Math.max(4, Math.min(window.innerWidth - width - 4, box.left + box.width / 2 - width / 2))}px`;
  }, 600);
}
/** Shows `label` over `owner` while the pointer rests on it, or its focus came from the keyboard. */
export function tooltip(owner: HTMLElement, label: () => string): void {
  owner.addEventListener("pointerenter", () => showTooltip(owner, label));
  owner.addEventListener("pointerleave", () => hideTooltip(owner));
  owner.addEventListener("pointerdown", () => hideTooltip(owner));
  owner.addEventListener("focus", () => { if (owner.matches(":focus-visible")) showTooltip(owner, label); });
  owner.addEventListener("blur", () => hideTooltip(owner));
}

export type SurfaceAction = {
  readonly element: HTMLButtonElement;
  update(state: { label?: string; disabled?: boolean; pressed?: boolean }): void;
};

/** FileSurfaceAction: a 28px ghost icon button with its label as a tooltip; a toggle when `pressed` is given. */
export function surfaceAction(props: { label: string; icon: IconNode; disabled?: boolean; pressed?: boolean; onPress: () => void }): SurfaceAction {
  let label = props.label;
  const toggle = props.pressed !== undefined;
  const button = h("button", { type: "button", class: "surface-action", "aria-label": label }, icon(props.icon));
  button.addEventListener("click", () => { if (!button.disabled) props.onPress(); });
  tooltip(button, () => label);
  const update: SurfaceAction["update"] = (state) => {
    if (state.label !== undefined && state.label !== label) {
      label = state.label;
      button.setAttribute("aria-label", label);
      if (tipOwner === button && tip) tip.textContent = label;
    }
    if (state.disabled !== undefined) {
      button.disabled = state.disabled;
      if (state.disabled) hideTooltip(button);
    }
    if (toggle && state.pressed !== undefined) {
      button.setAttribute("aria-pressed", String(state.pressed));
      button.toggleAttribute("data-pressed", state.pressed);
    }
  };
  update({ disabled: props.disabled ?? false, ...(toggle ? { pressed: props.pressed } : {}) });
  return { element: button, update };
}

/** FileSurfaceNotice: the warning band under a header. */
export function surfaceNotice(text: string): HTMLDivElement {
  return h("div", { class: "surface-notice", role: "status" }, text);
}

/** FileSurfaceLoading: the large spinner, centred. */
export function surfaceLoading(className = ""): HTMLDivElement {
  return h("div", { class: `surface-loading ${className}`.trim(), role: "status", "aria-label": "Loading file" },
    icon(LoaderCircle as IconNode, 20, "spinner"));
}

/** FileSurfaceFailure: the message in the destructive ink, with Try again when the body can load again. */
export function surfaceFailure(message: string, onRetry?: () => void): HTMLDivElement {
  const body = h("div", { class: "surface-failure", role: "alert" }, h("p", { class: "failure-message" }, message));
  if (onRetry) {
    const retry = h("button", { type: "button", class: "failure-retry" }, "Try again");
    retry.addEventListener("click", onRetry);
    body.append(retry);
  }
  return body;
}

/** The documents' "No preview for this file" body (OfficeViewer, OpenXmlOfficeViewer). */
export function noPreview(reason: string): HTMLDivElement {
  return h("div", { class: "no-preview", role: "alert" },
    h("p", { class: "no-preview-title" }, "No preview for this file"),
    h("p", { class: "no-preview-reason" }, `${reason} Save it or open it in another app.`));
}

// ---- Selects: the trigger here, the popup the app's (its rows, the chosen one checked).
export type MenuRow = { readonly id: string; readonly label: string; readonly selected: boolean };
let menuSerial = 0;
const menus = new Map<number, (id: string) => void>();
/** The app's answer to a menu: the chosen row, or none when it closed without one. */
export function menuPicked(serial: number, id: string): void {
  const pick = menus.get(serial);
  menus.clear();
  if (pick && id) pick(id);
}

export type SelectTrigger = {
  readonly element: HTMLButtonElement;
  update(state: { value?: string; disabled?: boolean; busy?: boolean }): void;
};

/** SelectTrigger size "xs": the value, a chevron, and a popup of `rows()` at the trigger when pressed. */
export function selectTrigger(props: { label: string; value: string; rows: () => readonly MenuRow[]; onPick: (id: string) => void }): SelectTrigger {
  const value = h("span", { class: "select-value" }, props.value);
  const button = h("button", { type: "button", class: "select-trigger", "aria-label": props.label, "aria-haspopup": "listbox" },
    value, icon(ChevronDown as IconNode, 14, "select-chevron"));
  let disabled = false;
  button.addEventListener("click", () => {
    if (disabled) return;
    const rows = props.rows();
    if (!rows.length) return;
    const box = button.getBoundingClientRect(), serial = ++menuSerial;
    menus.clear();
    menus.set(serial, props.onPick);
    post({ type: "menu", serial, x: box.left, y: box.top, width: box.width, height: box.height, label: props.label, rows });
  });
  return {
    element: button,
    update(state) {
      if (state.value !== undefined) value.textContent = state.value;
      if (state.disabled !== undefined) disabled = state.disabled;
      if (state.busy !== undefined) {
        button.setAttribute("aria-busy", String(state.busy));
        button.setAttribute("aria-disabled", String(state.busy));
        disabled = state.busy;
      }
    },
  };
}

/** Input size "compact", type search. */
export function searchField(label: string, onInput: (value: string) => void): HTMLInputElement {
  const input = h("input", { type: "search", class: "search-input", "aria-label": label, placeholder: label, autocomplete: "off", spellcheck: "false" });
  input.addEventListener("input", () => onInput(input.value));
  return input;
}

/** `observeResize`: the callback runs on each size change of `node`; the returned function stops it. */
export function observeResize(node: Element, callback: (entries: ResizeObserverEntry[]) => void): () => void {
  const observer = new ResizeObserver(callback);
  observer.observe(node);
  return () => observer.disconnect();
}

/** A size line's "· 1.2 MB", or "· 0 bytes" for an empty file (LargeTextViewer, NotebookPreview). */
export const sizeSuffix = (sizeBytes: number | null | undefined, format: (bytes: number) => string): string =>
  sizeBytes === null || sizeBytes === undefined ? "" : ` · ${sizeBytes === 0 ? "0 bytes" : format(sizeBytes)}`;
