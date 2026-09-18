import { resolve } from "node:path";

const root = resolve(import.meta.dir, "dist");
const requested = Number(process.env.PORT || 0);
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: requested,
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/favicon.ico") return new Response(null, { status: 204 });
    const relative = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
    const path = resolve(root, relative);
    if (!path.startsWith(root)) return new Response("Not found", { status: 404 });
    const file = Bun.file(path);
    if (!(await file.exists())) return new Response("Not found", { status: 404 });
    return new Response(file);
  },
});

console.log(JSON.stringify({ pid: process.pid, port: server.port, url: server.url.href }));
