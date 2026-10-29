// The viewer page's sources (file-preview-host/src) under happy-dom: one window for every test file that imports this
// (bun test shares a process's globals and modules between files), its packages from the generator's node_modules as
// gen-file-preview.mjs resolves them, and the observers and workers each test answers itself.
import { afterAll, afterEach, beforeAll, mock } from 'bun:test';
import { plugin } from 'bun';
import { join } from 'node:path';

const generate = join(import.meta.dir, '..', '..', 'tools', 'generate');
/** The page's sources. */
export const page = join(import.meta.dir, '..', '..', 'file-preview-host', 'src');

// The icons are imported by file, which lucide's `exports` do not name.
plugin({
  name: 'viewer-page-icons',
  setup(build) {
    build.onResolve({ filter: /^lucide\// }, args => ({ path: join(generate, 'node_modules', args.path) }));
  },
});
// The packages the page's main thread imports (its workers run as FakeWorker): a runtime plugin does not reach a
// scoped specifier here, so each is the generator's copy by name.
for (const name of ['@betteroffice/xlsx', '@betteroffice/xlsx/headless', '@betteroffice/pptx', '@betteroffice/docx/layout', '@betteroffice/docx/layout/render',
  '@betteroffice/docx/yrs', 'effect/Schema', 'effect/Option']) {
  const resolved = Bun.resolveSync(name, `${generate}/`);
  mock.module(name, () => require(resolved));
}

const { Window } = await import(Bun.resolveSync('happy-dom', `${generate}/`)) as { Window: new (options: object) => Window & typeof globalThis };
const scope = globalThis as unknown as Record<string, unknown>;
/** One window for every file, so what a page module keeps (its tooltip card) stays in a document that is there. */
export const domWindow = (scope.__viewerWindow ??= new Window({ url: 'http://localhost/', width: 1280, height: 800 })) as Window & typeof globalThis;
const window = domWindow;
/** The window's globals Bun has none of, bound to it: installed for a DOM test file, and taken away after it, as another
 *  file's page (the terminal host's entry.ts) boots when it finds a document. */
const globals: Record<string, unknown> = { window, document: window.document };
for (const key of Object.getOwnPropertyNames(window)) {
  if (key in scope || key in globals) continue;
  const value = (window as unknown as Record<string, unknown>)[key];
  try { globals[key] = typeof value === 'function' && !/^[A-Z]/.test(key) ? (value as (...args: unknown[]) => unknown).bind(window) : value; } catch { /* a getter the window refuses */ }
}

// DOMPurify binds to the window it is made for, and a page module imports it before a test file's window is installed.
{
  const resolved = Bun.resolveSync('dompurify', `${generate}/`);
  mock.module('dompurify', () => {
    const made = require(resolved) as { default?: (window: unknown) => unknown } & ((window: unknown) => unknown);
    return { default: (made.default ?? made)(window) };
  });
}

/** An IntersectionObserver whose entries the test sends. */
export class FakeIntersection {
  static all: FakeIntersection[] = [];
  targets: Element[] = [];
  constructor(readonly callback: (entries: unknown[]) => void, readonly options?: unknown) { FakeIntersection.all.push(this); }
  observe(target: Element) { this.targets.push(target); }
  unobserve(target: Element) { this.targets = this.targets.filter(node => node !== target); }
  disconnect() { this.targets = []; }
  takeRecords() { return []; }
}
/** A ResizeObserver whose sizes the test sends. */
export class FakeResize {
  static all: FakeResize[] = [];
  targets: Element[] = [];
  constructor(readonly callback: (entries: unknown[]) => void) { FakeResize.all.push(this); }
  observe(target: Element) { this.targets.push(target); }
  unobserve(target: Element) { this.targets = this.targets.filter(node => node !== target); }
  disconnect() { this.targets = []; }
}
/** A worker the test answers: `answer` gets each message the page posts, by the worker's file name. */
export class FakeWorker extends EventTarget {
  static answers: Record<string, (message: any, worker: FakeWorker) => void> = {};
  static all: FakeWorker[] = [];
  readonly url: string;
  readonly posted: any[] = [];
  terminated = false;
  constructor(url: URL | string) { super(); this.url = String(url); FakeWorker.all.push(this); }
  get name() { return this.url.split('/').pop()!; }
  postMessage(message: any) {
    this.posted.push(message);
    const answer = FakeWorker.answers[this.name];
    if (answer) setTimeout(() => answer(message, this), 0);
  }
  reply(data: unknown) { if (!this.terminated) this.dispatchEvent(new MessageEvent('message', { data })); }
  terminate() { this.terminated = true; }
}

/** Every observed node intersecting (or not). */
export function intersectAll(isIntersecting = true) {
  for (const observer of FakeIntersection.all) for (const target of observer.targets) observer.callback([{ isIntersecting, target, intersectionRatio: isIntersecting ? 1 : 0 }]);
}
/** Every observed node at this size. */
export function resizeAll(width: number, height = 600) {
  for (const observer of FakeResize.all) for (const target of observer.targets) observer.callback([{ target, contentRect: { width, height } }]);
}
/** A node's laid-out size, which happy-dom does not compute. */
export function sized(node: Element, width: number, height: number) {
  Object.defineProperty(node, 'clientWidth', { configurable: true, get: () => width });
  Object.defineProperty(node, 'clientHeight', { configurable: true, get: () => height });
  Object.defineProperty(node, 'offsetWidth', { configurable: true, get: () => width });
  Object.defineProperty(node, 'offsetHeight', { configurable: true, get: () => height });
}
export const tick = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));
/**
 * Waits until `ready` holds, reading it every 5 ms (state in this process, cheap to read; docs/testing.md, Waiting),
 * and fails naming `what` after `bound` ms.
 */
export async function until(ready: () => unknown, what: string, bound = 3000): Promise<void> {
  const start = Date.now();
  while (!ready()) {
    if (Date.now() - start > bound) throw new Error(`waited ${bound} ms for ${what}`);
    await tick(5);
  }
}

/** What the page's `fetch` answers, by URL; each test sets its own. */
export const network = { respond: (_url: string, _init?: RequestInit): Response | Promise<Response> => new Response('') };

/** The page's globals for one test file: the window's, these observers and workers, and `fetch` answered by `network`. */
export function viewerDom() {
  const saved = new Map<string, { had: boolean; value: unknown }>();
  const replace = {
    ...globals, IntersectionObserver: FakeIntersection, ResizeObserver: FakeResize, Worker: FakeWorker,
    fetch: async (input: string | URL | Request, init?: RequestInit) => network.respond(input instanceof Request ? input.url : String(input), init),
  };
  beforeAll(() => {
    for (const [key, value] of Object.entries(replace)) { saved.set(key, { had: key in scope, value: scope[key] }); scope[key] = value; }
  });
  afterEach(() => {
    document.body.replaceChildren();
    FakeIntersection.all = []; FakeResize.all = []; FakeWorker.all = []; FakeWorker.answers = {};
    network.respond = () => new Response('');
  });
  afterAll(() => {
    for (const [key, { had, value }] of saved) { if (had) scope[key] = value; else delete scope[key]; }
    saved.clear();
  });
}
