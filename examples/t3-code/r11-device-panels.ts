// Lane r11-device: the rest of the right panel's surfaces survive a relaunch (MIT reference, see
// LICENSE-T3: apps/web/src/rightPanelStore.ts persists every surface of `byThreadKey` and
// migratePersistedRightPanelState reads them back). Lane r10-device kept the Files explorer, file
// tabs and the linked pull requests list (r10-device-panels.ts); this adds the Diff (`{ id: "diff" }`),
// the Device surface with the device it shows (`{ kind: "device", target }`: DevicePanel streams it
// again when the thread's device session is still open, else lists the devices), a pull request's
// detail (`pullRequestSurface`: projectId, host, repository, number, url; a malformed one is dropped
// as the migration drops it) and a sent attachment (`attachment:<id>`, with the attachment's
// metadata, read again from its signed asset URL).
import { num, obj, str, type Obj } from './domain';
import { prSurfaceId, type PrTarget } from './r5-panels-pr';
import type { AttachmentMeta } from './r5-panels-attach';
import type { DeviceTarget } from './r6-media-device';

export type R11SavedSurface =
  | { id: 'diff'; kind: 'diff'; path: ''; line: 0 }
  | { id: string; kind: 'device'; path: ''; line: 0; device?: DeviceTarget; title?: string }
  | { id: string; kind: 'pull-request'; path: ''; line: 0; pr: PrTarget }
  | { id: string; kind: 'attachment'; path: string; line: 0; attachment: AttachmentMeta };
export type R11Surface = { id: string; kind: string; path: string; line: number; pr?: unknown; attachment?: unknown; device?: unknown; title?: string };
export const R11_KINDS = new Set(['diff', 'device', 'pull-request', 'attachment']);

/** rightPanelStore's DeviceTabTarget, or nothing when malformed. */
export function deviceTarget(value: unknown): DeviceTarget | undefined {
  const entry = obj(value), platform = str(entry.platform);
  if (!str(entry.hostId) || !str(entry.deviceId) || (platform !== 'ios' && platform !== 'android')) return undefined;
  return { hostId: str(entry.hostId), deviceId: str(entry.deviceId), platform, name: str(entry.name) };
}

/** The migration's pull request check: a project, a repository and a positive safe-integer number. */
export function prTargetOf(value: unknown): PrTarget | null {
  const entry = obj(value);
  if (typeof entry.projectId !== 'string' || typeof entry.repository !== 'string' || !Number.isSafeInteger(entry.number) || num(entry.number) < 1) return null;
  return { projectId: entry.projectId, host: str(entry.host).toLowerCase(), repository: entry.repository, number: num(entry.number), url: str(entry.url) };
}

function attachmentOf(value: unknown): AttachmentMeta | null {
  const entry = obj(value);
  if (!str(entry.id) || !str(entry.name)) return null;
  return { id: str(entry.id), name: str(entry.name), mimeType: str(entry.mimeType), sizeBytes: Math.max(0, num(entry.sizeBytes)) };
}

/** One of this lane's surfaces as saved, or null (not one of them, or malformed). */
export function keepR11(surface: R11Surface): R11SavedSurface | null {
  if (surface.kind === 'diff') return surface.id === 'diff' ? { id: 'diff', kind: 'diff', path: '', line: 0 } : null;
  if (surface.kind === 'device') {
    const device = deviceTarget(surface.device);
    const id = device ? `device:${encodeURIComponent(device.hostId)}:${encodeURIComponent(device.deviceId)}` : 'device';
    if (surface.id !== id && surface.id !== 'device') return null;
    return { id: surface.id, kind: 'device', path: '', line: 0, ...(device ? { device } : {}), ...(surface.title ? { title: surface.title } : {}) };
  }
  if (surface.kind === 'pull-request') {
    const pr = prTargetOf(surface.pr);
    return pr ? { id: prSurfaceId(pr), kind: 'pull-request', path: '', line: 0, pr } : null;
  }
  if (surface.kind === 'attachment') {
    const attachment = attachmentOf(surface.attachment);
    return attachment && surface.id === `attachment:${attachment.id}` ? { id: surface.id, kind: 'attachment', path: attachment.name, line: 0, attachment } : null;
  }
  return null;
}

/** A saved record's surface fields as read back (the raw entry, before keepR11 re-validates it). */
export function readR11(entry: Obj): R11Surface {
  return { id: str(entry.id), kind: str(entry.kind), path: str(entry.path), line: num(entry.line), pr: entry.pr, attachment: entry.attachment, device: entry.device, title: str(entry.title) };
}

/** What the restored panel must do once it is in place: reopen the Diff's own panel and name the device. */
export function restoredEffects(surfaces: readonly R11Surface[], active: string, visible: boolean): { diff: boolean; device: DeviceTarget | undefined } {
  const device = deviceTarget((surfaces.find(surface => surface.kind === 'device' && surface.id === active) ?? surfaces.find(surface => surface.kind === 'device'))?.device);
  return { diff: visible && active === 'diff' && surfaces.some(surface => surface.id === 'diff'), device };
}
