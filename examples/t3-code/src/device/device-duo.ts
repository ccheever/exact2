// The iPhone Duo's fold shapes and stands and an Android foldable's fold buttons beside the
// device rail (MIT reference 9c161462c9, see LICENSE-T3: components/device/DeviceDuoControls.tsx,
// DeviceAndroidFoldControls.tsx, DeviceDuoGlyph.tsx, DeviceStreamView.tsx foldingControls;
// packages/client-runtime/src/device/duoControl.ts duoScreenSettled / duoFoldState /
// duoHoldOrientation). The hinge itself, its command queue and the fold reads are the app module's
// (T3DeviceDuo.swift, T3DeviceFold.swift), reported as `presentation.deviceStreams[key].duo` and
// `.fold`; a press reaches it through `deviceInput` (`duo` with `angle:<deg>`, `orientation:<o>`
// or `pose:<id>`, `fold` with `closed` / `opened`).
import { obj, str, type Obj } from '../client/domain';

/** Fold shapes move only the hinge; stands are native presets that also place the device. */
export const DUO_FOLDS = [
  { id: 'closed', angle: 0 },
  { id: 'half', angle: 90 },
  { id: 'open', angle: 180 },
] as const;
export const DUO_STANDS = [
  { id: 'laptop', label: 'Laptop stand' },
  { id: 'tent', label: 'Tent stand' },
] as const;

export type DuoStand = { id: string; label: string; tip: string; selected: boolean; glyph: string; rotated: boolean; waits: boolean };
export type DuoStandGroup = { id: string; label: string; stands: DuoStand[] };
export type DeviceFolding = {
  /** "duo", "fold" or "" (nothing beside the rail). */
  kind: string; enabled: boolean; groups: DuoStandGroup[]; error: string;
};

const NONE: DeviceFolding = { kind: '', enabled: false, groups: [], error: '' };
export const emptyFolding = (): DeviceFolding => ({ ...NONE, groups: [] });

const num = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);

type DuoScreen = { orientation: string; screenId: number | null; hingeAngle: number | null; hingePose: string };

/**
 * duoScreenSettled: whether the reporting display is the one the fold shows. A fold across the
 * closed position hands off in steps, and the outgoing display first reports the incoming one's
 * orientation, so an unsettled screen's orientation does not say how the phone is held.
 */
export function duoScreenSettled(screen: Pick<DuoScreen, 'screenId' | 'hingeAngle'>): boolean {
  const angle = screen.hingeAngle ?? (screen.screenId === 1 ? 0 : 180);
  return (screen.screenId === 1) === (angle === 0);
}

/** duoFoldState: the inner panel is mounted a quarter turn from the cover, so its landscape means a vertical phone. */
export function duoFoldState(screen: DuoScreen) {
  const angle = screen.hingeAngle ?? (screen.screenId === 1 ? 0 : 180);
  const landscape = screen.orientation.startsWith('landscape');
  return {
    fold: angle === 0 ? 'closed' : angle === 180 ? 'open' : 'half',
    stand: screen.hingePose === 'laptop' || screen.hingePose === 'tent',
    phoneVertical: screen.screenId === 1 ? !landscape : landscape,
    settled: duoScreenSettled(screen),
  } as const;
}

/** duoHoldOrientation: native rotation is read in the frame of the display active when it arrives. */
export function duoHoldOrientation(vertical: boolean, screenId: number | null): string {
  if (screenId === 1) return vertical ? 'portrait' : 'landscape_left';
  return vertical ? 'landscape_left' : 'portrait_upside_down';
}

const duoScreen = (report: Obj): DuoScreen => {
  const duo = obj(report.duo);
  return { orientation: str(report.orientation, 'portrait'), screenId: num(duo.screenId), hingeAngle: num(duo.hingeAngle), hingePose: str(duo.hingePose) };
};

/**
 * DeviceDuoControls' two pieces of state, per stream: how the phone was last held on a settled
 * display (a fold never changes it; display handoffs report the other display's orientation), and
 * how it was held when a stand was chosen (folding out of a stand returns it there).
 */
const holds = new Map<string, { settled: boolean; stand: boolean }>();
function holdOf(key: string, screen: DuoScreen) {
  const state = duoFoldState(screen);
  let hold = holds.get(key);
  if (!hold) { hold = { settled: state.phoneVertical, stand: state.phoneVertical }; holds.set(key, hold); }
  if (state.settled) hold.settled = state.phoneVertical;
  return { state, hold, vertical: state.stand ? hold.stand : hold.settled };
}
/** DeviceStreamView foldingControls for the device the report describes (`key`: its stream). */
export function deviceFolding(report: Obj, platform: string, phoneShown: boolean, inputConnected: boolean, key = ''): DeviceFolding {
  if (platform === 'android') {
    // DeviceAndroidFoldControls: only once the emulator says it folds.
    const fold = obj(report.fold);
    if (fold.supported !== true) return emptyFolding();
    const posture = str(fold.posture), streaming = str(report.status) === 'streaming';
    return {
      kind: 'fold', enabled: streaming && fold.pending !== true, error: str(fold.error),
      groups: [{ id: 'fold', label: 'Android fold controls', stands: [
        { id: 'closed', label: 'Fold device', tip: 'Fold device', selected: posture === 'closed', glyph: 'closed', rotated: false, waits: false },
        { id: 'opened', label: 'Unfold device', tip: 'Unfold device', selected: posture === 'opened', glyph: 'open', rotated: false, waits: false },
      ] }],
    };
  }
  const duo = obj(report.duo);
  // DeviceDuoControls unmounts with the 3D Duo, and its held orientation with it.
  if (!phoneShown || duo.supported !== true) { holds.delete(key); return emptyFolding(); }
  const screen = duoScreen(report), { state, vertical } = holdOf(key, screen);
  // Folding out of a stand sends the rotation back, then the fold, and only one command can
  // wait. The fold buttons hold until a stand, or anything queued on one, has landed.
  const requested = obj(duo.requested);
  const foldWaits = duo.pending === true && (state.stand || str(requested.control) === 'pose');
  const labels: Record<string, string> = { closed: 'Closed', half: vertical ? 'Book' : 'Laptop', open: 'Open' };
  return {
    kind: 'duo', enabled: inputConnected, error: str(duo.error),
    groups: [
      { id: 'shape', label: 'Fold shape', stands: DUO_FOLDS.map(({ id }) => ({
        id, label: labels[id]!, tip: labels[id]!, selected: !state.stand && state.fold === id,
        glyph: id === 'half' ? 'book' : id, rotated: !vertical, waits: foldWaits })) },
      { id: 'stance', label: 'Device stance', stands: DUO_STANDS.map(({ id, label }) => ({
        id, label, tip: label, selected: screen.hingePose === id, glyph: id, rotated: false, waits: false })) },
    ],
  };
}

/** The `deviceInput` values a press sends, in order; none when the control cannot act. */
export function foldingValues(kind: string, id: string, report: Obj = {}, key = ''): string[] {
  if (kind === 'fold') return id === 'closed' || id === 'opened' ? [id] : [];
  if (kind !== 'duo') return [];
  const screen = duoScreen(report), { state, hold } = holdOf(key, screen);
  const fold = DUO_FOLDS.find(entry => entry.id === id);
  if (fold) return [...(state.stand ? [`orientation:${duoHoldOrientation(hold.stand, screen.screenId)}`] : []), `angle:${fold.angle}`];
  if (!DUO_STANDS.some(entry => entry.id === id)) return [];
  if (!state.stand) hold.stand = hold.settled;
  return [`pose:${id}`];
}
