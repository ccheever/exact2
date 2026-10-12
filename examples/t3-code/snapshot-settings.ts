// SnapShots settings and attachment presentation, after the reference
// SnapShotSettings.tsx / SnapShotSettings.logic.ts (macOS "direct" mode) and
// SnapShotAttachmentDetails.tsx. Pure projection plus one native availability check.
import { commandLabel, sameSnapshotShortcut, snapshotConflict, snapshotKeyLabels, snapshotShortcutAria, snapshotSystemConflict } from './snapshot-shortcut';
import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { referencedImageIds } from './r4-composer-attachments';

export type SnapshotNativeState = { available: boolean; registered: boolean; verified: boolean; message: string; screenRecording: boolean; accessibility: boolean; recording: boolean; candidate: string };
export type SnapshotCheck = { available: boolean; message: string } | null;

/** Reference snapShotStatus / snapShotSetupSummary for a macOS direct backend. */
export function snapshotStatus(state: SnapshotNativeState, enabled: boolean): string {
  if (!state.available) return 'Checking snapshots…';
  if (!enabled) return 'Turn this on to set up snapshots.';
  if (state.message) return 'Capture needs attention';
  if (state.verified) return 'Ready to capture';
  return state.registered ? 'Shortcut saved' : 'Finish shortcut setup';
}
/** Reference snapShotSetupComplete + snapShotSetupButtonLabel; "" hides the button. */
export function snapshotSetupLabel(state: SnapshotNativeState, enabled: boolean, includeAccessibility: boolean): string {
  if (!enabled) return '';
  const accessReady = state.available && !state.message;
  const permissionsReady = state.screenRecording && (!includeAccessibility || state.accessibility);
  if (accessReady && permissionsReady && state.registered) return '';
  return accessReady ? 'Manage capture' : 'Continue setup';
}
/** Reference shortcutStatus chain (recording, keybinding conflict, check, saved state). */
export function snapshotShortcutStatus(state: SnapshotNativeState, conflict: string, check: SnapshotCheck): string {
  if (state.recording) return 'Press your shortcut. Esc cancels.';
  if (conflict) return `T3 Code already uses this for "${commandLabel(conflict)}".`;
  if (check) return check.available ? 'Ready to save.' : check.message;
  return state.registered ? 'Shortcut saved.' : '';
}
/** Reference macPermissionMessage copy is native's; any of it marks a grant problem. */
export const isSnapshotPermissionMessage = (text: string) => /^Allow .* in System Settings, then restart T3 Code\.$/.test(text) || text.includes('access is required');

export function snapshotSettingsView(client: T3Client, state: SnapshotNativeState, check: SnapshotCheck, error: string) {
  const device = client.local.deviceSettings;
  const saved = device.snapShotShortcut, candidate = state.candidate;
  const changed = candidate !== '' && !sameSnapshotShortcut(candidate, saved);
  const display = changed ? candidate : saved;
  const conflict = changed ? snapshotConflict(client.config, candidate) : '';
  const soundSelection = device.snapShotPlaySound ? device.snapShotSound : 'off';
  return {
    available: state.available, captureAvailable: state.available, error, recording: state.recording, candidate, shortcut: saved,
    shortcutError: conflict ? `T3 Code already uses this for "${commandLabel(conflict)}".` : check && !check.available ? check.message : '',
    screenRecording: state.screenRecording, accessibility: state.accessibility, message: state.message,
    enabled: device.snapShotEnabled, includeAccessibility: device.snapShotIncludeAccessibility,
    playSound: device.snapShotPlaySound, sound: device.snapShotSound, flash: device.snapShotFlash, animations: device.snapShotAnimations,
    statusText: snapshotStatus(state, device.snapShotEnabled),
    setupLabel: snapshotSetupLabel(state, device.snapShotEnabled, device.snapShotIncludeAccessibility),
    shortcutStatus: snapshotShortcutStatus(state, conflict, changed ? check : null),
    shortcutAria: `Record snapshot shortcut, currently ${snapshotShortcutAria(display)}`,
    keys: snapshotKeyLabels(display).map((label, index) => ({ id: `${index}-${label}`, label })),
    changed, canSave: changed && !conflict && check?.available === true,
    soundSelection, soundLabel: soundSelection === 'off' ? 'Off' : soundSelection === 'soft-pop' ? 'Whoosh' : 'Click',
    // Reference captureSetupInitialStep (resume): Shortcut once on, access ready and registered.
    setupReady: device.snapShotEnabled && state.available && !state.message && state.registered,
  };
}

export async function snapshotSettings(client: T3Client, native: Native | null | undefined, active: boolean) {
  const idle: SnapshotNativeState = { available: false, registered: false, verified: false, message: '', screenRecording: false, accessibility: false, recording: false, candidate: '' };
  if (!active) return snapshotSettingsView(client, idle, null, '');
  try {
    if (!native?.available) throw new Error('Open on macOS to configure window capture.');
    const raw = await client.snapshotState(native);
    const message = str(raw.error);
    const state: SnapshotNativeState = { available: true, registered: raw.enabled === true, verified: raw.verified === true, message: isSnapshotPermissionMessage(message) ? message : '', screenRecording: raw.screenRecording === true, accessibility: raw.accessibility === true, recording: raw.recording === true, candidate: str(raw.candidate) };
    let check: SnapshotCheck = null;
    const candidate = state.candidate;
    if (candidate && !sameSnapshotShortcut(candidate, client.local.deviceSettings.snapShotShortcut) && !snapshotConflict(client.config, candidate)) {
      // Reference checkShortcut: system conflicts, then the platform's own probe.
      const system = snapshotSystemConflict(candidate);
      if (system) check = { available: false, message: system };
      else {
        try { const value = await client.snapshotCheck(native, candidate); check = { available: value.available === true, message: str(value.message) || 'Could not check this shortcut.' }; }
        catch (error) { check = { available: false, message: error instanceof Error ? error.message : 'Could not check this shortcut.' }; }
      }
    }
    return snapshotSettingsView(client, state, check, isSnapshotPermissionMessage(message) ? '' : message);
  } catch (error) { return snapshotSettingsView(client, idle, null, error instanceof Error ? error.message : 'Could not read capture permissions.'); }
}

/** Reference snapShotAccessibilityDetails: element-tree JSON, flat text, or accessibleText. */
export function snapshotContents(source: Obj): string {
  const accessibility = obj(source.accessibility);
  if (accessibility.format === 'element-tree') return JSON.stringify(accessibility, null, 2).slice(0, 32_000);
  const text = accessibility.format === 'flat-text' ? str(accessibility.text).trim() : str(source.accessibleText).trim();
  return text.slice(0, 32_000);
}
/** The composer's SnapShot attachment frames (reference SnapShotAttachmentDetails). */
export function snapshotDraftTiles(client: T3Client) {
  const referenced = referencedImageIds(client.draft, arr(client.snapshotDrafts).map(image => str(image.id))); // r4-composer: removal asks first
  return arr(client.snapshotDrafts).map(image => {
    const source = obj(image.source), app = str(source.appName, 'Window');
    const included = Boolean(source.accessibility) || str(source.accessibleText).trim() !== '';
    // An image picked with Attach files has no capture source: the shelf shows it as a plain tile.
    const snapshot = Object.keys(source).length > 0;
    return { id: str(image.id), name: str(image.name), snapshot, app, title: snapshot ? str(source.windowTitle) || 'Captured window' : '', letter: app.slice(0, 1).toUpperCase(),
      included, contents: snapshotContents(source), referenced: referenced.has(str(image.id)) };
  });
}
