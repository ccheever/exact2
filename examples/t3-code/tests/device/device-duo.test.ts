import { describe, expect, test } from 'bun:test';
import { duoFoldState, duoHoldOrientation, duoScreenSettled, deviceFolding, foldingValues } from '../../src/device/device-duo';

// DeviceDuoControls and DeviceAndroidFoldControls from the module's synthetic
// reports (the shapes T3DeviceDuo.swift / T3DeviceFold.swift publish).
describe('iPhone Duo fold shapes and stands', () => {
  // The inner display (3) reports landscape for a vertical phone; the cover (1) portrait.
  const duo = (extra: object = {}, orientation = 'landscape_left') => ({ status: 'streaming', inputConnected: true, orientation,
    duo: { supported: true, screenId: 3, hingeAngle: 180, hingePose: '', pending: false, requested: null, error: '', ...extra } });
  const summary = (view: ReturnType<typeof deviceFolding>) => view.groups.map(group => [group.label, group.stands.map(stand => [stand.label, stand.tip, stand.selected, stand.glyph, stand.rotated])]);
  test('two groups, the reference labels and tooltips, the open shape chosen', () => {
    const view = deviceFolding(duo(), 'ios', true, true, 'k1');
    expect(view.kind).toBe('duo');
    expect(summary(view)).toEqual([
      ['Fold shape', [['Closed', 'Closed', false, 'closed', false], ['Book', 'Book', false, 'book', false], ['Open', 'Open', true, 'open', false]]],
      ['Device stance', [['Laptop stand', 'Laptop stand', false, 'laptop', false], ['Tent stand', 'Tent stand', false, 'tent', false]]],
    ]);
  });
  test('a horizontal phone folds as a laptop and its fold glyphs turn a quarter', () => {
    const view = deviceFolding(duo({}, 'portrait'), 'ios', true, true, 'k2');
    expect(view.groups[0]!.stands.map(stand => [stand.label, stand.rotated])).toEqual([['Closed', true], ['Laptop', true], ['Open', true]]);
    expect(view.groups[1]!.stands.every(stand => !stand.rotated)).toBe(true);
  });
  test('duoFoldState, duoScreenSettled and duoHoldOrientation follow duoControl.ts', () => {
    expect(duoFoldState({ orientation: 'portrait', screenId: 1, hingeAngle: null, hingePose: '' })).toEqual({ fold: 'closed', stand: false, phoneVertical: true, settled: true });
    expect(duoFoldState({ orientation: 'landscape_right', screenId: 3, hingeAngle: 37.5, hingePose: 'laptop' })).toEqual({ fold: 'half', stand: true, phoneVertical: true, settled: true });
    expect(duoScreenSettled({ screenId: 1, hingeAngle: 90 })).toBe(false);
    expect(duoScreenSettled({ screenId: 3, hingeAngle: 0 })).toBe(false);
    expect([duoHoldOrientation(true, 1), duoHoldOrientation(false, 1), duoHoldOrientation(true, 3), duoHoldOrientation(false, null)])
      .toEqual(['portrait', 'landscape_left', 'landscape_left', 'portrait_upside_down']);
  });
  test('a fold keeps how the phone is held through an unsettled display handoff', () => {
    expect(deviceFolding(duo(), 'ios', true, true, 'k3').groups[0]!.stands[1]!.label).toBe('Book');
    // Closing: the inner display still reports, now in the cover's portrait, at angle 0.
    const handoff = deviceFolding(duo({ hingeAngle: 0 }, 'portrait'), 'ios', true, true, 'k3');
    expect(handoff.groups[0]!.stands.map(stand => [stand.label, stand.selected, stand.rotated])).toEqual([['Closed', true, false], ['Book', false, false], ['Open', false, false]]);
    // Settled on the cover held horizontally: it reads Laptop.
    expect(deviceFolding(duo({ screenId: 1, hingeAngle: 0 }, 'landscape_left'), 'ios', true, true, 'k3').groups[0]!.stands[1]!.label).toBe('Laptop');
  });
  test('a stand is pressed by its pose, not the fold; folding out of it restores the hold first', () => {
    const key = 'k4';
    deviceFolding(duo(), 'ios', true, true, key);
    expect(foldingValues('duo', 'tent', duo(), key)).toEqual(['pose:tent']);
    const tent = duo({ hingePose: 'tent', hingeAngle: 80 }, 'portrait');
    const view = deviceFolding(tent, 'ios', true, true, key);
    expect(view.groups.flatMap(group => group.stands.filter(stand => stand.selected).map(stand => stand.id))).toEqual(['tent']);
    expect(view.groups[0]!.stands[1]!.label).toBe('Book');
    expect(foldingValues('duo', 'open', tent, key)).toEqual(['orientation:landscape_left', 'angle:180']);
    expect(foldingValues('duo', 'half', duo(), key)).toEqual(['angle:90']);
    expect(foldingValues('duo', 'flat', duo(), key)).toEqual([]);
  });
  test('fold buttons wait while a stand, or a queued stand, lands', () => {
    const onStand = deviceFolding(duo({ hingePose: 'laptop', hingeAngle: 90, pending: true, requested: { control: 'angle', value: 90 } }), 'ios', true, true, 'k5');
    expect(onStand.groups[0]!.stands.every(stand => stand.waits)).toBe(true);
    expect(onStand.groups[1]!.stands.some(stand => stand.waits)).toBe(false);
    const queued = deviceFolding(duo({ pending: true, requested: { control: 'pose', value: 'tent' } }), 'ios', true, true, 'k5');
    expect(queued.groups[0]!.stands.every(stand => stand.waits)).toBe(true);
    const folding = deviceFolding(duo({ pending: true, requested: { control: 'angle', value: 0 } }), 'ios', true, true, 'k5');
    expect(folding.groups[0]!.stands.some(stand => stand.waits)).toBe(false);
  });
  test('only beside the 3D Duo the hub can hinge; disabled without input; the error shows', () => {
    expect(deviceFolding(duo(), 'ios', false, true).kind).toBe('');
    expect(deviceFolding({ duo: { supported: false } }, 'ios', true, true).kind).toBe('');
    expect(deviceFolding({}, 'ios', true, true).kind).toBe('');
    expect(deviceFolding(duo(), 'ios', true, false).enabled).toBe(false);
    expect(deviceFolding(duo({ error: 'Device control timed out. Its position is unknown.' }), 'ios', true, true).error).toBe('Device control timed out. Its position is unknown.');
  });
});

describe('Android fold controls', () => {
  const fold = (extra: object = {}, status = 'streaming') => ({ status, fold: { supported: true, posture: 'opened', hingeAngle: 180, pending: false, error: '', ...extra } });
  test('Fold and Unfold, the posture pressed, in 3D or flat', () => {
    for (const phone of [true, false]) {
      const view = deviceFolding(fold(), 'android', phone, true);
      expect(view.kind).toBe('fold');
      expect(view.groups[0]!.stands.map(stand => [stand.label, stand.glyph, stand.selected])).toEqual([['Fold device', 'closed', false], ['Unfold device', 'open', true]]);
    }
  });
  test('hidden until the emulator folds; disabled while pending or not streaming', () => {
    expect(deviceFolding({ fold: { supported: false } }, 'android', true, true).kind).toBe('');
    expect(deviceFolding({}, 'android', true, true).kind).toBe('');
    expect(deviceFolding(fold({ pending: true }), 'android', true, true).enabled).toBe(false);
    expect(deviceFolding(fold({}, 'connecting'), 'android', true, true).enabled).toBe(false);
    expect(deviceFolding(fold({ error: 'Fold command timed out.' }), 'android', true, true).error).toBe('Fold command timed out.');
  });
  test('press values', () => {
    expect([foldingValues('fold', 'closed'), foldingValues('fold', 'tent'), foldingValues('', 'open')]).toEqual([['closed'], [], []]);
  });
});
