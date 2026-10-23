// The dev loop's page refusing this tool's frame, in words (the second hands-on pass, finding 3): `tamber tool --via`
// asks the page's frame-ancestors (scripts/tamber-frame.mjs) and answers at /__tamber/tool/canvas-check, the tool's
// own route, not tamber/1. None when it lets the tool frame it, or when nothing answers (a tool served without
// --via, or the stub): the canvas then says nothing more.
export async function canvasRefusal(api: string): Promise<{ refused: string }> {
  try {
    const res = await fetch(`${api}/__tamber/tool/canvas-check`, { exactTimeout: 8000, redirect: 'error' });
    if (res.status !== 200) return { refused: '' };
    const c = (await res.json()) as { via?: unknown; allowed?: unknown; origins?: unknown; portFixes?: unknown };
    if (c.allowed !== false) return { refused: '' };
    // As the terminal says it (scripts/tamber-frame.mjs `refusalWords`): a port to set only when the page names a
    // loopback tool; else the page's own policy, which no port changes.
    const origins = Array.isArray(c.origins) ? c.origins.map(String) : [];
    const head = `The dev loop at ${String(c.via ?? '')} ${origins.length ? `lets only ${origins.join(' ')} frame its page` : 'lets no page frame it'}, not this tool, so the canvas stays empty`;
    return { refused: c.portFixes === true ? `${head}: start the loop with EXACT_TOOL_PORT=${new URL(api).port}.` : `${head}: its page's own policy refuses framing, which no port changes.` };
  } catch { return { refused: '' }; }
}
