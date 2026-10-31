// Annotate over a Browser tab's frame (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/desktop/src/preview/
// Manager.ts `pickElement`, `cancelPickElement`, `captureAnnotationScreenshot`; at 74541bab17 `normalizeCaptureRect` and
// `cropAnnotationScreenshot`; PickPreload.ts, here assets/browser-annotate.js; react-grab's `getElementContext`).
//
// - The overlay runs in a sandbox world of the frame's page (`@exact/browser-automation`'s `callFunction` with a
//   `sandbox`): it shares the page's DOM, not its JavaScript, so the page can neither see nor call it. Electron runs the
//   reference's preload in the page's world so react-grab can read React; here the overlay marks each picked element
//   `data-t3code-pick`, and one `evaluate` in the page's world reads React's fiber on those elements (`__reactFiber$…`:
//   the nearest component's name, its `_debugSource`, its `_debugOwner` chain), then removes the marks. Production React
//   keeps the fiber but not the debug fields, and minifies names, as in the reference.
// - The pick's outcome arrives as one `evaluate` of the overlay's `next()`, which resolves when the person picks or
//   cancels; it runs in one of the window's queued sends (`queueSlotWork`), since the page posts nothing to the app. A new
//   document ends that call, and the pick with it, as Manager's `onNavigated` does.
// - The crop is `captureScreenshot` of the pick's rect, clipped to the viewport, written straight to the composer's draft
//   images (app:/data/snapshots/drafts/<uuid>.png) within five seconds; a timeout, a failure or an image over 10 MB keeps
//   the annotation without it (`screenshotFailed`). So does a pick whose targets are all scrolled out of view.
// - Exactly one settlement per pick (Manager `claimSettle`): a result, Escape in the page, a cancel or a newer pick.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';
import { sleep } from '../client/command-waits';
import { appFiles } from '../media/save-file';
import { automationCalls } from './browser-automation-run';
import { ANNOTATE_OVERLAY, ANNOTATE_STYLES } from './page-scripts';
import { frameEntry } from './browser-frames';
import { queueSlotWork } from './browser-automation-recording';

const SANDBOX = 't3-annotate';
const DRAFTS = 'app:/data/snapshots/drafts';
export const SCREENSHOT_TIMEOUT_MS = 5000;
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const MAX_FIELD_LENGTH = 500;
const MAX_STACK_FRAMES = 8;

type Pick = { pick: number; active: boolean; serial: number };
const all = new WeakMap<object, Map<string, Pick>>();
function pickOf(owner: object, runtimeId: string): Pick {
  let map = all.get(owner);
  if (!map) { map = new Map(); all.set(owner, map); }
  let pick = map.get(runtimeId);
  if (!pick) { pick = { pick: 0, active: false, serial: 0 }; map.set(runtimeId, pick); }
  return pick;
}
/** Whether a tab's pick is on, and how many picks of it settled. */
export function annotateState(owner: object, runtimeId: string): { active: boolean; serial: number } {
  const pick = all.get(owner)?.get(runtimeId);
  return { active: pick?.active === true, serial: pick?.serial ?? 0 };
}
/** Every tab with a pick on (the panel cancels one whose tab it stops showing). */
export const activePicks = (owner: object): string[] => [...(all.get(owner)?.entries() ?? [])].filter(([, pick]) => pick.active).map(([runtimeId]) => runtimeId);

/** What the window does with a settled pick (browser-capture.ts: the composer's chip and crop, a send), made when the
 *  person's pick arrives, so it targets the composer the window showed then, not one shown after the crop. */
export type ApplyPick = (result: Obj, native: Native) => Promise<string>;
export type PrepareApply = () => ApplyPick;
const settle = (pick: Pick, id: number): boolean => {
  if (pick.pick !== id || !pick.active) return false;
  pick.active = false; pick.serial++;
  return true;
};

/** PreviewManager.pickElement: a new pick replaces the one in flight (cancelled first). */
export async function startAnnotate(client: T3Client, runtimeId: string, theme: Record<string, string>, prepare: PrepareApply): Promise<void> {
  const entry = frameEntry(client, runtimeId);
  if (!entry) return;
  const pick = pickOf(client, runtimeId);
  if (pick.active) await cancelAnnotate(client, runtimeId);
  const id = ++pick.pick, frame = entry.frameId;
  pick.active = true;
  try {
    // The overlay installs itself once per document (it keeps `globalThis.__t3codeAnnotate`), then starts.
    const started = await automationCalls().callFunction({ id: frame, sandbox: SANDBOX, arguments: [theme],
      functionDeclaration: `function (theme) {\nif (!globalThis.__t3codeAnnotate) {\nconst STYLES = ${JSON.stringify(ANNOTATE_STYLES)};\n${ANNOTATE_OVERLAY}\n}\nreturn globalThis.__t3codeAnnotate.start(theme) === true;\n}` });
    if (obj(started).result !== true) { settle(pick, id); return; }
  } catch (error) { if (letGo(error)) throw error; settle(pick, id); return; }
  queueSlotWork(client, `annotate:${frame}:${id}`, native => awaitPick(frame, pick, id, prepare, native));
}

/** PreviewManager.cancelPickElement: the overlay comes down and the pick settles with nothing. */
export async function cancelAnnotate(client: T3Client, runtimeId: string): Promise<void> {
  const pick = pickOf(client, runtimeId), entry = frameEntry(client, runtimeId);
  if (!settle(pick, pick.pick) || !entry) return;
  await automationCalls().evaluate({ id: entry.frameId, sandbox: SANDBOX, expression: 'globalThis.__t3codeAnnotate ? (globalThis.__t3codeAnnotate.cancel(), true) : false' })
    .catch((error: unknown) => { if (letGo(error)) throw error; });
}

/** The pick's one outcome: the overlay's next message, then the React read, the crop, the overlay's teardown and the apply. */
async function awaitPick(frame: string, pick: Pick, id: number, prepare: PrepareApply, native: Native): Promise<void> {
  let message: Obj | null = null;
  try {
    const reply = await automationCalls().evaluate({ id: frame, sandbox: SANDBOX, awaitPromise: true, expression: 'globalThis.__t3codeAnnotate ? globalThis.__t3codeAnnotate.next() : null' });
    const text = obj(reply).result;
    message = typeof text === 'string' ? obj(JSON.parse(text)) : null;
  } catch (error) { if (letGo(error)) throw error; } // a new document, or the frame gone
  if (pick.pick !== id || !pick.active) return; // a cancel or a newer pick settled it
  const annotation = obj(message?.annotation);
  if (message?.type !== 'picked' || !Object.keys(annotation).length) { settle(pick, id); return; }
  const apply = prepare();
  annotation.screenshot = null; // the page never supplies the crop (isPreviewAnnotationPayload)
  const submission = message.submission === 'send' ? 'send' : 'attach';
  const enriched = await enrich(frame, annotation);
  const screenshot = await Promise.race([crop(frame, normalizeCaptureRect(message.rect), targetRects(enriched), str(enriched.id, 'annotation')), sleep(SCREENSHOT_TIMEOUT_MS).then(() => null)]);
  // claimSettle: a crop outliving its pick is not attached.
  if (!settle(pick, id)) return;
  // ANNOTATION_CAPTURED_CHANNEL: the overlay tears down once its marks are in the crop.
  await automationCalls().evaluate({ id: frame, sandbox: SANDBOX, expression: 'globalThis.__t3codeAnnotate ? (globalThis.__t3codeAnnotate.captured(), true) : false' }).catch(() => undefined);
  const result: Obj = { annotation: enriched, submission, screenshotFailed: screenshot === null, ...(screenshot ? { screenshot } : {}) };
  await apply(result, native);
}

// ── React (the page's world) ─────────────────────────────────────────────────────────────────────
/** One evaluation in the page's world: for each element the overlay marked, React's component name, the element's source
 *  and the component owner chain, by target id; then the marks are removed. */
export const REACT_LOOKUP = `(() => {
  const clip = (value) => typeof value === "string" ? value.slice(0, ${MAX_FIELD_LENGTH}) : null;
  const num = (value) => typeof value === "number" && Number.isFinite(value) ? value : null;
  const nameOf = (type) => {
    if (!type || (typeof type !== "function" && typeof type !== "object")) return null;
    const name = type.displayName || type.name || (type.render && (type.render.displayName || type.render.name)) || (type.type && (type.type.displayName || type.type.name));
    return typeof name === "string" && name ? name : null;
  };
  const isComponent = (fiber) => !!fiber && (typeof fiber.type === "function" || (!!fiber.type && typeof fiber.type === "object"));
  const frame = (fiber, source) => {
    const at = source || null;
    return { functionName: clip(nameOf(fiber && fiber.type)), fileName: clip(at && at.fileName), lineNumber: num(at && at.lineNumber), columnNumber: num(at && at.columnNumber) };
  };
  const out = {};
  for (const element of Array.from(document.querySelectorAll("[data-t3code-pick]"))) {
    const id = element.getAttribute("data-t3code-pick");
    element.removeAttribute("data-t3code-pick");
    const info = { componentName: null, source: null, stack: [] };
    try {
      const key = Object.keys(element).find((name) => name.startsWith("__reactFiber$") || name.startsWith("__reactInternalInstance$"));
      const fiber = key ? element[key] : null;
      if (fiber) {
        let component = fiber._debugOwner && isComponent(fiber._debugOwner) ? fiber._debugOwner : null;
        for (let node = fiber.return, steps = 0; !component && node && steps < 200; node = node.return, steps += 1) if (isComponent(node)) component = node;
        if (component) {
          info.componentName = clip(nameOf(component.type));
          for (let owner = component, steps = 0; owner && steps < ${MAX_STACK_FRAMES}; owner = owner._debugOwner, steps += 1) info.stack.push(frame(owner, owner._debugSource));
          const source = fiber._debugSource || component._debugSource;
          info.source = source ? { ...frame(component, source) } : (info.stack[0] || null);
        }
      }
    } catch (_) {}
    if (id) out[id] = info;
  }
  return JSON.stringify(out);
})()`;

async function enrich(frame: string, annotation: Obj): Promise<Obj> {
  if (!Array.isArray(annotation.elements) || !annotation.elements.length) return annotation;
  let found: Record<string, Obj> = {};
  try {
    const reply = await automationCalls().evaluate({ id: frame, expression: REACT_LOOKUP });
    const text = obj(reply).result;
    if (typeof text === 'string') found = obj(JSON.parse(text)) as Record<string, Obj>;
  } catch (error) { if (letGo(error)) throw error; }
  return mergeReact(annotation, found);
}

const clip = (value: unknown): string | null => typeof value === 'string' ? value.slice(0, MAX_FIELD_LENGTH) : null;
const finite = (value: unknown): number | null => typeof value === 'number' && Number.isFinite(value) ? value : null;
/** A stack frame of strings or nulls and finite numbers or nulls. */
export function stackFrame(value: unknown): Obj | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const frame = value as Obj;
  return { functionName: clip(frame.functionName), fileName: clip(frame.fileName), lineNumber: finite(frame.lineNumber), columnNumber: finite(frame.columnNumber) };
}
/** The page world's findings merged into `elements[i].element` by target id, each field checked and bounded. */
export function mergeReact(annotation: Obj, react: Record<string, Obj>): Obj {
  if (!Array.isArray(annotation.elements)) return annotation;
  return { ...annotation, elements: annotation.elements.map(raw => {
    const target = obj(raw), info = react[str(target.id)], element = target.element;
    if (!info || !element || typeof element !== 'object') return raw;
    const next: Obj = { ...(element as Obj) };
    if (typeof info.componentName === 'string' && info.componentName) next.componentName = info.componentName.slice(0, MAX_FIELD_LENGTH);
    const stack = (Array.isArray(info.stack) ? info.stack : []).slice(0, MAX_STACK_FRAMES).map(stackFrame).filter((frame): frame is Obj => frame !== null);
    next.stack = stack;
    next.source = stackFrame(info.source) ?? stack[0] ?? null;
    return { ...target, element: next };
  }) };
}

// ── The crop ─────────────────────────────────────────────────────────────────────────────────────
type Rect = { x: number; y: number; width: number; height: number };
const readRect = (value: unknown): Rect | null => {
  const rect = obj(value), x = finite(rect.x), y = finite(rect.y), width = finite(rect.width), height = finite(rect.height);
  return x === null || y === null || width === null || height === null ? null : { x, y, width, height };
};
/** Manager `normalizeCaptureRect`: finite numbers and a positive size, in CSS pixels as the page sent them. */
export function normalizeCaptureRect(value: unknown): Rect | null {
  const rect = readRect(value);
  return rect && rect.width > 0 && rect.height > 0 ? rect : null;
}
/** Manager's `targetRects`: each element's and region's rect and each stroke's bounds, in CSS pixels. */
export function targetRects(annotation: Obj): Rect[] {
  const rects = (key: string, field: string) => (Array.isArray(annotation[key]) ? annotation[key] as unknown[] : []).map(target => readRect(obj(target)[field])).filter((rect): rect is Rect => rect !== null);
  return [...rects('elements', 'rect'), ...rects('regions', 'rect'), ...rects('strokes', 'bounds')];
}
/** `cropAnnotationScreenshot`'s geometry, in the viewport's CSS pixels: the region to keep, the whole viewport without a
 *  rect, or null when it would hold none of the pick. At least one target must be in view; a zero-width or zero-height
 *  SVG line still paints at the top or left edge. A rect with no pixels in the viewport is refused rather than clamped
 *  onto its edge, which would attach an unrelated strip. */
export function cropRegion(rect: Rect | null, targets: Rect[], size: { width: number; height: number }): Rect | null {
  if (size.width <= 0 || size.height <= 0) return null;
  const visible = targets.some(target => (target.width > 0 || target.height > 0) && target.x < size.width && target.y < size.height
    && (target.width === 0 ? target.x >= 0 : target.x + target.width > 0) && (target.height === 0 ? target.y >= 0 : target.y + target.height > 0));
  if (targets.length && !visible) return null;
  if (!rect) return { x: 0, y: 0, width: size.width, height: size.height };
  const x = Math.max(0, Math.floor(rect.x)), y = Math.max(0, Math.floor(rect.y));
  const right = Math.min(size.width, Math.ceil(rect.x + rect.width)), bottom = Math.min(size.height, Math.ceil(rect.y + rect.height));
  return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
}
/** A v4 id for the draft image (the composer's drafts are named by one), from `crypto.getRandomValues`. */
function draftId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6]! & 0x0f) | 0x40; bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
/** `captureAnnotationScreenshot`: the rect (or the whole viewport) as a PNG draft image, or null. */
async function crop(frame: string, rect: Rect | null, targets: Rect[], annotationId: string): Promise<Obj | null> {
  try {
    const sized = await automationCalls().evaluate({ id: frame, sandbox: SANDBOX, expression: 'JSON.stringify([innerWidth, innerHeight])' });
    const [width, height] = JSON.parse(str(obj(sized).result, '[0,0]')) as [number, number];
    const region = cropRegion(rect, targets, { width, height });
    if (!region) return null;
    const id = draftId(), path = `${DRAFTS}/${id}.png`;
    await appFiles().fs.mkdir(DRAFTS, { recursive: true });
    const shot = obj(await automationCalls().captureScreenshot({ id: frame, path, clip: region }));
    const bytes = await appFiles().fs.readFile(path);
    if (!bytes.byteLength || bytes.byteLength > MAX_IMAGE_BYTES) return null;
    // The payload's cropRect is the page's rect (the reference's), or the whole view without one.
    return { id, name: `preview-annotation-${annotationId}.png`, sizeBytes: bytes.byteLength, mimeType: 'image/png', width: Number(shot.width) || 0, height: Number(shot.height) || 0, cropRect: rect ?? region };
  } catch (error) { if (letGo(error)) throw error; return null; }
}
