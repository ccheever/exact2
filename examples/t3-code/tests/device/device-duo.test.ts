import { describe, expect, test } from 'bun:test';
import { DUO_POSES, duoSelected, deviceFolding, foldingValue } from '../../src/device/device-duo';

// DeviceDuoControls and DeviceAndroidFoldControls from the module's synthetic
// reports (the shapes T3DeviceDuo.swift / T3DeviceFold.swift publish).
describe('iPhone Duo stands', () => {
  const duo = (extra: object = {}) => ({ status: 'streaming', inputConnected: true, duo: { supported: true, screenId: 3, hingeAngle: 180, hingePose: 'open', pending: false, requested: null, error: '', ...extra } });
  test('two groups, the reference labels and tooltips, the open shape chosen', () => {
    const view = deviceFolding(duo(), 'ios', true, true);
    expect(view.kind).toBe('duo');
    expect(view.groups.map(group => [group.label, group.stands.map(stand => [stand.label, stand.tip, stand.selected])])).toEqual([
      ['Fold shape', [['Closed stand', 'Closed', false], ['Book stand', 'Book / bookshelf', false], ['Open stand', 'Open', true]]],
      ['Device stance', [['Laptop stand', 'Laptop', false], ['Tent stand', 'Tent', false]]],
    ]);
    expect(DUO_POSES.map(pose => pose.angle)).toEqual([0, 90, 180, 90, 80]);
  });
  test('shape follows the angle; a stance only its own pose', () => {
    expect(['closed', 'book', 'open', 'laptop', 'tent'].filter(id => duoSelected(id, 90, 'laptop'))).toEqual(['book', 'laptop']);
    expect(['closed', 'book', 'open', 'laptop', 'tent'].filter(id => duoSelected(id, 0, ''))).toEqual(['closed']);
    expect(['closed', 'book', 'open', 'laptop', 'tent'].filter(id => duoSelected(id, 37.5, ''))).toEqual(['book']);
    expect(['closed', 'book', 'open', 'laptop', 'tent'].filter(id => duoSelected(id, null, 'tent'))).toEqual(['tent']);
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
    expect([foldingValue('duo', 'tent'), foldingValue('duo', 'flat'), foldingValue('fold', 'closed'), foldingValue('fold', 'tent'), foldingValue('', 'open')])
      .toEqual(['pose:tent', null, 'closed', null, null]);
  });
});
