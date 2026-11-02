// The Browser tab's annotation overlay, a classic script data code runs in a sandbox world of the tab's frame
// (browser-annotate.ts, `@exact/browser-automation`'s `callFunction` with a `sandbox`): it sees the page's DOM, not
// its JS, and the page cannot call it. MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/preview/PickPreload.ts (startAnnotation and its helpers), AnnotationKeyboard.ts
// (resolveAnnotationSubmission); T3 Code 74541bab17: PickPreload.ts `unionRects`, AnnotationIds.ts; T3 Code 7c3b3b3605ef:
// PickPreload.ts's input layer over a PDF. Its stylesheet, `STYLES`, is assets/browser-annotate.css, which
// browser-annotate.ts declares around this script. Not ported: PickPreload's recording cursor and human-input
// listeners (browser-recording.js has the recording side) and the mouse back/forward buttons. Electron's
// react-grab `getElementContext` reads React from the page's own world; here this world fills the DOM facts
// (selector, HTML preview, styles) and marks each picked element `data-t3code-pick`, and data code reads the
// component name and source from React's fiber in the page's world before it crops the screenshot.
//
// The page installs it once per document as `globalThis.__t3codeAnnotate` ({ start(theme), cancel(),
// captured(), applyTheme(theme), state(), next() }). Its outcomes, `{ type: "picked", annotation, rect, submission }`
// and `{ type: "cancelled" }`, wait in order for `next()`, a promise that resolves with the next one as JSON: data code
// awaits it with one `evaluate`, since a frame's page posts nothing to the app. Loaded by `require` (bun tests) it
// exports its pure helpers and touches no window or document.
(function () {
  "use strict";

  // ── Pure helpers (exported to the tests) ────────────────────────────────────────────────────────────────

  /** AnnotationKeyboard.ts resolveAnnotationSubmission: Enter attaches, ⌘/Ctrl+Enter sends; Shift+Enter and
   *  composition stay with the editor. */
  function resolveAnnotationSubmission(event) {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return null;
    return event.metaKey || event.ctrlKey ? "send" : "attach";
  }

  function normalizeRect(startX, startY, endX, endY) {
    return {
      x: Math.min(startX, endX),
      y: Math.min(startY, endY),
      width: Math.abs(endX - startX),
      height: Math.abs(endY - startY),
    };
  }

  function isUsableRect(rect) {
    return rect.width >= 3 && rect.height >= 3;
  }

  /** The targets' union grown by `padding` (PickPreload unionRects). Data code clips it to the captured viewport;
   *  clipping here would lose the bounds of targets scrolled out of view. */
  function unionRects(rects, padding) {
    if (padding === undefined) padding = 20;
    if (rects.length === 0) return null;
    const left = Math.min(...rects.map((rect) => rect.x));
    const top = Math.min(...rects.map((rect) => rect.y));
    const right = Math.max(...rects.map((rect) => rect.x + rect.width));
    const bottom = Math.max(...rects.map((rect) => rect.y + rect.height));
    return {
      x: left - padding,
      y: top - padding,
      width: right - left + padding * 2,
      height: bottom - top + padding * 2,
    };
  }

  /** AnnotationIds.ts createAnnotationIdSource: ids for annotations and their targets, unique per page load. The composer
   *  keeps annotations by id across reloads and this script starts again on every load, so a bare counter would reissue
   *  ids a draft already holds. `getRandomValues`, not `randomUUID`: plain-http pages are not secure contexts. */
  function createAnnotationIdSource(randomValues) {
    const pageLoad = Array.from(randomValues(new Uint32Array(2)), (part) => part.toString(36)).join("");
    let sequence = 0;
    return (prefix) => {
      sequence += 1;
      return `${prefix}_${pageLoad}_${sequence.toString(36)}`;
    };
  }

  function pathFromPoints(points) {
    if (points.length === 0) return "";
    if (points.length === 1) return `M ${points[0].x} ${points[0].y} l 0.01 0.01`;
    let path = `M ${points[0].x} ${points[0].y}`;
    for (let index = 1; index < points.length - 1; index += 1) {
      const current = points[index];
      const next = points[index + 1];
      path += ` Q ${current.x} ${current.y} ${(current.x + next.x) / 2} ${(current.y + next.y) / 2}`;
    }
    const last = points[points.length - 1];
    path += ` L ${last.x} ${last.y}`;
    return path;
  }

  function strokeBounds(points, width) {
    const xs = points.map((point) => point.x);
    const ys = points.map((point) => point.y);
    const padding = width + 3;
    const left = Math.min(...xs) - padding;
    const top = Math.min(...ys) - padding;
    const right = Math.max(...xs) + padding;
    const bottom = Math.max(...ys) + padding;
    return { x: left, y: top, width: right - left, height: bottom - top };
  }

  /** `tag#id.first.second` for an element's label. Takes anything with tagName, id and className. */
  function describeRawElement(element) {
    const tag = String(element.tagName || "").toLowerCase();
    const id = element.id ? `#${element.id}` : "";
    const classes =
      typeof element.className === "string"
        ? element.className
            .trim()
            .split(/\s+/)
            .filter(Boolean)
            .slice(0, 2)
            .map((name) => `.${name}`)
            .join("")
        : "";
    return `${tag}${id}${classes}`;
  }

  const api = {
    resolveAnnotationSubmission,
    normalizeRect,
    isUsableRect,
    unionRects,
    createAnnotationIdSource,
    pathFromPoints,
    strokeBounds,
    describeRawElement,
  };
  if (typeof module === "object" && module && module.exports) {
    module.exports = api;
    return;
  }
  if (globalThis.__t3codeAnnotate) return;

  // ── The overlay (PickPreload startAnnotation) ───────────────────────────────────────────────────────────

  const OVERLAY_ATTRIBUTE = "data-t3code-annotation-ui";
  const INPUT_LAYER_ATTRIBUTE = "data-t3code-annotation-input";
  const PICK_ATTRIBUTE = "data-t3code-pick";
  const Z_INDEX_OVERLAY = 2147483646;
  const PRIMARY = "var(--t3-primary)";
  const PRIMARY_FILL = "color-mix(in srgb, var(--t3-primary) 10%, transparent)";
  const MAX_MARQUEE_ELEMENTS = 20;
  const CONTENT_LAYER_Z_INDEX = 1;
  const CHROME_LAYER_Z_INDEX = 10;
  /** Truncation for the DOM-only preview (PickPreload HTML_PREVIEW_MAX_CHARS). */
  const HTML_PREVIEW_MAX_CHARS = 500;
  const STYLE_SUMMARY_MAX_CHARS = 2000;
  const SELECTOR_MAX_DEPTH = 12;
  /** Manager.ts DEFAULT_ANNOTATION_THEME: what the overlay wears until the app sends its own. */
  const DEFAULT_THEME = {
    colorScheme: "light",
    radius: "0.625rem",
    background: "white",
    foreground: "oklch(0.269 0 0)",
    popover: "white",
    popoverForeground: "oklch(0.269 0 0)",
    primary: "oklch(0.488 0.217 264)",
    primaryForeground: "white",
    muted: "rgb(0 0 0 / 4%)",
    mutedForeground: "oklch(0.556 0 0)",
    accent: "rgb(0 0 0 / 4%)",
    accentForeground: "oklch(0.269 0 0)",
    border: "rgb(0 0 0 / 8%)",
    input: "rgb(0 0 0 / 10%)",
    ring: "oklch(0.488 0.217 264)",
    fontSans: "system-ui, sans-serif",
    fontMono: "ui-monospace, monospace",
  };

  let activeSession = null;
  let annotationTheme = DEFAULT_THEME;

  // The outcomes `next()` has not taken yet, and the resolver of the one waiting (the promise stays reachable through it).
  const outbox = [];
  let waiting = null;
  function post(message) {
    const text = JSON.stringify(message);
    if (waiting) {
      const resolve = waiting;
      waiting = null;
      resolve(text);
    } else outbox.push(text);
  }

  function readTheme(theme) {
    const next = { ...DEFAULT_THEME };
    if (theme && typeof theme === "object") {
      for (const key of Object.keys(DEFAULT_THEME)) {
        if (typeof theme[key] === "string" && theme[key].trim()) next[key] = theme[key];
      }
    }
    return next;
  }

  function applyAnnotationTheme(host, theme) {
    if (!theme) return;
    host.style.colorScheme = theme.colorScheme;
    const variables = {
      "--t3-radius": theme.radius,
      "--t3-background": theme.background,
      "--t3-foreground": theme.foreground,
      "--t3-popover": theme.popover,
      "--t3-popover-foreground": theme.popoverForeground,
      "--t3-primary": theme.primary,
      "--t3-primary-foreground": theme.primaryForeground,
      "--t3-muted": theme.muted,
      "--t3-muted-foreground": theme.mutedForeground,
      "--t3-accent": theme.accent,
      "--t3-accent-foreground": theme.accentForeground,
      "--t3-border": theme.border,
      "--t3-input": theme.input,
      "--t3-ring": theme.ring,
      "--t3-font-sans": theme.fontSans,
      "--t3-font-mono": theme.fontMono,
    };
    for (const [name, value] of Object.entries(variables)) host.style.setProperty(name, value);
  }

  const nextId = createAnnotationIdSource((values) => crypto.getRandomValues(values));

  const rectFromDomRect = (rect) => ({ x: rect.left, y: rect.top, width: rect.width, height: rect.height });

  function isAnnotationNode(element) {
    return element instanceof Element && element.closest(`[${OVERLAY_ATTRIBUTE}]`) !== null;
  }

  function pickFromPoint(clientX, clientY) {
    for (const candidate of document.elementsFromPoint(clientX, clientY)) {
      if (!(candidate instanceof Element)) continue;
      if (isAnnotationNode(candidate) || candidate.hasAttribute(INPUT_LAYER_ATTRIBUTE)) continue;
      if (candidate === document.documentElement || candidate === document.body) continue;
      return candidate;
    }
    return null;
  }

  function createBox(color, fill) {
    const node = document.createElement("div");
    node.setAttribute(OVERLAY_ATTRIBUTE, "");
    node.style.cssText = [
      "position:fixed",
      "pointer-events:none",
      `border:2px solid ${color}`,
      `background:${fill}`,
      "border-radius:3px",
      "box-sizing:border-box",
      "display:none",
      `z-index:${CONTENT_LAYER_Z_INDEX}`,
    ].join(";");
    return node;
  }

  function positionBox(node, rect) {
    node.style.display = "block";
    node.style.transform = `translate(${rect.x}px, ${rect.y}px)`;
    node.style.width = `${rect.width}px`;
    node.style.height = `${rect.height}px`;
  }

  function createLabel() {
    const label = document.createElement("div");
    label.setAttribute(OVERLAY_ATTRIBUTE, "");
    label.className =
      "fixed z-1 max-w-70 overflow-hidden rounded-md bg-primary px-2 py-1 font-sans text-xs font-semibold text-primary-foreground shadow-md";
    label.style.cssText = [
      "position:fixed",
      "pointer-events:none",
      "white-space:nowrap",
      "text-overflow:ellipsis",
      `z-index:${CONTENT_LAYER_Z_INDEX}`,
    ].join(";");
    return label;
  }

  function updateSelectedVisual(target) {
    if (!target.element.isConnected) {
      target.outline.style.display = "none";
      target.label.style.display = "none";
      return;
    }
    const rect = target.element.getBoundingClientRect();
    positionBox(target.outline, rectFromDomRect(rect));
    target.label.textContent = describeRawElement(target.element);
    target.label.style.display = "block";
    target.label.style.transform = `translate(${Math.max(4, rect.left)}px, ${Math.max(4, rect.top - 22)}px)`;
  }

  // ── The DOM half of react-grab's element context ─────────────────────────────────────────────────────────

  function cssEscape(value) {
    if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(value);
    return String(value).replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character}`);
  }

  function selects(selector, element) {
    try {
      const matches = document.querySelectorAll(selector);
      return matches.length === 1 && matches[0] === element;
    } catch (_) {
      return false;
    }
  }

  /** A selector that matches the element alone: its id when unique, else an nth-of-type path from the nearest
   *  unique id (or the root), as short as uniqueness allows; null when none is found within the bound. */
  function uniqueSelector(element) {
    try {
      if (element.id && selects(`#${cssEscape(element.id)}`, element)) return `#${cssEscape(element.id)}`;
      const parts = [];
      let node = element;
      for (let depth = 0; node && node.nodeType === 1 && depth < SELECTOR_MAX_DEPTH; depth += 1) {
        if (node !== element && node.id && document.querySelectorAll(`#${cssEscape(node.id)}`).length === 1) {
          parts.unshift(`#${cssEscape(node.id)}`);
          const selector = parts.join(" > ");
          return selects(selector, element) ? selector : null;
        }
        const tag = node.tagName.toLowerCase();
        let part = tag;
        const parent = node.parentElement;
        if (parent) {
          const siblings = Array.from(parent.children).filter((child) => child.tagName === node.tagName);
          if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
        }
        parts.unshift(part);
        const selector = parts.join(" > ");
        if (selects(selector, element)) return selector;
        node = parent;
      }
      return null;
    } catch (_) {
      return null;
    }
  }

  const STYLE_PROPERTIES = [
    "display", "position", "box-sizing", "width", "height", "margin", "padding", "gap", "flex-direction",
    "align-items", "justify-content", "color", "background-color", "font-family", "font-size", "font-weight",
    "line-height", "text-align", "border", "border-radius", "box-shadow", "opacity", "overflow", "z-index",
  ];

  /** The element's notable computed styles, one `property: value;` a line, bounded. */
  function describeStyles(element) {
    try {
      const computed = getComputedStyle(element);
      const lines = [];
      for (const property of STYLE_PROPERTIES) {
        const value = computed.getPropertyValue(property).trim();
        if (value) lines.push(`${property}: ${value};`);
      }
      return lines.join("\n").slice(0, STYLE_SUMMARY_MAX_CHARS);
    } catch (_) {
      return "";
    }
  }

  /** PickPreload captureElement, the facts this world can read; data code adds the component name, its source
   *  and stack from the page's world (null and empty until then). */
  function captureElement(element) {
    return {
      pageUrl: location.href,
      pageTitle: (document.title || "").trim() || null,
      tagName: element.tagName.toLowerCase(),
      selector: uniqueSelector(element),
      htmlPreview: String(element.outerHTML || "").slice(0, HTML_PREVIEW_MAX_CHARS),
      componentName: null,
      source: null,
      stack: [],
      styles: describeStyles(element),
      pickedAt: new Date().toISOString(),
    };
  }

  // ── Controls ─────────────────────────────────────────────────────────────────────────────────────────────

  function createButton(label, title) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = label;
    button.title = title;
    button.className =
      "inline-flex h-7 cursor-pointer items-center justify-center rounded-md border border-transparent px-2 font-sans text-xs font-medium text-foreground outline-none hover:bg-accent disabled:pointer-events-none disabled:opacity-60";
    return button;
  }

  function styleControl(input) {
    input.setAttribute("aria-label", input.getAttribute("aria-label") || "Style value");
    input.className =
      "h-7 min-w-0 w-full appearance-none rounded-md border border-input bg-background px-2 font-mono text-xs text-foreground shadow-xs outline-none";
  }

  function createUnitControl(input) {
    const wrapper = document.createElement("div");
    wrapper.style.cssText = "position:relative;min-width:0";
    const unit = document.createElement("span");
    unit.textContent = input.dataset.unit || "";
    unit.className =
      "pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 font-mono text-xs text-muted-foreground";
    wrapper.append(input, unit);
    return wrapper;
  }

  function createField(labelText, input) {
    const label = document.createElement("label");
    label.className =
      "grid min-h-7 grid-cols-[82px_minmax(0,1fr)] items-center gap-2 font-sans text-xs font-medium text-muted-foreground";
    const text = document.createElement("span");
    text.textContent = labelText;
    styleControl(input);
    label.append(text, input instanceof HTMLInputElement && input.dataset.unit ? createUnitControl(input) : input);
    return label;
  }

  function createStyleSection() {
    const section = document.createElement("section");
    section.className = "grid gap-1 border-t border-border py-2";
    return section;
  }

  function createUnitInput(unit, placeholder) {
    const input = document.createElement("input");
    input.type = "number";
    input.placeholder = placeholder === undefined ? "0" : placeholder;
    input.style.paddingRight = "30px";
    input.dataset.unit = unit;
    return input;
  }

  function startAnnotation() {
    if (activeSession) activeSession.teardown(false);
    let finished = false;
    const host = document.createElement("div");
    host.setAttribute(OVERLAY_ATTRIBUTE, "");
    host.setAttribute("dir", "ltr");
    host.style.cssText = `position:fixed;inset:0;z-index:${Z_INDEX_OVERLAY};pointer-events:none`;
    applyAnnotationTheme(host, annotationTheme);
    const shadowRoot = host.attachShadow({ mode: "closed" });
    const themeStyle = document.createElement("style");
    themeStyle.textContent = STYLES;
    shadowRoot.appendChild(themeStyle);

    const root = document.createElement("div");
    root.setAttribute(OVERLAY_ATTRIBUTE, "");
    root.setAttribute("dir", "ltr");
    root.className = "fixed inset-0 font-sans text-foreground";
    root.style.cssText = "pointer-events:none";
    const cursorStyle = document.createElement("style");
    cursorStyle.setAttribute(OVERLAY_ATTRIBUTE, "");
    cursorStyle.textContent = `html[data-t3code-annotation-tool] body, html[data-t3code-annotation-tool] body * { cursor: crosshair !important; } [${OVERLAY_ATTRIBUTE}], [${OVERLAY_ATTRIBUTE}] * { cursor: default !important; } [${OVERLAY_ATTRIBUTE}] input[type=number]::-webkit-inner-spin-button, [${OVERLAY_ATTRIBUTE}] input[type=number]::-webkit-outer-spin-button { appearance:none; margin:0; }`;
    document.documentElement.appendChild(cursorStyle);
    shadowRoot.appendChild(root);

    const hoverOutline = createBox(PRIMARY, PRIMARY_FILL);
    const marqueeBox = createBox(PRIMARY, PRIMARY_FILL);
    root.append(hoverOutline, marqueeBox);

    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute(OVERLAY_ATTRIBUTE, "");
    svg.setAttribute("width", "100%");
    svg.setAttribute("height", "100%");
    svg.setAttribute("viewBox", `0 0 ${window.innerWidth} ${window.innerHeight}`);
    svg.style.cssText = "position:fixed;inset:0;overflow:visible;pointer-events:none";
    svg.style.zIndex = String(CONTENT_LAYER_Z_INDEX);
    root.appendChild(svg);

    const toolbar = document.createElement("div");
    toolbar.setAttribute(OVERLAY_ATTRIBUTE, "");
    toolbar.className =
      "pointer-events-auto fixed top-2.5 left-1/2 flex -translate-x-1/2 gap-0.5 rounded-lg border border-border bg-popover/95 p-1 text-popover-foreground shadow-lg backdrop-blur-xl";
    toolbar.style.zIndex = String(CHROME_LAYER_Z_INDEX);
    root.appendChild(toolbar);

    const editor = document.createElement("div");
    editor.setAttribute(OVERLAY_ATTRIBUTE, "");
    editor.className =
      "pointer-events-auto fixed hidden max-h-[calc(100vh-16px)] w-[min(360px,calc(100vw-16px))] flex-col overflow-hidden rounded-xl border border-border bg-popover/96 text-popover-foreground shadow-2xl backdrop-blur-xl";
    editor.style.zIndex = String(CHROME_LAYER_Z_INDEX);
    root.appendChild(editor);

    const composerRow = document.createElement("div");
    composerRow.className = "flex items-start gap-2 p-2";

    const adjust = createButton("", "Expand annotation editor");
    adjust.setAttribute("aria-label", "Expand annotation editor");
    adjust.setAttribute("aria-expanded", "false");
    adjust.className += " h-8 w-8 shrink-0 bg-muted p-0 text-muted-foreground hover:bg-accent hover:text-accent-foreground";
    adjust.innerHTML =
      '<svg viewBox="0 0 20 20" width="15" height="15" aria-hidden="true"><path d="M4 5h12M4 10h12M4 15h12M7 3v4M13 8v4M9 13v4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>';
    composerRow.appendChild(adjust);

    const comment = document.createElement("textarea");
    comment.placeholder = "Describe the change…";
    comment.setAttribute("dir", "auto");
    comment.rows = 1;
    comment.className =
      "min-h-8 max-h-24 min-w-0 flex-1 resize-none overflow-y-hidden border-0 border-b border-b-transparent bg-transparent px-0 py-1.5 font-sans text-sm leading-5 text-foreground outline-none ring-0 placeholder:text-muted-foreground focus:border-b-primary focus:outline-none focus:ring-0";
    composerRow.appendChild(comment);

    const dragHandle = document.createElement("button");
    dragHandle.type = "button";
    dragHandle.textContent = "⠿";
    dragHandle.title = "Drag annotation editor";
    dragHandle.className =
      "hidden h-8 w-6 shrink-0 cursor-grab select-none border-0 bg-transparent p-0 font-sans text-lg font-bold leading-5 text-muted-foreground";
    composerRow.appendChild(dragHandle);

    const submit = createButton("Attach", "Attach annotation and screenshot (Enter)");
    submit.className += " h-8 shrink-0 border-primary bg-primary px-3 text-primary-foreground shadow-sm hover:bg-primary/90";
    composerRow.appendChild(submit);
    editor.appendChild(composerRow);

    const stylePanel = document.createElement("div");
    stylePanel.className =
      "hidden max-h-[min(176px,calc(100vh-180px))] overflow-auto border-t border-border bg-muted/40 px-3";
    editor.appendChild(stylePanel);

    const selected = new Map();
    const regions = [];
    const strokes = [];
    const styleChanges = new Map();
    const toolButtons = new Map();
    /** The style panel's controls by their field label (the tests' handle on the closed shadow root). */
    const fields = new Map();
    // WebKit draws a PDF in its own plugin view, so pointer and key input over it never reaches this document. A
    // transparent layer above the viewer catches the pointer and takes the focus for Escape and the tool keys. A PDF has
    // no elements to select, so it starts on Region.
    const isPdf = document.contentType === "application/pdf";
    const inputLayer = isPdf ? document.createElement("div") : null;
    if (inputLayer) {
      inputLayer.setAttribute(INPUT_LAYER_ATTRIBUTE, "");
      inputLayer.tabIndex = -1;
      inputLayer.style.cssText = `position:fixed;inset:0;z-index:${Z_INDEX_OVERLAY - 1};cursor:crosshair;outline:none`;
    }
    let tool = isPdf ? "marquee" : "select";
    let dragStart = null;
    let activeStroke = null;
    let pendingCapture = false;
    let editorExpanded = false;
    let editorWasShown = false;
    let editorPosition = null;
    let editorDrag = null;
    let editorLayoutFrame = null;

    const resizeComment = () => {
      const maxHeight = 96;
      comment.style.height = "auto";
      const nextHeight = Math.min(comment.scrollHeight, maxHeight);
      comment.style.height = `${nextHeight}px`;
      comment.style.overflowY = comment.scrollHeight > maxHeight ? "auto" : "hidden";
      queueEditorLayout();
    };
    comment.addEventListener("input", resizeComment);

    const updateStatus = () => {
      const hasTargets = selected.size > 0 || regions.length > 0 || strokes.length > 0;
      editor.style.display = hasTargets ? "flex" : "none";
      submit.disabled = !hasTargets;
      submit.style.opacity = hasTargets ? "1" : "0.45";
      adjust.disabled = !hasTargets;
      stylePanel.style.display = editorExpanded && selected.size > 0 ? "grid" : "none";
      queueEditorLayout();
      if (hasTargets && !editorWasShown) {
        editorWasShown = true;
        window.setTimeout(() => comment.focus({ preventScroll: true }), 0);
      }
    };

    const refreshToolButtons = () => {
      for (const [candidate, button] of toolButtons) {
        const active = candidate === tool;
        button.classList.toggle("bg-primary/10", active);
        button.classList.toggle("text-primary", active);
        button.classList.toggle("text-foreground", !active);
      }
      if (tool !== "select") hoverOutline.style.display = "none";
      if (tool !== "marquee") marqueeBox.style.display = "none";
      document.documentElement.setAttribute("data-t3code-annotation-tool", tool);
    };

    const restoreBaselines = (target) => {
      if (!(target.element instanceof HTMLElement || target.element instanceof SVGElement)) return;
      for (const [property, baseline] of target.baselineStyles) {
        if (baseline) target.element.style.setProperty(property, baseline);
        else target.element.style.removeProperty(property);
      }
    };

    const removeSelected = (target) => {
      restoreBaselines(target);
      selected.delete(target.element);
      target.outline.remove();
      target.label.remove();
      for (const [key, change] of styleChanges) {
        if (change.targetId === target.id) styleChanges.delete(key);
      }
      updateStatus();
    };

    const addSelected = (element) => {
      if (selected.has(element)) return;
      const target = {
        id: nextId("element"),
        element,
        outline: createBox(PRIMARY, PRIMARY_FILL),
        label: createLabel(),
        baselineStyles: new Map(),
      };
      selected.set(element, target);
      root.append(target.outline, target.label);
      updateSelectedVisual(target);
      updateStatus();
      if (editorExpanded) {
        stylePanel.style.display = "grid";
        syncStyleControls();
      }
    };

    const toggleSelected = (element, additive) => {
      const existing = selected.get(element);
      if (existing) {
        removeSelected(existing);
        return;
      }
      if (!additive) {
        for (const target of Array.from(selected.values())) removeSelected(target);
      }
      addSelected(element);
    };

    const setStyleForSelected = (property, value) => {
      for (const target of selected.values()) {
        if (!(target.element instanceof HTMLElement || target.element instanceof SVGElement)) continue;
        if (!target.baselineStyles.has(property)) {
          target.baselineStyles.set(property, target.element.style.getPropertyValue(property));
        }
        const key = `${target.id}:${property}`;
        const previousValue =
          styleChanges.get(key)?.previousValue ?? getComputedStyle(target.element).getPropertyValue(property).trim();
        target.element.style.setProperty(property, value, "important");
        styleChanges.set(key, { targetId: target.id, selector: null, property, previousValue, value });
        updateSelectedVisual(target);
      }
    };

    const textSection = createStyleSection();
    const colorsSection = createStyleSection();
    const bordersSection = createStyleSection();
    const sizingSection = createStyleSection();
    stylePanel.append(textSection, colorsSection, bordersSection, sizingSection);
    const field = (labelText, input) => {
      fields.set(labelText, input);
      return createField(labelText, input);
    };

    const fontFamily = document.createElement("select");
    for (const value of ["inherit", "system-ui", "sans-serif", "serif", "monospace"]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value;
      fontFamily.appendChild(option);
    }
    fontFamily.addEventListener("change", () => setStyleForSelected("font-family", fontFamily.value));
    textSection.appendChild(field("Font", fontFamily));

    const fontSize = createUnitInput("px", "16");
    fontSize.min = "1";
    fontSize.max = "300";
    fontSize.addEventListener("input", () => {
      if (fontSize.value) setStyleForSelected("font-size", `${fontSize.value}px`);
    });
    textSection.appendChild(field("Font size", fontSize));

    const fontWeight = document.createElement("select");
    for (const value of ["300", "400", "500", "600", "700", "800", "900"]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value;
      fontWeight.appendChild(option);
    }
    fontWeight.addEventListener("change", () => setStyleForSelected("font-weight", fontWeight.value));
    textSection.appendChild(field("Font weight", fontWeight));

    const lineHeight = document.createElement("input");
    lineHeight.type = "text";
    lineHeight.placeholder = "normal / 1.4";
    lineHeight.addEventListener("change", () => {
      if (lineHeight.value.trim()) setStyleForSelected("line-height", lineHeight.value.trim());
    });
    textSection.appendChild(field("Line height", lineHeight));

    const createColorRow = (labelText, property, section) => {
      const row = document.createElement("label");
      row.className =
        "grid min-h-7 grid-cols-[82px_minmax(0,1fr)] items-center gap-2 font-sans text-xs font-medium text-muted-foreground";
      const label = document.createElement("span");
      label.textContent = labelText;
      const control = document.createElement("div");
      control.className =
        "grid h-7 grid-cols-[22px_minmax(0,1fr)] items-center gap-1 rounded-md border border-input bg-background px-1 shadow-xs";
      const color = document.createElement("input");
      color.type = "color";
      color.setAttribute("aria-label", labelText);
      color.style.cssText =
        "width:20px;height:20px;padding:0;border:0;border-radius:5px;overflow:hidden;background:transparent;cursor:pointer";
      const text = document.createElement("input");
      text.type = "text";
      text.setAttribute("aria-label", `${labelText} value`);
      text.className = "min-w-0 w-full border-0 bg-transparent font-mono text-xs text-foreground outline-none";
      color.addEventListener("input", () => {
        text.value = color.value;
        setStyleForSelected(property, color.value);
      });
      text.addEventListener("change", () => {
        const value = text.value.trim();
        if (!value) return;
        setStyleForSelected(property, value);
        if (/^#[0-9a-f]{6}$/i.test(value)) color.value = value;
      });
      control.append(color, text);
      row.append(label, control);
      section.appendChild(row);
      fields.set(labelText, text);
      return { row, color, text };
    };

    const textColor = createColorRow("Text color", "color", colorsSection);
    const backgroundColor = createColorRow("Background", "background-color", colorsSection);

    const opacity = document.createElement("input");
    opacity.type = "range";
    opacity.min = "0";
    opacity.max = "1";
    opacity.step = "0.05";
    opacity.value = "1";
    opacity.style.accentColor = PRIMARY;
    opacity.addEventListener("input", () => setStyleForSelected("opacity", opacity.value));
    colorsSection.appendChild(field("Opacity", opacity));

    const radius = createUnitInput("px", "0");
    radius.min = "0";
    radius.max = "300";
    radius.addEventListener("input", () => {
      if (radius.value) setStyleForSelected("border-radius", `${radius.value}px`);
    });
    bordersSection.appendChild(field("Radius", radius));

    const borderColor = createColorRow("Border color", "border-color", bordersSection);

    const borderWidth = createUnitInput("px", "0");
    borderWidth.min = "0";
    borderWidth.max = "100";
    borderWidth.addEventListener("input", () => {
      if (borderWidth.value) {
        setStyleForSelected("border-style", "solid");
        setStyleForSelected("border-width", `${borderWidth.value}px`);
      }
    });
    bordersSection.appendChild(field("Border width", borderWidth));

    const dimensions = document.createElement("div");
    dimensions.style.cssText = "display:grid;grid-template-columns:82px minmax(0,1fr);gap:8px;align-items:center";
    const dimensionLabel = document.createElement("div");
    dimensionLabel.className = "grid gap-2 font-sans text-xs font-medium text-muted-foreground";
    dimensionLabel.innerHTML = "<span>Width</span><span>Height</span>";
    const dimensionControls = document.createElement("div");
    dimensionControls.style.cssText = "position:relative;display:grid;gap:3px;padding-left:22px";
    const widthInput = createUnitInput("px", "auto");
    const heightInput = createUnitInput("px", "auto");
    styleControl(widthInput);
    styleControl(heightInput);
    fields.set("Width", widthInput);
    fields.set("Height", heightInput);
    const aspectLock = createButton("", "Lock aspect ratio");
    aspectLock.setAttribute("aria-pressed", "true");
    aspectLock.style.cssText +=
      ";position:absolute;left:0;top:50%;transform:translateY(-50%);width:18px;height:38px;padding:0";
    aspectLock.className += " bg-primary/10 text-primary";
    dimensionControls.append(createUnitControl(widthInput), createUnitControl(heightInput), aspectLock);
    dimensions.append(dimensionLabel, dimensionControls);
    sizingSection.appendChild(dimensions);

    let aspectLocked = true;
    let aspectRatio = 1;
    const refreshAspectButton = () => {
      aspectLock.innerHTML = aspectLocked
        ? '<svg viewBox="0 0 20 20" width="14" height="14" aria-hidden="true"><path d="M8 6.5 9.5 5A3.5 3.5 0 0 1 14.5 10l-1.5 1.5M12 13.5 10.5 15A3.5 3.5 0 0 1 5.5 10L7 8.5M7.5 12.5l5-5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>'
        : '<svg viewBox="0 0 20 20" width="14" height="14" aria-hidden="true"><path d="m6 6 8 8M8 6.5 9.5 5A3.5 3.5 0 0 1 14 9M12 13.5 10.5 15A3.5 3.5 0 0 1 6 11" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>';
      aspectLock.setAttribute("aria-pressed", String(aspectLocked));
      aspectLock.classList.toggle("bg-primary/10", aspectLocked);
      aspectLock.classList.toggle("text-primary", aspectLocked);
      aspectLock.classList.toggle("bg-muted", !aspectLocked);
      aspectLock.classList.toggle("text-muted-foreground", !aspectLocked);
    };
    aspectLock.addEventListener("click", () => {
      aspectLocked = !aspectLocked;
      refreshAspectButton();
    });
    widthInput.addEventListener("input", () => {
      const width = Number(widthInput.value);
      if (!Number.isFinite(width) || width <= 0) return;
      setStyleForSelected("width", `${width}px`);
      if (aspectLocked && aspectRatio > 0) {
        const height = Math.max(1, Math.round(width / aspectRatio));
        heightInput.value = String(height);
        setStyleForSelected("height", `${height}px`);
      }
    });
    heightInput.addEventListener("input", () => {
      const height = Number(heightInput.value);
      if (!Number.isFinite(height) || height <= 0) return;
      setStyleForSelected("height", `${height}px`);
      if (aspectLocked && aspectRatio > 0) {
        const width = Math.max(1, Math.round(height * aspectRatio));
        widthInput.value = String(width);
        setStyleForSelected("width", `${width}px`);
      }
    });
    refreshAspectButton();

    const addSpacingField = (label, property, placeholder) => {
      const input = document.createElement("input");
      input.type = "text";
      input.placeholder = placeholder;
      input.addEventListener("change", () => {
        if (input.value.trim()) setStyleForSelected(property, input.value.trim());
      });
      sizingSection.appendChild(field(label, input));
      return input;
    };
    const padding = addSpacingField("Padding", "padding", "0 0 0 0");
    const margin = addSpacingField("Margin", "margin", "0 0 0 0");
    const gap = addSpacingField("Gap", "gap", "0px");

    const syncStyleControls = () => {
      const first = selected.values().next().value;
      if (!first) return;
      const computed = getComputedStyle(first.element);
      const rect = first.element.getBoundingClientRect();
      aspectRatio = rect.height > 0 ? rect.width / rect.height : 1;
      widthInput.value = String(Math.round(rect.width));
      heightInput.value = String(Math.round(rect.height));
      fontSize.value = String(Math.round(Number.parseFloat(computed.fontSize) || 16));
      fontWeight.value = /^[0-9]+$/.test(computed.fontWeight) ? computed.fontWeight : "400";
      lineHeight.value = computed.lineHeight;
      fontFamily.value = Array.from(fontFamily.options).some((option) => option.value === computed.fontFamily)
        ? computed.fontFamily
        : "inherit";
      textColor.text.value = computed.color;
      backgroundColor.text.value = computed.backgroundColor;
      borderColor.text.value = computed.borderColor;
      opacity.value = computed.opacity;
      radius.value = String(Math.round(Number.parseFloat(computed.borderRadius) || 0));
      borderWidth.value = String(Math.round(Number.parseFloat(computed.borderWidth) || 0));
      padding.value = computed.padding;
      margin.value = computed.margin;
      gap.value = computed.gap === "normal" ? "0px" : computed.gap;
    };

    const tools = [
      ["select", "Select", "Select elements (V)"],
      ["marquee", "Region", "Draw a region or marquee-select elements (R)"],
      ["draw", "Draw", "Draw freehand (D)"],
      ["erase", "Erase", "Remove an annotation target (E)"],
    ];
    for (const [candidate, label, title] of tools) {
      const button = createButton(label, title);
      button.className += " h-8 px-2.5 text-sm";
      button.addEventListener("click", () => {
        tool = candidate;
        refreshToolButtons();
      });
      toolButtons.set(candidate, button);
      toolbar.appendChild(button);
    }

    const clampEditorPosition = (left, top) => {
      const edge = 8;
      const rect = editor.getBoundingClientRect();
      return {
        left: Math.min(Math.max(edge, left), Math.max(edge, window.innerWidth - rect.width - edge)),
        top: Math.min(Math.max(edge, top), Math.max(edge, window.innerHeight - rect.height - edge)),
      };
    };

    const applyEditorPosition = (position) => {
      const clamped = clampEditorPosition(position.left, position.top);
      editor.style.left = `${clamped.left}px`;
      editor.style.top = `${clamped.top}px`;
      editor.style.right = "auto";
      editor.style.bottom = "auto";
      if (editorExpanded) editorPosition = clamped;
    };

    const getAnnotationBounds = () =>
      unionRects(
        [
          ...Array.from(selected.values(), (target) => rectFromDomRect(target.element.getBoundingClientRect())),
          ...regions.map((region) => region.rect),
          ...strokes.map((stroke) => stroke.bounds),
        ],
        0,
      );

    const positionCompactEditor = () => {
      const bounds = getAnnotationBounds();
      if (!bounds) return;
      const editorRect = editor.getBoundingClientRect();
      const spacing = 8;
      const candidates = [
        { left: bounds.x + bounds.width + spacing, top: bounds.y },
        { left: bounds.x - editorRect.width - spacing, top: bounds.y },
        { left: bounds.x + bounds.width - editorRect.width, top: bounds.y + bounds.height + spacing },
        { left: bounds.x + bounds.width - editorRect.width, top: bounds.y - editorRect.height - spacing },
      ];
      const overflow = (position) =>
        Math.max(0, -position.left) +
        Math.max(0, -position.top) +
        Math.max(0, position.left + editorRect.width - window.innerWidth) +
        Math.max(0, position.top + editorRect.height - window.innerHeight);
      const best = candidates.reduce((current, candidate) => (overflow(candidate) < overflow(current) ? candidate : current));
      applyEditorPosition(best);
    };

    function queueEditorLayout() {
      if (editorLayoutFrame !== null) window.cancelAnimationFrame(editorLayoutFrame);
      editorLayoutFrame = window.requestAnimationFrame(() => {
        editorLayoutFrame = null;
        if (editor.style.display === "none") return;
        if (editorExpanded && editorPosition) applyEditorPosition(editorPosition);
        else positionCompactEditor();
      });
    }

    adjust.addEventListener("click", () => {
      if (selected.size === 0) return;
      if (!editorExpanded) {
        const rect = editor.getBoundingClientRect();
        editorExpanded = true;
        editorPosition = { left: rect.left, top: rect.top };
        stylePanel.style.display = selected.size > 0 ? "grid" : "none";
        dragHandle.style.display = "block";
        adjust.setAttribute("aria-expanded", "true");
        adjust.title = "Collapse annotation editor";
        adjust.setAttribute("aria-label", "Collapse annotation editor");
        if (selected.size > 0) syncStyleControls();
      } else {
        editorExpanded = false;
        editorPosition = null;
        stylePanel.style.display = "none";
        dragHandle.style.display = "none";
        adjust.setAttribute("aria-expanded", "false");
        adjust.title = "Expand annotation editor";
        adjust.setAttribute("aria-label", "Expand annotation editor");
      }
      queueEditorLayout();
    });

    const onEditorPointerDown = (event) => {
      if (event.button !== 0 || !editorExpanded) return;
      const rect = editor.getBoundingClientRect();
      editorDrag = { pointerId: event.pointerId, offsetX: event.clientX - rect.left, offsetY: event.clientY - rect.top };
      dragHandle.setPointerCapture(event.pointerId);
      dragHandle.style.cursor = "grabbing";
      event.preventDefault();
      event.stopPropagation();
    };
    const onEditorPointerMove = (event) => {
      if (!editorDrag || editorDrag.pointerId !== event.pointerId) return;
      applyEditorPosition({ left: event.clientX - editorDrag.offsetX, top: event.clientY - editorDrag.offsetY });
      event.preventDefault();
      event.stopPropagation();
    };
    const onEditorPointerUp = (event) => {
      if (!editorDrag || editorDrag.pointerId !== event.pointerId) return;
      editorDrag = null;
      dragHandle.style.cursor = "grab";
      if (dragHandle.hasPointerCapture(event.pointerId)) dragHandle.releasePointerCapture(event.pointerId);
      event.preventDefault();
      event.stopPropagation();
    };
    dragHandle.addEventListener("pointerdown", onEditorPointerDown);
    dragHandle.addEventListener("pointermove", onEditorPointerMove);
    dragHandle.addEventListener("pointerup", onEditorPointerUp);
    dragHandle.addEventListener("pointercancel", onEditorPointerUp);

    const repaint = () => {
      for (const target of selected.values()) updateSelectedVisual(target);
      queueEditorLayout();
    };

    const removeTargetAtPoint = (x, y) => {
      for (const target of Array.from(selected.values()).reverse()) {
        const rect = target.element.getBoundingClientRect();
        if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
          removeSelected(target);
          return true;
        }
      }
      const regionIndex = regions.findIndex(
        (region) =>
          x >= region.rect.x && x <= region.rect.x + region.rect.width && y >= region.rect.y && y <= region.rect.y + region.rect.height,
      );
      if (regionIndex >= 0) {
        const [removed] = regions.splice(regionIndex, 1);
        const box = root.querySelector(`[data-region-id="${removed && removed.id}"]`);
        if (box) box.remove();
        updateStatus();
        return true;
      }
      const strokeIndex = strokes.findIndex(
        (stroke) =>
          x >= stroke.bounds.x && x <= stroke.bounds.x + stroke.bounds.width && y >= stroke.bounds.y && y <= stroke.bounds.y + stroke.bounds.height,
      );
      if (strokeIndex >= 0) {
        const [removed] = strokes.splice(strokeIndex, 1);
        const path = svg.querySelector(`[data-stroke-id="${removed && removed.id}"]`);
        if (path) path.remove();
        updateStatus();
        return true;
      }
      return false;
    };

    const selectElementsInRect = (rect) => {
      const candidates = Array.from(document.querySelectorAll("body *"))
        .filter((element) => !isAnnotationNode(element))
        .map((element) => ({ element, rect: element.getBoundingClientRect() }))
        .filter(({ rect: candidate }) => {
          if (candidate.width < 2 || candidate.height < 2) return false;
          return !(
            candidate.right < rect.x ||
            candidate.left > rect.x + rect.width ||
            candidate.bottom < rect.y ||
            candidate.top > rect.y + rect.height
          );
        })
        .filter(({ element, rect: candidate }) => {
          const centerX = candidate.left + candidate.width / 2;
          const centerY = candidate.top + candidate.height / 2;
          return (
            centerX >= rect.x &&
            centerX <= rect.x + rect.width &&
            centerY >= rect.y &&
            centerY <= rect.y + rect.height &&
            (element.children.length === 0 ||
              element instanceof HTMLButtonElement ||
              element instanceof HTMLAnchorElement ||
              element.getAttribute("role") === "button")
          );
        })
        .sort((left, right) => left.rect.width * left.rect.height - right.rect.width * right.rect.height)
        .slice(0, MAX_MARQUEE_ELEMENTS);
      for (const candidate of candidates) addSelected(candidate.element);
      return candidates.length;
    };

    const clearHoverOutline = () => {
      hoverOutline.style.display = "none";
    };

    const onPointerMove = (event) => {
      if (isAnnotationNode(event.target)) {
        clearHoverOutline();
        return;
      }
      if (tool === "select" && dragStart === null) {
        const target = pickFromPoint(event.clientX, event.clientY);
        if (target) positionBox(hoverOutline, rectFromDomRect(target.getBoundingClientRect()));
        else clearHoverOutline();
        return;
      }
      clearHoverOutline();
      if (tool === "marquee" && dragStart) {
        positionBox(marqueeBox, normalizeRect(dragStart.x, dragStart.y, event.clientX, event.clientY));
        return;
      }
      if (tool === "draw" && activeStroke) {
        activeStroke.target.points = [...activeStroke.target.points, { x: event.clientX, y: event.clientY }];
        activeStroke.target.bounds = strokeBounds(activeStroke.target.points, activeStroke.target.width);
        activeStroke.path.setAttribute("d", pathFromPoints(activeStroke.target.points));
      }
    };

    const onPointerDown = (event) => {
      if (event.button !== 0 || isAnnotationNode(event.target)) return;
      event.preventDefault();
      event.stopPropagation();
      if (tool === "select") {
        const target = pickFromPoint(event.clientX, event.clientY);
        if (target) toggleSelected(target, event.shiftKey);
        return;
      }
      if (tool === "erase") {
        removeTargetAtPoint(event.clientX, event.clientY);
        return;
      }
      dragStart = { x: event.clientX, y: event.clientY };
      if (tool === "draw") {
        const stroke = {
          id: nextId("stroke"),
          color: (annotationTheme && annotationTheme.primary) || "#2563eb",
          width: 4,
          points: [dragStart],
          bounds: { x: dragStart.x, y: dragStart.y, width: 1, height: 1 },
        };
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute(OVERLAY_ATTRIBUTE, "");
        path.setAttribute("data-stroke-id", stroke.id);
        path.setAttribute("fill", "none");
        path.setAttribute("stroke", stroke.color);
        path.setAttribute("stroke-width", String(stroke.width));
        path.setAttribute("stroke-linecap", "round");
        path.setAttribute("stroke-linejoin", "round");
        svg.appendChild(path);
        activeStroke = { target: stroke, path };
      }
    };

    const onPointerUp = (event) => {
      if (!dragStart) return;
      event.preventDefault();
      event.stopPropagation();
      if (tool === "marquee") {
        const rect = normalizeRect(dragStart.x, dragStart.y, event.clientX, event.clientY);
        marqueeBox.style.display = "none";
        if (isUsableRect(rect)) {
          const found = selectElementsInRect(rect);
          if (found === 0) {
            const region = { id: nextId("region"), rect };
            regions.push(region);
            const regionBox = createBox(PRIMARY, "color-mix(in srgb, var(--t3-primary) 6%, transparent)");
            regionBox.setAttribute("data-region-id", region.id);
            positionBox(regionBox, rect);
            root.appendChild(regionBox);
          }
        }
      } else if (tool === "draw" && activeStroke) {
        if (activeStroke.target.points.length > 1) strokes.push(activeStroke.target);
        else activeStroke.path.remove();
        activeStroke = null;
      }
      dragStart = null;
      updateStatus();
    };

    const onClick = (event) => {
      if (isAnnotationNode(event.target)) return;
      event.preventDefault();
      event.stopPropagation();
    };
    const onPointerOut = (event) => {
      if (event.relatedTarget === null) clearHoverOutline();
    };
    const onWindowBlur = () => clearHoverOutline();

    const restoreStyles = () => {
      for (const target of selected.values()) restoreBaselines(target);
    };

    const teardown = (notify) => {
      if (finished) return;
      finished = true;
      restoreStyles();
      window.removeEventListener("pointermove", onPointerMove, true);
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("pointerup", onPointerUp, true);
      window.removeEventListener("pointerout", onPointerOut, true);
      window.removeEventListener("click", onClick, true);
      window.removeEventListener("blur", onWindowBlur);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("scroll", repaint, true);
      window.removeEventListener("resize", repaint);
      dragHandle.removeEventListener("pointerdown", onEditorPointerDown);
      dragHandle.removeEventListener("pointermove", onEditorPointerMove);
      dragHandle.removeEventListener("pointerup", onEditorPointerUp);
      dragHandle.removeEventListener("pointercancel", onEditorPointerUp);
      if (editorLayoutFrame !== null) window.cancelAnimationFrame(editorLayoutFrame);
      document.documentElement.removeAttribute("data-t3code-annotation-tool");
      // A pick data code never read (a cancel during capture) leaves no mark on the page.
      for (const target of selected.values()) {
        if (target.element.getAttribute(PICK_ATTRIBUTE) === target.id) target.element.removeAttribute(PICK_ATTRIBUTE);
      }
      cursorStyle.remove();
      inputLayer?.remove();
      host.remove();
      activeSession = null;
      if (notify) post({ type: "cancelled" });
    };

    const onKeyDown = (event) => {
      if (isAnnotationNode(event.target) && event.key !== "Escape") return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        teardown(true);
        return;
      }
      if (event.key === "v") tool = "select";
      else if (event.key === "r") tool = "marquee";
      else if (event.key === "d") tool = "draw";
      else if (event.key === "e") tool = "erase";
      else return;
      refreshToolButtons();
    };

    const submitAnnotation = (submission) => {
      if (pendingCapture || (selected.size === 0 && regions.length === 0 && strokes.length === 0)) return;
      pendingCapture = true;
      submit.disabled = true;
      submit.textContent = "Capturing…";
      // Snapshot everything the annotation will carry before the capture runs.
      const submittedComment = comment.value.trim();
      const submittedRegions = regions.map((region) => ({ id: region.id, rect: { ...region.rect } }));
      const submittedStrokes = strokes.map((stroke) => ({
        id: stroke.id,
        color: stroke.color,
        width: stroke.width,
        points: stroke.points.map((point) => ({ x: point.x, y: point.y })),
        bounds: { ...stroke.bounds },
      }));
      const submittedStyleChanges = Array.from(styleChanges.values(), (change) => ({ ...change }));
      try {
        const elements = Array.from(selected.values(), (target) => {
          const element = captureElement(target.element);
          for (const change of submittedStyleChanges) {
            if (change.targetId === target.id && element.selector !== null) change.selector = element.selector;
          }
          // Data code reads React's component for this mark in the page's world, then removes it.
          target.element.setAttribute(PICK_ATTRIBUTE, target.id);
          return { id: target.id, element, rect: rectFromDomRect(target.element.getBoundingClientRect()) };
        });
        const annotation = {
          id: nextId("annotation"),
          pageUrl: location.href,
          pageTitle: (document.title || "").trim() || null,
          comment: submittedComment,
          elements,
          regions: submittedRegions,
          strokes: submittedStrokes,
          styleChanges: submittedStyleChanges,
          screenshot: null,
          createdAt: new Date().toISOString(),
        };
        editor.style.display = "none";
        toolbar.style.display = "none";
        hoverOutline.style.display = "none";
        const screenshotRect = unionRects(
          [
            ...elements.map((target) => target.rect),
            ...submittedRegions.map((region) => region.rect),
            ...submittedStrokes.map((stroke) => stroke.bounds),
          ],
          20,
        );
        post({ type: "picked", annotation, rect: screenshotRect, submission });
      } catch (_) {
        // Last resort: data code is waiting, so hand it an empty pick rather than a stuck "Capturing…".
        teardown(true);
      }
    };
    submit.addEventListener("click", () => submitAnnotation("attach"));
    root.addEventListener("keydown", (event) => {
      const submission = event.target === comment ? resolveAnnotationSubmission(event) : null;
      // Bubble phase: editor inputs get the key first, then it is kept from the inspected page's listeners.
      event.stopImmediatePropagation();
      if (!submission) return;
      event.preventDefault();
      submitAnnotation(submission);
    });

    window.addEventListener("pointermove", onPointerMove, { capture: true, passive: false });
    window.addEventListener("pointerdown", onPointerDown, { capture: true, passive: false });
    window.addEventListener("pointerup", onPointerUp, { capture: true, passive: false });
    window.addEventListener("pointerout", onPointerOut, { capture: true, passive: true });
    window.addEventListener("click", onClick, { capture: true, passive: false });
    window.addEventListener("blur", onWindowBlur);
    window.addEventListener("keydown", onKeyDown, { capture: true });
    window.addEventListener("scroll", repaint, { capture: true, passive: true });
    window.addEventListener("resize", repaint, { passive: true });
    if (inputLayer) {
      document.documentElement.appendChild(inputLayer);
      inputLayer.focus({ preventScroll: true });
    }
    document.documentElement.appendChild(host);
    refreshToolButtons();
    updateStatus();

    activeSession = {
      teardown,
      applyTheme: (theme) => applyAnnotationTheme(host, theme),
      // What the tests read and drive through this world (the page never sees this object).
      state: () => ({
        tool,
        tools: Array.from(toolButtons.values(), (button) => ({ label: button.textContent, title: button.title })),
        selected: Array.from(selected.values(), (target) => describeRawElement(target.element)),
        regions: regions.length,
        strokes: strokes.length,
        styleChanges: Array.from(styleChanges.values(), (change) => ({ ...change })),
        editorShown: editor.style.display !== "none",
        toolbarShown: toolbar.style.display !== "none",
        expanded: editorExpanded,
        submitText: submit.textContent,
        submitTitle: submit.title,
        commentFocused: shadowRoot.activeElement === comment,
        direction: getComputedStyle(root).direction,
        commentDirection: getComputedStyle(comment).direction,
        placeholder: comment.placeholder,
      }),
      setComment: (text) => {
        comment.value = text;
        comment.dispatchEvent(new Event("input", { bubbles: true }));
        comment.focus({ preventScroll: true });
      },
      setField: (label, value) => {
        const control = fields.get(label);
        if (!control) return false;
        control.value = value;
        control.dispatchEvent(new Event("input", { bubbles: true }));
        control.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      },
      press: (name) => {
        const button = name === "adjust" ? adjust : name === "submit" ? submit : toolButtons.get(name);
        if (!button) return false;
        button.click();
        return true;
      },
    };
  }

  globalThis.__t3codeAnnotate = {
    start(theme) {
      annotationTheme = readTheme(theme);
      outbox.length = 0;
      startAnnotation();
      return true;
    },
    cancel() {
      if (activeSession) activeSession.teardown(false);
      post({ type: "cancelled" });
    },
    captured() {
      if (activeSession) activeSession.teardown(false);
    },
    applyTheme(theme) {
      annotationTheme = readTheme(theme);
      if (activeSession) activeSession.applyTheme(annotationTheme);
    },
    state() {
      return activeSession ? JSON.stringify(activeSession.state()) : "null";
    },
    next() {
      if (outbox.length) return Promise.resolve(outbox.shift());
      return new Promise((resolve) => {
        waiting = resolve;
      });
    },
    test: {
      setComment: (text) => (activeSession ? (activeSession.setComment(text), true) : false),
      setField: (label, value) => (activeSession ? activeSession.setField(label, value) : false),
      press: (name) => (activeSession ? activeSession.press(name) : false),
    },
  };
})();
