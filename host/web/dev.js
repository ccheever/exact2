// The dev connection owns complete, verified generations (LLP 1023 D3).
// A new epoch resets ordering; each commit replaces the entire asset roster.
const es = typeof EventSource === "undefined" ? null : new EventSource("/__dev");
const encoder = new TextEncoder();
let overlay = null;
const errors = new Map();
function show(message, source = "app") {
  if (message) errors.set(source, message); else errors.delete(source);
  const text = [...errors.values()].join("\n\n");
  if (!text) { overlay?.remove(); overlay = null; return; }
  overlay ??= document.body.appendChild(Object.assign(document.createElement("pre"), { style: "position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647" }));
  overlay.textContent = text + "\n(click to dismiss)";
  overlay.onclick = () => { errors.clear(); show(null); };
}

// SHA-256 also works on trusted-LAN HTTP pages where SubtleCrypto is absent.
// The constants are the fractional square/cube roots of the first primes.
// All integer additions below are reduced modulo 2^32.
let hashConstants;
async function digest(bytes) {
  if (globalThis.crypto?.subtle) return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (n) => n.toString(16).padStart(2, "0")).join("");
  if (!hashConstants) {
    const primes = [];
    for (let n = 2; primes.length < 64; n++) if (primes.every((p) => n % p)) primes.push(n);
    hashConstants = { initial: primes.slice(0, 8).map((n) => (Math.sqrt(n) % 1 * 0x100000000) | 0),
      round: primes.map((n) => (Math.cbrt(n) % 1 * 0x100000000) | 0) };
  }
  const hash = hashConstants.initial.slice(), words = new Int32Array(64);
  const padded = new Uint8Array(Math.ceil((bytes.length + 9) / 64) * 64);
  padded.set(bytes); padded[bytes.length] = 128;
  const view = new DataView(padded.buffer);
  view.setUint32(padded.length - 8, Math.floor(bytes.length / 0x20000000));
  view.setUint32(padded.length - 4, bytes.length * 8);
  const rotate = (n, by) => n >>> by | n << (32 - by);
  for (let start = 0; start < padded.length; start += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getInt32(start + i * 4);
    for (let i = 16; i < 64; i++) {
      const a = words[i - 15], b = words[i - 2];
      words[i] = words[i - 16] + (rotate(a, 7) ^ rotate(a, 18) ^ a >>> 3) + words[i - 7] + (rotate(b, 17) ^ rotate(b, 19) ^ b >>> 10);
    }
    let [a, b, c, d, e, f, g, h] = hash;
    for (let i = 0; i < 64; i++) {
      const t1 = (h + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + (e & f ^ ~e & g) + hashConstants.round[i] + words[i]) | 0;
      const t2 = ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + (a & b ^ a & c ^ b & c)) | 0;
      h = g; g = f; f = e; e = d + t1 | 0; d = c; c = b; b = a; a = t1 + t2 | 0;
    }
    [a, b, c, d, e, f, g, h].forEach((n, i) => { hash[i] = hash[i] + n | 0; });
  }
  return hash.map((n) => (n >>> 0).toString(16).padStart(8, "0")).join("");
}
const validIdentity = (m) => m && /^[0-9a-f]{32}$/.test(m.epoch) && Number.isSafeInteger(m.seq) && m.seq >= 0 && /^[0-9a-f]{64}$/.test(m.generation);
const utf8Compare = (a, b) => {
  const aa = encoder.encode(a), bb = encoder.encode(b);
  for (let i = 0; i < Math.min(aa.length, bb.length); i++) if (aa[i] !== bb[i]) return aa[i] - bb[i];
  return aa.length - bb.length;
};
const validName = (name) => typeof name === "string" && /^(assets|deck|shaders)\//.test(name)
  && !name.includes("\\") && !name.includes(":") && !name.includes("\0")
  && name.split("/").every((part) => part && part !== "." && part !== "..");
const sameOrigin = (path, base) => {
  const url = new URL(path, base);
  if (url.origin !== location.origin || !["http:", "https:"].includes(url.protocol)) throw new Error("the dev generation leaves the app origin");
  return url.href;
};
async function readBounded(url, limit, signal) {
  const response = await fetch(url, { cache: "no-store", signal });
  sameOrigin(response.url || url, location.href);
  if (!response.ok) throw new Error(`generation fetch HTTP ${response.status}: ${url}`);
  if (Number(response.headers.get("content-length")) > limit) throw new Error("generation response exceeds its declared size");
  const reader = response.body.getReader(), chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      length += value.length;
      if (length > limit) throw new Error("generation response exceeds its declared size");
      chunks.push(value);
    }
  } catch (error) { await reader.cancel(); throw error; }
  const bytes = new Uint8Array(length);
  let at = 0; for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return { bytes, url: response.url || url };
}
async function readEnvelope(url, signal) {
  const response = await readBounded(sameOrigin(url, location.href), 64 * 1024, signal);
  return { envelope: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(response.bytes)), url: response.url };
}
async function fetchGeneration(message, signal, first = false) {
  const started = performance.now();
  const { envelope, url } = await readEnvelope(message.envelope, signal);
  const identity = envelope.dev;
  if (envelope.exact !== 1 || !validIdentity(identity) || identity.epoch !== message.epoch
      || identity.seq !== message.seq || identity.generation !== message.generation || identity.program !== message.program || !Array.isArray(envelope.assets)) throw new Error("the fetched envelope does not name the announced generation");
  // The page's first generation is fetched inside glue's boot, compat known.
  if (!first) await globalThis.exact.ready;
  if (envelope.app?.id !== globalThis.exact.compat?.inputs?.app) throw new Error("the dev generation names another app");
  const assets = new Map(), names = new Set();
  const load = async (card, name) => {
    if (!card || !Number.isSafeInteger(card.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024 || !/^[0-9a-f]{64}$/.test(card.sha256)) throw new Error(`invalid payload card: ${name}`);
    const path = sameOrigin(card.url, url);
    const { bytes } = await readBounded(path, card.bytes, signal);
    if (bytes.length !== card.bytes || await digest(bytes) !== card.sha256) throw new Error(`payload integrity failed: ${name}`);
    return { bytes, url: path, sha256: card.sha256 };
  };
  for (const card of envelope.assets) {
    if (!validName(card.name) || names.has(card.name)) throw new Error("invalid or duplicate asset name");
    names.add(card.name);
  }
  const moduleKeys = ['native', 'receipt', 'web'];
  if (envelope.module !== undefined && (!envelope.module || Object.keys(envelope.module).sort().join(',') !== moduleKeys.join(','))) throw new Error('invalid module manifest');
  const moduleCards = envelope.module ? moduleKeys.map(key => envelope.module[key]) : [];
  const rust = envelope.rust;
  if (rust && (!rust.wasm || Object.keys(rust).some(k => !["wasm", "native", "tiered"].includes(k)))) throw new Error("invalid Rust manifest");
  const rustCards = rust ? Object.values(rust).flatMap(v => [v.receipt, v.module]) : [];
  const cards = [envelope.plan, ...envelope.assets, ...moduleCards, ...rustCards];
  if (cards.some((card) => !Number.isSafeInteger(card?.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024)
      || cards.reduce((sum, card) => sum + card.bytes, 0) > 256 * 1024 * 1024) throw new Error("generation exceeds the payload budget");
  // All admitted payloads are independent. One bounded queue avoids separate
  // network rounds for the plan, assets and module without raising concurrency.
  const jobs = [[envelope.plan, "app.plan"], ...envelope.assets.map(card => [card, card.name]),
    ...(envelope.module ? ['receipt', 'web'].map(key => [envelope.module[key], `module ${key}`]) : []),
    ...(rust ? [[rust.wasm.receipt, 'Rust receipt'], [rust.wasm.module, 'Rust Wasm']] : [])];
  const loaded = new Array(jobs.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, jobs.length) }, async () => {
    while (next < jobs.length) {
      const index = next++;
      loaded[index] = await load(...jobs[index]);
    }
  }));
  const plan = loaded[0];
  envelope.assets.forEach((card, index) => assets.set(card.name, loaded[index + 1]));
  const moduleAt = 1 + envelope.assets.length;
  const module = envelope.module ? { receipt: loaded[moduleAt].bytes, script: loaded[moduleAt + 1].bytes } : null;
  const rustPayload = rust ? { receipt: loaded.at(-2).bytes, module: loaded.at(-1).bytes } : null;
  const canonical = { assets: [...envelope.assets].sort((a, b) => utf8Compare(a.name, b.name)).map((a) => ({ bytes: a.bytes, name: a.name, sha256: a.sha256 })),
    ...(envelope.module ? { module: Object.fromEntries(moduleKeys.map(key => [key, { bytes: envelope.module[key].bytes, sha256: envelope.module[key].sha256 }])) } : {}),
    plan: { bytes: plan.bytes.length, sha256: plan.sha256 },
    ...(rust ? { rust: Object.fromEntries(Object.keys(rust).sort().map(k => [k, { module: {bytes:rust[k].module.bytes,sha256:rust[k].module.sha256}, receipt:{bytes:rust[k].receipt.bytes,sha256:rust[k].receipt.sha256}, ...(k !== "wasm" ? {target:rust[k].target} : {}) }])) } : {}) };
  if (await digest(encoder.encode(JSON.stringify(canonical))) !== identity.generation) throw new Error("the generation digest does not bind its complete manifest");
  return { ...identity, plan: plan.bytes, assets, module, rust: rustPayload, fetchMs: performance.now() - started };
}

// Fetches may finish in any order. Only the latest request can enter the
// host's acceptance/commit operation; a failed candidate keeps the live one.
function generationClient({ fetchGeneration, apply, applied = () => {}, failed = () => {}, programChanged = () => {} }) {
  let epoch = null, queued = -1, committed = null, attempt = 0, inFlight = null, controller = null;
  const retired = new Set();
  let program = null, stopped = false;
  const stop = () => { stopped = true; attempt++; controller?.abort(); inFlight = null; programChanged(); };
  return {
    // The page booted this generation itself (its first boot): never again.
    adopt(generation) { committed = generation; },
    receive(message) {
      if (stopped) return Promise.resolve(false);
      if (message?.rebuilt) { stop(); return Promise.resolve(false); }
      if (!validIdentity(message) || retired.has(message.epoch)) return Promise.resolve(false);
      if (message.program !== undefined && !/^[0-9a-f]{64}$/.test(message.program)) return Promise.resolve(false);
      if (program && program !== message.program) { stop(); return Promise.resolve(false); }
      if (message.program) program = message.program;
      if (epoch !== message.epoch) { if (epoch !== null) retired.add(epoch); epoch = message.epoch; queued = -1; attempt++; }
      if (message.seq < queued || message.seq === queued && inFlight) return Promise.resolve(false);
      queued = message.seq;
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      const request = ++attempt, current = () => request === attempt;
      if (committed === message.generation) { inFlight = null; return Promise.resolve(false); }
      const activeController = controller;
      const timeout = setTimeout(() => activeController.abort(), 15000);
      const pending = (async () => {
        try {
          const candidate = await fetchGeneration(message, signal);
          if (!current() || committed === candidate.generation) return false;
          let accepted;
          try { accepted = await apply(candidate, current); }
          catch (error) { throw Object.assign(new Error(String(error)), { hostRefused: true }); }
          if (!current()) return false;
          if (!accepted) throw Object.assign(new Error("the host refused the current generation"), { hostRefused: true });
          committed = candidate.generation;
          applied(candidate);
          return true;
        } catch (error) { if (current()) { failed(error, current); } return false; }
        finally { clearTimeout(timeout); if (current()) inFlight = null; }
      })();
      inFlight = pending;
      return pending;
    },
  };
}

if (!es) globalThis.exactDevProtocol = { digest, generationClient, validIdentity };
if (es) {
  globalThis.exact.devError = show;
  // Host affordance, outside the app tree and absent from static/production pages.
  // Agent input targets the app tree; this host-only link covers app controls.
  // It covers whatever the app puts in that corner (a phone layout's tab bar),
  // so it can be put away; the choice is kept for this origin.
  const hidden = 'exact.dev.open.hidden';
  if (!new URLSearchParams(location.search).has('agent') && localStorage.getItem(hidden) !== '1') {
    const bar = document.body.appendChild(document.createElement('div'));
    bar.style = 'position:fixed;right:10px;bottom:10px;display:flex;align-items:center;background:#fff;border:1px solid #ccd3df;border-radius:8px;font:13px system-ui;z-index:2147483646';
    const opening = bar.appendChild(document.createElement('a'));
    opening.href = '/__dev/open' + location.search + location.hash;
    opening.textContent = 'Open in native…';
    opening.style = 'padding:8px 4px 8px 12px;color:#164bc4';
    const close = bar.appendChild(document.createElement('button'));
    close.textContent = '×';
    close.title = 'Hide (clear site data to bring it back)';
    close.setAttribute('aria-label', 'Hide the Open in native link');
    close.style = 'border:0;background:none;padding:8px 10px;font:16px system-ui;color:#5b6475;cursor:pointer';
    close.onclick = () => { localStorage.setItem(hidden, '1'); bar.remove(); };
  }
  // The module loader uses this implementation on trusted LAN HTTP too.
  globalThis.exact.moduleDigest = digest;
  let retry = null;
  const client = generationClient({ fetchGeneration, programChanged: () => { clearTimeout(retry); location.reload(); },
    apply: async (candidate, current) => {
      if (!current()) return false;
      const t = performance.now();
      const accepted = await globalThis.exact.reloadGeneration(candidate.plan, candidate.assets, current, candidate.module, candidate.rust);
      if (accepted) {
        navigator.sendBeacon(`/__dev/reloaded?epoch=${candidate.epoch}&seq=${candidate.seq}&dom=${Date.now()}&fetch=${candidate.fetchMs.toFixed(1)}&boot=${(performance.now() - t).toFixed(1)}`);
        requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?epoch=${candidate.epoch}&seq=${candidate.seq}&paint=${Date.now()}`));
      }
      return accepted;
    },
    // A new generation runs its plan's own design: the live one since, if any, comes after.
    // And the pin held, set again on its runner; its status says the new generation,
    // and the design its runner runs (read from it: /__dev/design is 204 when the
    // plan's own is the one to run, so no live answer would say it).
    applied: (candidate) => { clearTimeout(retry); show(null); generations += 1; delete globalThis.exact.designDigest; syncDesign(); channel.plan = candidate?.generation ?? channel.plan; if (channel.held) command(channel.held); else if (channel.nonce) status(); },
    failed: (error, current) => {
      show(String(error)); console.error("exact dev:", String(error));
      clearTimeout(retry);
      // A deterministic host refusal needs a new edit, not repeated execution
      // of the same candidate every 250 ms. Network discovery still retries.
      if (error.hostRefused) return;
      const discover = async () => {
        if (!current()) return;
        try {
          const { envelope, url } = await readEnvelope("/exact.json", AbortSignal.timeout(15000));
          if (current() && validIdentity(envelope.dev)) client.receive({ ...envelope.dev, envelope: new URL("exact.json", new URL(envelope.plan.url, url)).href });
        } catch { if (current()) retry = setTimeout(discover, 1000); }
      };
      retry = setTimeout(discover, 250);
    },
  });
  // The first boot's generation, named in the page (LLP 1007 §6): fetched
  // when glue.js asks. Its Rust module, if any, is still applied as an update.
  const firstMeta = document.querySelector('meta[name="exact-dev-generation"]');
  if (firstMeta) {
    const slot = globalThis.exactDevFirst ??= {};
    slot.provide = async () => {
      try {
        const candidate = await fetchGeneration(JSON.parse(firstMeta.content), AbortSignal.timeout(15000), true);
        if (!candidate.rust) client.adopt(candidate.generation);
        return candidate;
      } catch (error) { show(String(error)); console.error("exact dev:", String(error)); return null; }
    };
    slot.ready?.();
  }
  // The live table (LLP 1121 stage 4): a batch applies on its base; a stale
  // build asks for the snapshot. A frame before boot waits for it, the
  // latest only (a snapshot carries the whole design).
  let designing = Promise.resolve();
  // Applied in place (the wasm's `exact_design_rows`): its batch is the
  // page's only when `live` says applied.
  // `still()`, after every wait, says whether the frame is still this page's.
  const apply = async (frame, still = () => true) => {
    const x = globalThis.exact;
    await x.ready;
    if (!still()) return { live: "structure" };
    if (typeof x.wasm?.exact_design_rows !== "function") return { live: "unsupported" };
    const n = x.wasm.exact_design_rows(x.writeIn(JSON.stringify(frame))), out = JSON.parse(x.readOut(n));
    if (out.live === "applied") x.send(n);
    if (typeof out.design === "string" && out.design !== x.designDigest) { x.designDigest = out.design; status(); }
    if (out.unsupported) { console.warn("exact dev: the wide colour table is full: a colour was refused, its rows keep their value"); report({ kind: "unsupported", what: "batch", detail: "the wide colour table is full: a colour was refused" }); }
    return out;
  };
  // One at a time, in the stream's order: a snapshot fetched is never older
  // than a batch applied before it.
  // A snapshot asked for under one generation is that generation's: one
  // that lands after the next is dropped (that one asks again).
  let generations = 0;
  const snapshot = async () => {
    const asked = generations, still = () => asked === generations, response = await fetch("/__dev/design", { cache: "no-store" });
    if (response.status !== 200 || !still()) return;
    const frame = await response.json();
    if (still()) await apply(frame, still);
  };
  const syncDesign = () => { designing = designing.then(snapshot).catch((error) => console.error("exact dev:", String(error))); };
  const design = (frame) => {
    designing = designing.then(async () => {
      const out = await apply(frame);
      // A batch for another structure: a generation is on its way, which
      // syncs when applied.
      if (out.live === "stale") await snapshot();
      else if (out.live.startsWith("refused")) console.error("exact dev:", out.live);
    }).catch((error) => console.error("exact dev:", String(error)));
  };
  // The build channel (LLP 1122 D4): this page registers with its stream's
  // nonce, reports its effective facts, and takes `pin` from the design
  // server's clients; the other commands are reserved, and answered
  // unsupported. Every report waits for the stream's registration.
  const channel = { nonce: null, registered: null, revision: null, before: null, held: null, plan: null, told: null };
  // The tab's own id, kept across its reloads (sessionStorage); a duplicated
  // tab, which copies that storage, takes another when this one answers.
  const buildId = (async () => {
    const fresh = () => `web-${crypto.getRandomValues(new Uint32Array(2)).join("")}`;
    let id;
    try { id = sessionStorage.getItem("exact.dev.build"); } catch { return fresh(); }
    const tabs = typeof BroadcastChannel === "function" ? new BroadcastChannel("exact.dev.build") : null;
    if (id && tabs) {
      const mine = id;
      const taken = await new Promise((done) => { tabs.onmessage = (e) => { if (e.data === `taken:${mine}`) done(true); }; tabs.postMessage(`who:${mine}`); setTimeout(() => done(false), 60); });
      if (taken) id = null;
    }
    id ??= fresh();
    // Answered for any id this tab holds, one it just made included: a tab
    // duplicated from it copies that too.
    if (tabs) { const own = id; tabs.onmessage = (e) => { if (e.data === `who:${own}`) tabs.postMessage(`taken:${own}`); }; }
    try { sessionStorage.setItem("exact.dev.build", id); } catch { /* storage refused */ }
    return id;
  })();
  const post = (nonce, body) => fetch("/__dev/report", { method: "POST", body: JSON.stringify({ nonce, ...body }) });
  // A report goes on the stream whose registration it waited for: one that
  // a new stream (a reconnect) replaced meanwhile is dropped.
  const report = async (body) => {
    const { nonce, registered } = channel;
    if (!nonce || !(await registered) || channel.nonce !== nonce || channel.registered !== registered) return;
    await post(nonce, body).catch(() => {});
  };
  const wasmCall = async (name, ...args) => { const x = globalThis.exact; await x.ready; if (typeof x.wasm?.[name] !== "function") return null; const n = x.wasm[name](...args), out = JSON.parse(x.readOut(n)); x.send(n); return out; };
  const modes = async () => { const x = globalThis.exact; await x.ready; return x.wasm?.exact_set_mode ? JSON.parse(x.readOut(x.wasm.exact_set_mode(x.writeIn("?")))).modes ?? null : null; };
  // The design this build runs: the runner's, said beside any answer (a
  // batch for no structure changes nothing).
  const designNow = async () => { const x = globalThis.exact; await x.ready; if (typeof x.wasm?.exact_design_rows !== "function") return null; const out = JSON.parse(x.readOut(x.wasm.exact_design_rows(x.writeIn(JSON.stringify({ kind: "tamber-batch", structure: "", base: "", design: "", tokens: [] })))));  return out.design ?? null; };
  // The root's own size, the app's `setRootFontSize` rule set aside (navigation.js `beneathApp`).
  const hostSize = () => { const rule = document.getElementById("exact-root-font-size"); if (rule) rule.disabled = true; try { return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16; } finally { if (rule) rule.disabled = false; } };
  // The facts as they paint: the root's `color-scheme` (the app's
  // `setScheme` over the pin's, over the browser's `light dark`), resolved by
  // the system's scheme, which is a pin's while one is held.
  const facts = async () => {
    const rootStyle = getComputedStyle(document.documentElement), cs = rootStyle.colorScheme;
    const scheme = /dark/.test(cs) && !/light/.test(cs) ? "dark" : /light/.test(cs) && !/dark/.test(cs) ? "light" : globalThis.exact.systemScheme ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    const contrast = matchMedia("(prefers-contrast: more)").matches ? "more" : matchMedia("(prefers-contrast: less)").matches ? "less" : "no-preference";
    return { kind: "status", scheme, contrast, mode: await modes(), rootFontSize: parseFloat(rootStyle.fontSize), plan: channel.plan, design: globalThis.exact.designDigest ?? await designNow(), revision: channel.revision };
  };
  const status = async () => { const body = await facts(); channel.told = JSON.stringify(body); report(body); };
  // The facts change under the page too, and by the app's own `setScheme`,
  // `setMode` and `setRootFontSize`: after its commits, once a turn, a status
  // when they moved.
  for (const query of ["(prefers-color-scheme: dark)", "(prefers-contrast: more)", "(prefers-contrast: less)"]) matchMedia(query).addEventListener?.("change", () => { if (channel.nonce) status(); });
  let due = false;
  const committed = () => {
    if (due || !channel.nonce) return;
    due = true;
    setTimeout(() => {
      designing = designing.then(async () => {
        due = false;
        const body = await facts();
        if (JSON.stringify(body) !== channel.told) { channel.told = JSON.stringify(body); report(body); }
      }).catch((error) => console.error("exact dev:", String(error)));
    }, 0);
  };
  globalThis.exact.ready?.then?.(() => { globalThis.exact.committed = committed; });
  const pin = async (frame) => {
    const root = document.documentElement.style, p = frame.pin;
    // What it was before the first pin, handed back by `none`; each pin
    // replaces the whole set.
    channel.before ??= { fontSize: root.fontSize, hostFont: root.getPropertyValue("--exact-host-font-size"), mode: await modes() };
    const x = globalThis.exact, pins = (m) => Object.entries(m).map(([a, v]) => `${a}=${v}`).join(",");
    // The scheme is the system's, beneath the app's `setScheme` (the root's
    // own style): a rule over the page's `light dark`, and the system scheme
    // the runner's `prefers-color-scheme` reads (navigation.js
    // `preferences`), told as the browser's change is.
    const sheet = document.getElementById("exact-dev-pin") ?? document.head.appendChild(Object.assign(document.createElement("style"), { id: "exact-dev-pin" }));
    sheet.textContent = p?.scheme ? `:root{color-scheme:${p.scheme}}` : "";
    if ((x.systemScheme ?? null) !== (p?.scheme ?? null)) { x.systemScheme = p?.scheme ?? null; dispatchEvent(new Event("resize")); }
    // The root font size as `prefer` sets it (navigation.js): the size an
    // app's percentage is of, too.
    root.fontSize = p?.rootFontSize ? `${p.rootFontSize}px` : channel.before.fontSize;
    if (p?.rootFontSize) root.setProperty("--exact-host-font-size", root.fontSize);
    else if (channel.before.hostFont) root.setProperty("--exact-host-font-size", channel.before.hostFont);
    else root.removeProperty("--exact-host-font-size");
    await wasmCall("exact_set_root_font_size", hostSize());
    const mode = pins({ ...(channel.before.mode ?? {}), ...(p?.mode ?? {}) });
    if (mode) {
      // A mode refused leaves the modes before the first pin, not the last pin's.
      const out = await wasmCall("exact_set_mode", x.writeIn(mode));
      if (out?.error) { report({ kind: "unsupported", what: "pin.mode", detail: out.error }); const back = pins(channel.before.mode ?? {}); if (back) await wasmCall("exact_set_mode", x.writeIn(back)); }
    }
    // A page cannot override the browser's `prefers-contrast`.
    if (p?.contrast) report({ kind: "unsupported", what: "pin.contrast", detail: "a web page cannot pin the browser's contrast" });
    if (!p) channel.before = null;
    channel.held = p ? frame : null;
    channel.revision = frame.revision;
    status();
  };
  const command = (frame) => {
    designing = designing.then(() => frame.kind === "pin" ? pin(frame) : report({ kind: "unsupported", what: frame.kind })).catch((error) => console.error("exact dev:", String(error)));
  };
  es.onmessage = (event) => {
    let message;
    try { message = JSON.parse(event.data); } catch { show("the dev stream sent invalid JSON"); return; }
    if (message.error) { show(message.error, message.source); console.error("exact dev:", message.error); return; }
    if (message.fresh) { if (new URLSearchParams(location.search).get('agent') !== '1') location.reload(); return; }
    if (message.gpu !== undefined) {
      // A drive owns its clock and code. Do not change either behind the driver.
      if (new URLSearchParams(location.search).get('agent') === '1') return;
      // A declared module's rebuild names it (LLP 1009 D6); the primary's does not.
      const versions = message.module ? (globalThis.exact.gpuVersions ??= {}) : globalThis.exact, key = message.module ? `gpu/${message.module}` : 'gpuVersion';
      versions[key] = message.gpu;
      if (globalThis.exact.gpu) (async () => {
        if (message.gpu !== versions[key]) return;
        const result = await globalThis.exact.gpu.swap(message.gpu, message.module);
        show(result.errors.join('\n') || null, "gpu");
        navigator.sendBeacon(`/__dev/gpu?g=${message.gpu}&swap=${result.ms.toFixed(1)}`);
      })().catch(error => { show(String(error), "gpu"); console.error('exact dev:', error); });
      return;
    }
    if (message.kind === "tamber-batch" || message.kind === "tamber-snapshot") { design(message); return; }
    if (message.kind === "pin" || message.kind === "inspect" || message.kind === "outline" || message.kind === "hold") { command(message); return; }
    if (message.hello && message.nonce && message.nonce !== channel.nonce) {
      const nonce = channel.nonce = message.nonce;
      channel.plan ??= message.generation ?? null;
      // Reports wait for this; a refused registration sends none.
      channel.registered = (async () => {
        await globalThis.exact.ready;
        const answer = await post(nonce, { kind: "register", id: await buildId, host: "web", capabilities: ["pin"], plan: channel.plan, design: globalThis.exact.designDigest ?? await designNow() });
        return answer.ok;
      })().catch(() => false);
      status();
    }
    if (message.ready === false) return;
    client.receive(message);
  };
}
