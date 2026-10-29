// The page's Content-Security-Policy, for its workers too. A worker started from its script's URL takes only that
// response's policy, and the app's scheme sends none for the page's files; one started from a `blob:` URL inherits the
// policy of the context that started it (HTML, "worker processing model": a local-scheme worker's policy container
// is its owner's). So each Worker starts from a module in a Blob that imports its script, and the narrowing the page
// adds (entry.ts `narrowConnections`) bounds what the workers fetch. Every context that starts workers imports this
// first: the page, and the Word worker, whose engine starts its own.
import { contextBase } from "./packed";

/** Worker, started from a Blob module that imports its script (or a classic Blob that imports it), its options kept. */
export function policyWorker(Native: typeof Worker, base: () => string): typeof Worker {
  const modules = new Map<string, string>();
  return class PolicyWorker extends Native {
    constructor(url: string | URL, options?: WorkerOptions) {
      let href: string;
      try { href = new URL(String(url), base()).href; } catch { href = String(url); }
      if (/^(?:blob|data):/i.test(href)) {
        super(url, options);
        return;
      }
      const module = options?.type === "module";
      const key = `${module ? "module" : "classic"} ${href}`;
      let wrapper = modules.get(key);
      if (!wrapper) {
        wrapper = URL.createObjectURL(new Blob([module ? `import ${JSON.stringify(href)};` : `importScripts(${JSON.stringify(href)});`], { type: "text/javascript" }));
        modules.set(key, wrapper);
      }
      super(wrapper, options);
    }
  } as typeof Worker;
}

// In the page and its workers; a test imports `policyWorker` alone.
if (typeof Worker !== "undefined" && (typeof document !== "undefined" || typeof WorkerGlobalScope !== "undefined")) {
  globalThis.Worker = policyWorker(Worker, contextBase);
}
