// The page a GPU surface shows when this browser cannot draw it (LLP 1139.003
// D1): the host's `no-adapter` refusal, which used to leave the canvas empty
// behind its HUD. It sits in the canvas's box and nowhere else, as a
// `role=status` the driver's `tree` names; the canvas's own HUD is inert under
// it, so a control that leads nowhere cannot be pressed, and the app's title,
// menus and other canvases keep working around it. Loaded with gpu-glue.js.

/** What the page says: the requirement and the browsers that meet it. */
export const REFUSAL_TEXT = "This game needs WebGPU, which this browser does not have.";
export const BROWSERS_TEXT = "It runs in Chrome 121 or later on Android 12 or later, Safari 26 or later, and Chrome or Edge 113 or later on a computer.";

/** Show the page in `entry`'s canvas box: `host` is the node the canvas and its HUD children are in. */
export function showRefusal(entry, platform = globalThis) {
  const host = entry.host, doc = platform.document;
  if (!host || entry.refusalPage || !doc?.createElement) return;
  const page = doc.createElement("div");
  page.setAttribute("role", "status");
  page.dataset.gpuRefusal = "";
  Object.assign(page.style, { position: "absolute", inset: "0", zIndex: "2147483647", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: "12px", padding: "24px", boxSizing: "border-box", textAlign: "center", font: "16px/1.4 system-ui, sans-serif", overflow: "auto" });
  // The app's own colours where it set them (the text's inherited; the nearest painted background), else the platform's.
  let painted = host;
  while (painted && /^(transparent|rgba\(0, 0, 0, 0\))$/.test(platform.getComputedStyle?.(painted).backgroundColor ?? "transparent")) painted = painted.parentElement;
  page.style.background = painted ? platform.getComputedStyle(painted).backgroundColor : "Canvas";
  if (!painted) page.style.color = "CanvasText";
  const line = (text, weight) => { const p = doc.createElement("p"); p.textContent = text; Object.assign(p.style, { margin: "0", maxWidth: "36em", fontWeight: weight ?? "" }); page.append(p); return p; };
  line(REFUSAL_TEXT, "600"); line(BROWSERS_TEXT);
  // The canvas's own HUD (its other children, and any it gains) is inert while the page stands.
  const quiet = () => { for (const child of host.children ?? []) if (child !== entry.el && child !== page && !child.inert) child.inert = true; };
  quiet(); if (platform.MutationObserver) new platform.MutationObserver(quiet).observe(host, { childList: true });
  if (platform.getComputedStyle?.(host).position === "static") host.style.position = "relative";
  host.append(page);
  entry.refusalPage = { text: `${REFUSAL_TEXT} ${BROWSERS_TEXT}` };
  nativeBuilds(page, doc, platform).then(extra => { if (extra) entry.refusalPage.text += ` ${extra}`; }, () => {});
}

/** The app's native builds, when its install data (`/.exact/install.json`, scripts/install-page.mjs) lists any. */
async function nativeBuilds(page, doc, platform) {
  if (typeof platform.fetch !== "function") return "";
  const response = await platform.fetch(new URL("/.exact/install.json", doc.baseURI));
  if (!response.ok) return "";
  const data = await response.json();
  const methods = ["ios", "macos"].flatMap(p => (data?.platforms?.[p]?.methods ?? []).filter(m => typeof m.url === "string" && /^(https:|itms-services:)/.test(m.url)));
  if (!methods.length) return "";
  const row = doc.createElement("p"); Object.assign(row.style, { margin: "0", display: "flex", gap: "16px", flexWrap: "wrap", justifyContent: "center" });
  for (const m of methods) { const a = doc.createElement("a"); a.href = m.url; a.textContent = m.label ?? m.url; row.append(a); }
  page.append(row);
  return methods.map(m => m.label ?? m.url).join(", ");
}
