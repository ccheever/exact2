const port = Number(process.env.PORT ?? 4173);
const server = Bun.serve({
  port,
  async fetch(request) {
    const url = new URL(request.url);
    const path = url.pathname === "/" ? "dist/index.html" : `dist${decodeURIComponent(url.pathname)}`;
    const file = Bun.file(path);
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    const headers = new Headers();
    if (path.endsWith(".wasm")) headers.set("content-type", "application/wasm");
    if (path.endsWith(".js")) headers.set("content-type", "text/javascript");
    if (path.endsWith(".html")) headers.set("content-type", "text/html; charset=utf-8");
    return new Response(file, { headers });
  },
});
console.log(`Lanterns at http://127.0.0.1:${server.port}`);
