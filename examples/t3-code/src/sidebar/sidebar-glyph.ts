// The project glyph on sidebar surfaces (ProjectFavicon.tsx,
// ProjectMonogram.tsx; MIT, see LICENSE-T3). An icon override wins (monogram
// with its own letters and color, emoji, Lucide icon); otherwise the server's
// favicon image, else the automatic monogram. Favicon URLs are asked of each project's own server
// once per project and connection and cached, inside the pass that sees the project.
import { obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { assetUrl, decodeIcon, iconPath, inkOf, surfaceOf } from '../settings/settings-b-icons';
import { fleet } from '../connections/settings-b-fleet';

export interface ProjectGlyph { kind: string; d: string; emoji: string; src: string; text: string; ink: string; surface: string; gray: string; graySurface: string }
type Identity = (name: string) => { projectMark: string; projectInk: string; projectSurface: string };

/** The favicon's `grayscale` filter (Filter Effects luminance) for receding slim rows, per scheme. */
export function grayIdentity(ink: string): { projectGray: string; projectGraySurface: string } {
  const gray = (hex: string) => {
    const value = parseInt(hex.slice(1, 7), 16);
    if (!/^#[0-9a-fA-F]{6}/.test(hex) || !Number.isFinite(value)) return '#808080';
    const level = Math.round(0.2126 * (value >> 16 & 255) + 0.7152 * (value >> 8 & 255) + 0.0722 * (value & 255));
    return `#${level.toString(16).padStart(2, '0').repeat(3)}`;
  };
  const [light = '#71717b', dark = light] = (/^light-dark\((#[0-9a-fA-F]{6}),\s*(#[0-9a-fA-F]{6})\)$/.exec(ink) ?? [ink, ink, ink]).slice(1);
  return { projectGray: `light-dark(${gray(light)}, ${gray(dark)})`, projectGraySurface: `light-dark(${gray(light)}24, ${gray(dark)}24)` };
}

// Data sources have no clock: entries are keyed by the server (its environment and connection generation) and bounded.
const favicons = new Map<string, string>();
const asking = new Set<string>();
/** The connection a project's favicon is asked of: the focused environment's, or a connected background one's. */
export interface FaviconServer { environmentId: string; generation: number }
const faviconKey = (server: FaviconServer, project: Obj) => JSON.stringify([server.generation, server.environmentId, str(project.workspaceRoot), str(project.faviconPath)]);
/**
 * The server a project record belongs to: `environmentId` when given, else the record's own (a sidebar scope member), else
 * a background row's `fleet:<environment>:<id>` id; with none, the focused environment.
 */
export function faviconServer(client: T3Client, project: Obj, environmentId = ''): FaviconServer | null {
  const environment = environmentId || str(project.environmentId) || (/^fleet:([^:]+):/.exec(str(project.id))?.[1] ?? '');
  if (!environment || environment === client.environmentId) return { environmentId: client.environmentId, generation: client.generation };
  const entry = [...fleet.entries.values()].find(candidate => candidate.environmentId === environment);
  return entry ? { environmentId: entry.environmentId, generation: entry.generation } : null;
}
/** projectFaviconUrlAtom's answer for a project on its own server, '' while unknown or for the server's fallback marker. */
export function faviconSrc(client: T3Client, project: Obj, environmentId = ''): string {
  const server = faviconServer(client, project, environmentId);
  return server ? favicons.get(faviconKey(server, project)) ?? '' : '';
}

/** isProjectFaviconFallbackUrl: the server answers a missing favicon with its fallback asset. */
export function faviconFromAnswer(origin: string, answer: unknown): string {
  const url = assetUrl(origin, str(obj(answer).relativeUrl));
  return !url || url.split(/[?#]/)[0]!.split('/').pop() === 'project-favicon-missing' ? '' : url;
}

/**
 * Asks one server once per connection for the favicon of each of its projects without an override (`ask` makes
 * `assets.createUrl` on that server; its URL is resolved against `origin`). A failure is remembered as no favicon until
 * the connection changes, so a pass never waits on it twice. Awaited inside the pass that sees the projects.
 */
export async function syncProjectFavicons(server: FaviconServer, origin: string, projects: readonly Obj[], ask: (payload: Obj) => Promise<unknown>): Promise<void> {
  const asks = projects.flatMap(project => {
    const cwd = str(project.workspaceRoot), key = faviconKey(server, project);
    if (!cwd || decodeIcon(project.projectIcon).kind || favicons.has(key) || asking.has(key)) return [];
    asking.add(key);
    return [(async () => {
      let src = '';
      try { src = faviconFromAnswer(origin, await ask({ resource: { _tag: 'project-favicon', cwd, ...(str(project.faviconPath) ? { path: str(project.faviconPath) } : {}) } })); }
      catch { src = ''; }
      favicons.set(key, src);
      if (favicons.size > 128) favicons.delete(favicons.keys().next().value!);
      asking.delete(key);
    })()];
  });
  await Promise.all(asks);
}
/** Inside each refresh: the focused environment's projects. Background environments ask in their own pass (settings-b-fleet.ts). */
export async function syncFavicons(client: T3Client, native: Native): Promise<void> {
  if (!native?.available || client.connection !== 'connected') return;
  await syncProjectFavicons({ environmentId: client.environmentId, generation: client.generation }, client.origin, client.shell.projects,
    payload => client.rpc(native, 'assets.createUrl', payload));
}

/** ProjectFavicon for one project record (never a display label: the saved title decides the monogram). */
/** `environmentId`: the project's own server when it is not the focused one (faviconServer infers it otherwise). */
export function projectGlyph(client: T3Client, project: Obj | undefined, identity: Identity, environmentId = ''): ProjectGlyph {
  const name = str(project?.title), auto = identity(name);
  const glyph = (kind: string, ink: string, surface: string, extra: Partial<ProjectGlyph> = {}): ProjectGlyph => {
    const { projectGray, projectGraySurface } = grayIdentity(ink);
    return { kind, d: '', emoji: '', src: '', text: auto.projectMark, ink, surface, gray: projectGray, graySurface: projectGraySurface, ...extra };
  };
  const icon = decodeIcon(project?.projectIcon);
  if (icon.kind === 'monogram') return glyph('monogram', icon.color ? inkOf(icon.color) : auto.projectInk, icon.color ? surfaceOf(icon.color) : auto.projectSurface, { text: icon.text || auto.projectMark });
  if (icon.kind === 'emoji' && icon.emoji) return glyph('emoji', auto.projectInk, auto.projectSurface, { emoji: icon.emoji });
  if (icon.kind === 'lucide') return glyph('lucide', icon.color ? inkOf(icon.color) : 'light-dark(#71717b, #818181)', '#00000000', { d: iconPath(icon.name) });
  const src = project ? faviconSrc(client, project, environmentId) : '';
  return src ? glyph('image', auto.projectInk, auto.projectSurface, { src }) : glyph('monogram', auto.projectInk, auto.projectSurface);
}
