#!/usr/bin/env bun
// An older-server stand-in: a loopback proxy in front of a lane T3 server that reports an older
// version, with the self-update a server under T3's service launcher advertises (`boot-service`,
// with progress), so the clone offers to update it: the composer's Auto balance banner and its
// machines popover, the composer notice, Settings › Connections. Everything passes through except
// the version and capabilities of the environment descriptor (HTTP `/.well-known/t3/environment`,
// `server.getConfig`, `subscribeServerConfig`'s snapshots) and the update itself, which is answered
// here as the reference streams it (apps/server/src/cloud/selfUpdate.ts, ws.ts): progress
// "downloading", then "installing", then "complete". The descriptor then keeps the old version
// while the server "restarts", then reports the target and closes the client sockets, as a
// restart would. Nothing is downloaded or installed.
//
//   bun older-server.mjs <listen port> <server port> [--version V] [--target V] [--download ms] [--install ms] [--restart ms]

/** The capabilities a launcher-managed server advertises (reference ServerEnvironment.ts, environment.ts). */
export const SELF_UPDATE = { serverSelfUpdate: "boot-service", serverSelfUpdateProgress: true };
export const UPDATE_TAGS = ["server.updateServerWithProgress", "server.updateServer"];

/** Every environment descriptor in `value` (an object with `environmentId` and `serverVersion`) at `version`, with self-update. */
export function rewrite(value, version) {
  if (Array.isArray(value)) return value.map((item) => rewrite(item, version));
  if (!value || typeof value !== "object") return value;
  const out = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, rewrite(item, version)]));
  if (typeof out.environmentId === "string" && typeof out.serverVersion === "string") {
    out.serverVersion = version;
    out.capabilities = { ...(out.capabilities ?? {}), ...SELF_UPDATE };
  }
  return out;
}

/** The frames that answer one update request, each with the delay (ms) before it. */
export function updateScript(request, { target, download, install }) {
  const result = { targetVersion: target, method: "boot-service" };
  if (request.tag === "server.updateServer") return [[download + install, { _tag: "Exit", requestId: request.id, exit: { _tag: "Success", value: result } }]];
  const chunk = (value) => ({ _tag: "Chunk", requestId: request.id, values: [value] });
  return [[0, chunk({ type: "progress", stage: "downloading" })], [download, chunk({ type: "progress", stage: "installing" })],
    [install, chunk({ type: "complete", result })], [0, { _tag: "Exit", requestId: request.id, exit: { _tag: "Success", value: null } }]];
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name, fallback) => { const at = argv.indexOf(name); return at >= 0 ? argv[at + 1] : fallback; };
  const listen = Number(argv[0]), upstream = Number(argv[1]);
  if (!Number.isInteger(listen) || !Number.isInteger(upstream)) throw new Error("usage: bun older-server.mjs <listen port> <server port> [--version V] [--target V] [--download ms] [--install ms] [--restart ms]");
  const options = { older: flag("--version", "0.0.46-nightly.20261001.1"), target: flag("--target", "0.1.1-nightly.20261018.3302"),
    download: Number(flag("--download", 6000)), install: Number(flag("--install", 3000)), restart: Number(flag("--restart", 10000)) };
  let updated = false;
  const version = () => (updated ? options.target : options.older);
  const sockets = new Set();
  const rewriteText = (text) => { try { return JSON.stringify(rewrite(JSON.parse(text), version())); } catch { return text; } };

  /** Answers an update request here; true when the frame was the proxy's to answer (or an Ack of one it answered). */
  const intercept = (ws, frame) => {
    if ((frame?._tag === "Ack" || frame?._tag === "Interrupt") && ws.data.scripted.has(String(frame.requestId))) return true;
    if (frame?._tag !== "Request" || !UPDATE_TAGS.includes(frame.tag)) return false;
    ws.data.scripted.add(String(frame.id));
    console.log(`update asked: ${frame.tag} ${JSON.stringify(frame.payload ?? {})}`);
    let at = 0;
    for (const [delay, reply] of updateScript(frame, options)) { at += delay; setTimeout(() => ws.send(JSON.stringify(reply)), at); }
    setTimeout(() => { console.log("restarting"); setTimeout(() => { updated = true; console.log(`now ${options.target}`); for (const socket of sockets) socket.close(1012, "restart"); }, options.restart); }, at);
    return true;
  };

  Bun.serve({
    hostname: "127.0.0.1", port: listen,
    async fetch(request, server) {
      const url = new URL(request.url);
      if (url.pathname === "/ws" && server.upgrade(request, { data: { query: url.search, queue: [], upstream: null, scripted: new Set() } })) return undefined;
      const headers = new Headers(request.headers);
      headers.delete("host");
      headers.delete("accept-encoding");
      const reply = await fetch(`http://127.0.0.1:${upstream}${url.pathname}${url.search}`, { method: request.method, headers, redirect: "manual",
        body: request.method === "GET" || request.method === "HEAD" ? undefined : await request.arrayBuffer() });
      const out = new Headers(reply.headers);
      out.delete("content-encoding");
      out.delete("content-length");
      if (!(reply.headers.get("content-type") ?? "").includes("json")) return new Response(reply.body, { status: reply.status, headers: out });
      return new Response(rewriteText(await reply.text()), { status: reply.status, headers: out });
    },
    websocket: {
      open(ws) {
        sockets.add(ws);
        const up = new WebSocket(`ws://127.0.0.1:${upstream}/ws${ws.data.query}`);
        ws.data.upstream = up;
        up.onopen = () => { for (const text of ws.data.queue) up.send(text); ws.data.queue = []; };
        up.onmessage = (event) => ws.send(rewriteText(String(event.data)));
        up.onclose = () => ws.close();
        up.onerror = () => ws.close();
      },
      message(ws, message) {
        const text = String(message);
        let frames;
        try { frames = [JSON.parse(text)].flat(); } catch { frames = null; }
        if (frames) {
          const rest = frames.filter((frame) => !intercept(ws, frame));
          if (rest.length === 0) return;
          if (rest.length < frames.length) return void forward(ws, JSON.stringify(rest.length === 1 ? rest[0] : rest));
        }
        forward(ws, text);
      },
      close(ws) { sockets.delete(ws); ws.data.upstream?.close(); },
    },
  });
  const forward = (ws, text) => { if (ws.data.upstream?.readyState === WebSocket.OPEN) ws.data.upstream.send(text); else ws.data.queue.push(text); };
  console.log(`older-server stand-in on 127.0.0.1:${listen} in front of ${upstream}: reports ${options.older}, updates to ${options.target}`);
}
