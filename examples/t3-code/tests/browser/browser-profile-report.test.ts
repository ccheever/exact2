// BrowserProfileReporter.test.ts at T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3), its 3 tests under their own names, and
// the pass over this client: an operable server browser is told the profiles once, and again when they change.
import { describe, expect, it } from 'bun:test';
import { profileReport, reportBrowserProfiles, takeDueProfileReports, type ProfileReport, type ReportedProfiles } from '../../src/browser/browser-profile-report';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

const first = 'environment-1', second = 'environment-2';
const config = (label: string) => ({ label });
const input = (recordingFrameRate: number): ProfileReport => ({ profiles: [], defaultProfileId: 'default', recordingFrameRate });

describe('takeDueProfileReports', () => {
  it('reports each environment once per config and input', () => {
    const reported: ReportedProfiles = new Map();
    const configA = config('a'), configB = config('b'), settings = input(10);
    const targets = new Map([[first, configA], [second, configB]]);
    expect(takeDueProfileReports(reported, targets, settings)).toEqual([first, second]);
    expect(takeDueProfileReports(reported, new Map(targets), settings)).toEqual([]);
    const restarted = config('a-restarted');
    expect(takeDueProfileReports(reported, new Map([[first, restarted], [second, configB]]), settings)).toEqual([first]);
  });
  it('reports every target again when the profile settings change', () => {
    const reported: ReportedProfiles = new Map();
    const targets = new Map([[first, config('a')], [second, config('b')]]);
    takeDueProfileReports(reported, targets, input(10));
    expect(takeDueProfileReports(reported, targets, input(30))).toEqual([first, second]);
  });
  it('reports an environment again once it becomes operable again', () => {
    const reported: ReportedProfiles = new Map();
    const configA = config('a'), settings = input(10);
    takeDueProfileReports(reported, new Map([[first, configA]]), settings);
    expect(takeDueProfileReports(reported, new Map(), settings)).toEqual([]);
    expect(takeDueProfileReports(reported, new Map([[first, configA]]), settings)).toEqual([first]);
  });
});

describe('the profile report pass', () => {
  it('tells an operable server browser once, and again when the frame rate changes', () => {
    const calls: Obj[] = [];
    const client = {
      environmentId: first, ready: true, connection: 'connected', preferencesLoaded: true, scopes: ['preview:operate'],
      config: { environment: { capabilities: { serverBrowser: true } } },
      local: { clientSettings: { browserRecordingFrameRate: 60 }, browserProfiles: [], browserDefaultProfileId: 'default' },
      rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, payload }); return {}; },
    } as unknown as T3Client;
    const native = { available: true } as unknown as Native, fleet = new EnvironmentFleet();
    reportBrowserProfiles(client, native, fleet);
    reportBrowserProfiles(client, native, fleet);
    expect(calls).toEqual([{ method: 'preview.reportProfiles', payload: { profiles: [], defaultProfileId: 'default', recordingFrameRate: 60 } }]);
    (client.local.clientSettings as Obj).browserRecordingFrameRate = 30;
    reportBrowserProfiles(client, native, fleet);
    expect(calls.map(call => (call.payload as Obj).recordingFrameRate)).toEqual([60, 30]);
    expect(profileReport(client).recordingFrameRate).toBe(30);
  });
  it('tells no environment without a server browser or the operate scope', () => {
    const calls: unknown[] = [];
    const client = {
      environmentId: first, ready: true, connection: 'connected', preferencesLoaded: true, scopes: ['orchestration:read'],
      config: { environment: { capabilities: { serverBrowser: true } } }, local: { clientSettings: {} },
      rpc: async () => { calls.push(1); return {}; },
    } as unknown as T3Client;
    reportBrowserProfiles(client, { available: true } as unknown as Native, new EnvironmentFleet());
    expect(calls).toEqual([]);
  });
});
