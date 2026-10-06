// The floating player's per-thread state (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/previewMiniPlayerStore.ts). The reference's zustand store becomes one plain object
// per client connection (r6-media-device.ts keeps it), keyed by thread. It lives in memory only,
// as the reference's store does: nothing is persisted, so a relaunch forgets position and width.
// The browser source, browserMiniPlayerSource and selectThreadPreviewMiniPlayerTabId are left out:
// this client floats devices only.
import type { DevicePlatform, PreviewMiniPlayerPosition } from './previewMiniPlayerLayout';

export type { PreviewMiniPlayerPosition, PreviewMiniPlayerSize } from './previewMiniPlayerLayout';

/** What the floating player mirrors: a device stream. */
export type PreviewMiniPlayerSource = {
  readonly kind: 'device';
  readonly hostId: string;
  readonly deviceId: string;
  readonly platform: DevicePlatform;
  readonly name: string;
};

export interface PreviewMiniPlayerState {
  readonly source: PreviewMiniPlayerSource;
  readonly position: PreviewMiniPlayerPosition | null;
  /** Height always follows the mirrored source's aspect ratio. */
  readonly width: number | null;
  readonly lastInteraction: 'drag' | 'resize';
}

export function previewMiniPlayerSourceKey(source: Pick<PreviewMiniPlayerSource, 'hostId' | 'deviceId'>): string {
  return `device:${encodeURIComponent(source.hostId)}:${encodeURIComponent(source.deviceId)}`;
}

/** usePreviewMiniPlayerStore's state and actions, keyed by the thread's key instead of a ScopedThreadRef. */
export class PreviewMiniPlayerStore {
  byThreadKey: Record<string, PreviewMiniPlayerState> = {};

  get(threadKey: string): PreviewMiniPlayerState | null { return this.byThreadKey[threadKey] ?? null; }

  open(threadKey: string, source: PreviewMiniPlayerSource): void {
    const current = this.byThreadKey[threadKey];
    if (current && previewMiniPlayerSourceKey(current.source) === previewMiniPlayerSourceKey(source)) return;
    this.byThreadKey = { ...this.byThreadKey, [threadKey]: {
      source, position: current?.position ?? null, width: current?.width ?? null, lastInteraction: current?.lastInteraction ?? 'drag',
    } };
  }

  close(threadKey: string): void {
    if (!(threadKey in this.byThreadKey)) return;
    const { [threadKey]: _closed, ...byThreadKey } = this.byThreadKey;
    this.byThreadKey = byThreadKey;
  }

  /** `sourceKey` guards against a drag that outlives the source it started on. */
  move(threadKey: string, sourceKey: string, position: PreviewMiniPlayerPosition): void {
    const current = this.byThreadKey[threadKey];
    if (!current || previewMiniPlayerSourceKey(current.source) !== sourceKey) return;
    if (current.position?.x === position.x && current.position.y === position.y && current.lastInteraction === 'drag') return;
    this.byThreadKey = { ...this.byThreadKey, [threadKey]: { ...current, position, lastInteraction: 'drag' } };
  }

  resize(threadKey: string, sourceKey: string, width: number, position?: PreviewMiniPlayerPosition): void {
    const current = this.byThreadKey[threadKey];
    if (!current || previewMiniPlayerSourceKey(current.source) !== sourceKey
      || (current.width === width && current.lastInteraction === 'resize' && (!position || (current.position?.x === position.x && current.position.y === position.y)))) return;
    this.byThreadKey = { ...this.byThreadKey, [threadKey]: { ...current, width, position: position ?? current.position, lastInteraction: 'resize' } };
  }

  removeThread(threadKey: string): void { this.close(threadKey); }
}
