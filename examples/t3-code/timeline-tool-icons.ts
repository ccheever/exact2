// Ported from T3 Code 1e2ecbd975, packages/shared/src/favicon.ts and
// MessagesTimeline.tsx (MIT, LICENSE-T3). Exact adaptation: native app URLs
// are awaited by an independent read mutation, keyed by connection, and re-minted after 5 min.
import { composerNow } from './composer-controls';
import { wakeShell } from './r10-connect-timing';
import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { assetUrl } from './settings-b-icons';

/**
 * Mirrors Codex's generic Browser Use fallback: ask the page origin for its
 * conventional favicon and let the image element fall back to a browser glyph.
 * Chrome-backed tools can pass their tab's explicit favicon URL separately.
 */
function faviconUrlForPage(rawUrl: string | null | undefined, _size = 32): string | null {
  if (!rawUrl || rawUrl.length > 4096) return null;
  try {
    const pageUrl = new URL(rawUrl);
    if (pageUrl.protocol !== "http:" && pageUrl.protocol !== "https:") return null;
    return new URL("/favicon.ico", pageUrl.origin).href;
  } catch {
    return null;
  }
}

const THEMED_FAVICON_BY_HOSTNAME: Readonly<
  Record<string, Readonly<{ light: string; dark: string }>>
> = {
  "github.com": {
    light: "https://github.githubassets.com/favicons/favicon.svg",
    dark: "https://github.githubassets.com/favicons/favicon-dark.svg",
  },
};

function themedFaviconUrlForPage(
  rawUrl: string | null | undefined,
  appearance: "light" | "dark",
): string | null {
  if (!rawUrl || rawUrl.length > 4096) return null;
  try {
    const pageUrl = new URL(rawUrl);
    if (pageUrl.protocol !== "http:" && pageUrl.protocol !== "https:") return null;
    return THEMED_FAVICON_BY_HOSTNAME[pageUrl.hostname.toLowerCase()]?.[appearance] ?? null;
  } catch {
    return null;
  }
}

/** Accepts image URLs supplied by a trusted provider event. */
function explicitFaviconUrl(rawUrl: string | null | undefined): string | null {
  if (!rawUrl || rawUrl.length > 4096) return null;
  try {
    const url = new URL(rawUrl);
    return url.protocol === "http:" || url.protocol === "https:" || url.protocol === "data:"
      ? url.href
      : null;
  } catch {
    return null;
  }
}

/**
 * Chooses a website icon for the app's resolved theme. Provider-supplied
 * variants mirror Codex's Chrome-selected favicon path. A small site-owned
 * fallback table covers websites whose conventional favicon is illegible in
 * one appearance without recoloring full-color icons.
 */
export function toolActivityFaviconUrl(
  icon: {
    readonly pageUrl: string;
    readonly faviconUrl?: string | undefined;
    readonly faviconUrlDark?: string | undefined;
  },
  appearance: "light" | "dark",
  size = 32,
): string | null {
  if (appearance === "dark") {
    return (
      explicitFaviconUrl(icon.faviconUrlDark) ??
      themedFaviconUrlForPage(icon.pageUrl, "dark") ??
      explicitFaviconUrl(icon.faviconUrl) ??
      faviconUrlForPage(icon.pageUrl, size)
    );
  }
  return (
    explicitFaviconUrl(icon.faviconUrl) ??
    themedFaviconUrlForPage(icon.pageUrl, "light") ??
    faviconUrlForPage(icon.pageUrl, size)
  );
}


// Source precedence matches the provider schema. Warning/severe rows deliberately
// use the status glyph rather than an integration image.
export function toolActivityIcon(item: Obj): Obj {
  if (item.tone === 'warning' || item.tone === 'error' || item.tone === 'severe') return {};
  return obj(item.toolIcon ?? obj(item.toolSource).icon);
}

type IconClient = Pick<T3Client, 'generation' | 'environmentId' | 'origin' | 'connection' | 'projection' | 'rpc'>;
type Asset = { src: string; at: number };
const nativeUrls = new WeakMap<IconClient, Map<string, Asset>>();
const inflight = new WeakMap<IconClient, Map<string, Promise<void>>>();
const nativeKey = (client: IconClient, app: Obj) => JSON.stringify([client.generation, client.environmentId, client.origin, app]);
const nativeApp = (icon: Obj): Obj | null => {
  const app = obj(icon.app);
  return icon._tag === 'native-app' &&
    ((app._tag === 'app-id' && str(app.appId)) || (app._tag === 'display-name' && str(app.displayName))) ? app : null;
};

export function toolActivityIconSources(client: IconClient, item: Obj): { iconLight: string; iconDark: string } {
  const icon = toolActivityIcon(item);
  if (icon._tag === 'website') {
    const website = { pageUrl: str(icon.pageUrl), faviconUrl: str(icon.faviconUrl), faviconUrlDark: str(icon.faviconUrlDark) };
    return { iconLight: toolActivityFaviconUrl(website, 'light') ?? '', iconDark: toolActivityFaviconUrl(website, 'dark') ?? '' };
  }
  if (icon._tag === 'themed-logo') return { iconLight: str(icon.logoUrl), iconDark: str(icon.logoUrlDark, str(icon.logoUrl)) };
  const app = nativeApp(icon);
  const src = app ? nativeUrls.get(client)?.get(nativeKey(client, app))?.src ?? '' : '';
  return { iconLight: src, iconDark: src };
}

/** Call during the connected refresh before projecting timeline rows. */
export async function syncToolActivityIcons(client: IconClient, native: Native, now = composerNow(client)): Promise<void> {
  if (!native.available || client.connection !== 'connected') return;
  let cache = nativeUrls.get(client);
  if (!cache) { cache = new Map(); nativeUrls.set(client, cache); }
  let pending = inflight.get(client);
  if (!pending) { pending = new Map(); inflight.set(client, pending); }
  const waits: Promise<void>[] = [];
  for (const row of arr(client.projection.visibleTurnItems)) {
    const app = nativeApp(toolActivityIcon(obj(row.item)));
    if (!app) continue;
    const key = nativeKey(client, app), previous = cache.get(key);
    if (previous && now - previous.at < 5 * 60_000) continue;
    const existing = pending.get(key);
    if (existing) { waits.push(existing); continue; }
    // Pass iteration values as arguments: pinned Hermes does not preserve
    // for-of block captures in an async IIFE after the loop advances.
    const request = loadNativeToolIcon(client, native, app, key, previous, now, client.origin, cache, pending);
    pending.set(key, request);
    waits.push(request);
  }
  await Promise.all(waits);
}

async function loadNativeToolIcon(
  client: IconClient, native: Native, app: Obj, key: string, previous: Asset | undefined,
  now: number, origin: string, destination: Map<string, Asset>, requests: Map<string, Promise<void>>,
): Promise<void> {
  let src = previous?.src ?? '';
  try {
    const reply = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'native-app-icon', app } }));
    src = assetUrl(origin, str(reply.relativeUrl));
  } catch { /* Keep an already drawn icon during a transient refresh failure. */ }
  destination.set(key, { src, at: now });
  if (destination.size > 128) destination.delete(destination.keys().next().value!);
  requests.delete(key);
  await wakeShell(native);
}

/** Pure readiness: snapshot rendering never waits for an icon URL. */
export function toolActivityIconsNeeded(client: IconClient, now = composerNow(client)): boolean {
  if (client.connection !== 'connected') return false;
  return arr(client.projection.visibleTurnItems).some(row => {
    const app = nativeApp(toolActivityIcon(obj(row.item)));
    if (!app) return false;
    const key = nativeKey(client, app), previous = nativeUrls.get(client)?.get(key);
    return !inflight.get(client)?.has(key) && (!previous || now - previous.at >= 5 * 60_000);
  });
}
