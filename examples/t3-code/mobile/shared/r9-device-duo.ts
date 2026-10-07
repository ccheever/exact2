// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r9-device-duo.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r9-device: the iPhone Duo's stands and an Android foldable's fold buttons beside the device
// rail (MIT reference, see LICENSE-T3: components/device/DeviceDuoControls.tsx,
// DeviceAndroidFoldControls.tsx, DeviceDuoGlyph.tsx, DeviceStreamView.tsx foldingControls;
// packages/client-runtime/src/device/duoControl.ts DUO_POSES). The hinge itself, its command queue
// and the fold reads are the app module's (R9DeviceDuo.swift, R9DeviceFold.swift), reported as
// `presentation.deviceStreams[key].duo` and `.fold`; a press reaches it through `r6DeviceInput`
// (`duo` with `pose:<id>`, `fold` with `closed` / `opened`).
import { obj, str, type Obj } from './domain';

export const DUO_POSES = [
  { id: 'closed', label: 'Closed', angle: 0 },
  { id: 'book', label: 'Book', angle: 90 },
  { id: 'open', label: 'Open', angle: 180 },
  { id: 'laptop', label: 'Laptop', angle: 90 },
  { id: 'tent', label: 'Tent', angle: 80 },
] as const;
export type DuoPoseId = (typeof DUO_POSES)[number]['id'];

export type R9DevStand = { id: string; label: string; tip: string; selected: boolean; glyph: string };
export type R9DevStandGroup = { id: string; label: string; stands: R9DevStand[] };
export type R9DevFolding = {
  /** "duo", "fold" or "" (nothing beside the rail). */
  kind: string; enabled: boolean; groups: R9DevStandGroup[]; error: string;
};

const NONE: R9DevFolding = { kind: '', enabled: false, groups: [], error: '' };
export const emptyR9Folding = (): R9DevFolding => ({ ...NONE, groups: [] });

const num = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);

/** DeviceDuoControls `selected`: the fold shape from the hinge angle, a stance from the pose. */
export function duoSelected(id: string, hingeAngle: number | null, hingePose: string): boolean {
  const fold = hingeAngle === null ? null : hingeAngle === 0 ? 'closed' : hingeAngle === 180 ? 'open' : 'book';
  return id === 'laptop' || id === 'tent' ? hingePose === id : fold === id;
}

/** DeviceStreamView foldingControls for the device the report describes. */
export function r9Folding(report: Obj, platform: string, phoneShown: boolean, inputConnected: boolean): R9DevFolding {
  if (platform === 'android') {
    // DeviceAndroidFoldControls: only once the emulator says it folds.
    const fold = obj(report.fold);
    if (fold.supported !== true) return emptyR9Folding();
    const posture = str(fold.posture), streaming = str(report.status) === 'streaming';
    return {
      kind: 'fold', enabled: streaming && fold.pending !== true, error: str(fold.error),
      groups: [{ id: 'fold', label: 'Android fold controls', stands: [
        { id: 'closed', label: 'Fold device', tip: 'Fold device', selected: posture === 'closed', glyph: 'closed' },
        { id: 'opened', label: 'Unfold device', tip: 'Unfold device', selected: posture === 'opened', glyph: 'open' },
      ] }],
    };
  }
  const duo = obj(report.duo);
  if (!phoneShown || duo.supported !== true) return emptyR9Folding();
  const angle = num(duo.hingeAngle), pose = str(duo.hingePose);
  const stand = (pose_: (typeof DUO_POSES)[number]): R9DevStand => ({
    id: pose_.id, label: `${pose_.label} stand`, tip: `${pose_.label}${pose_.id === 'book' ? ' / bookshelf' : ''}`,
    selected: duoSelected(pose_.id, angle, pose), glyph: pose_.id,
  });
  return {
    kind: 'duo', enabled: inputConnected, error: str(duo.error),
    groups: [
      { id: 'shape', label: 'Fold shape', stands: DUO_POSES.slice(0, 3).map(stand) },
      { id: 'stance', label: 'Device stance', stands: DUO_POSES.slice(3).map(stand) },
    ],
  };
}

/** The `r6DeviceInput` value a press sends, or null when the control cannot act. */
export function r9FoldingValue(kind: string, id: string): string | null {
  if (kind === 'duo') return DUO_POSES.some(pose => pose.id === id) ? `pose:${id}` : null;
  if (kind === 'fold') return id === 'closed' || id === 'opened' ? id : null;
  return null;
}
