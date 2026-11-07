import { describe, expect, test } from 'bun:test';
import { decodeHomeChromeEvent, homeConnectionTitle } from './home-chrome';

describe('native Home chrome events', () => {
  test('accepts workspace tab selections and rejects unknown event kinds', () => {
    expect(decodeHomeChromeEvent('{"kind":"tab","value":"settings"}')).toEqual({ kind: 'tab', value: 'settings' });
    expect(decodeHomeChromeEvent('{"kind":"tab","value":"unknown"}')).toEqual({ kind: 'tab', value: 'unknown' });
    expect(decodeHomeChromeEvent('{"kind":"tabs","value":"settings"}')).toBeNull();
  });

  test('cleans a duplicated possessive in the reconnecting environment label', () => {
    expect(homeConnectionTitle({ networkStatus: 'online', connectionError: null, connectingEnvironments: [{ environmentLabel: "Nate's’s Mac Mini" }],
      hasConnectingEnvironment: true, hasPendingShellSnapshot: false, hasLoadedShellSnapshot: true, hasReadyEnvironment: false })).toEqual({
      label: "Reconnecting to Nate's Mac Mini", progress: true,
    });
    expect(homeConnectionTitle({ networkStatus: 'online', connectionError: null, connectingEnvironments: [{ environmentLabel: "Nate's Mac Mini" }],
      hasConnectingEnvironment: true, hasPendingShellSnapshot: false, hasLoadedShellSnapshot: true, hasReadyEnvironment: false }).label).toBe("Reconnecting to Nate's Mac Mini");
  });
});
