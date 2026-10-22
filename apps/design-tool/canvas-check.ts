// The dev loop's page refusing this tool's frame, in words (the second hands-on pass, finding 3): `tamber tool --via`
// asks the page's frame-ancestors (scripts/tamber-frame.mjs) and answers at /__tamber/tool/canvas-check, the tool's
// own route, not tamber/1. None when it lets the tool frame it, or when nothing answers (a tool served without
// --via, or the stub): the canvas then says nothing more.
export async function canvasRefusal(api: string): Promise<{ refused: string }> {
  try {
    const res = await fetch(`${api}/__tamber/tool/canvas-check`, { exactTimeout: 8000, redirect: 'error' });
    if (res.status !== 200) return { refused: '' };
    const c = (await res.json()) as { via?: unknown; allowed?: unknown; origins?: unknown };
    if (c.allowed !== false) return { refused: '' };
    const origins = Array.isArray(c.origins) ? c.origins.map(String).join(' ') : '';
    return { refused: `The dev loop at ${String(c.via ?? '')} lets only ${origins || 'no page'} frame its page, not this tool, so the canvas stays empty: start the loop with EXACT_TOOL_PORT=${new URL(api).port}.` };
  } catch { return { refused: '' }; }
}
