// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3: apps/web/src/browser/BrowserProfileReporter.ts.
//
// Every environment that runs its own browser (`environment.capabilities.serverBrowser`) and that this client may
// operate (`preview:operate`) hears this client's browser profiles, default profile and recording frame rate
// (`preview.reportProfiles`), so the agent's tabs open in them and its recordings keep the chosen rate. An environment is
// told again when its server config changes (a restarted server has forgotten them), when it becomes operable again, and
// when the settings change. Each shell answer runs one pass; a failed report is not shown, as the reference's is not.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, type Native } from '../client/protocol';
import { EnvironmentFleet, fleet } from '../connections/settings-b-fleet';
import { letGo } from '../client/let-go';
import { browserProfilePrefs, type BrowserProfile } from './browser-profiles';

/** PreviewReportProfilesInput. */
export type ProfileReport = { profiles: BrowserProfile[]; defaultProfileId: string; recordingFrameRate: number };
/** What each environment was last told, with the config it was running then. */
export type ReportedProfiles = Map<string, { config: object; input: ProfileReport }>;

/**
 * takeDueProfileReports: the targets that need `input` reported, recorded in `reported`. An environment that dropped
 * out of `targets` is forgotten, so it is reported again once it becomes operable.
 */
export function takeDueProfileReports(reported: ReportedProfiles, targets: ReadonlyMap<string, object>, input: ProfileReport): string[] {
  for (const environmentId of reported.keys()) if (!targets.has(environmentId)) reported.delete(environmentId);
  const due: string[] = [];
  for (const [environmentId, config] of targets) {
    const last = reported.get(environmentId);
    if (last && last.config === config && sameReportInput(last.input, input)) continue;
    reported.set(environmentId, { config, input });
    due.push(environmentId);
  }
  return due;
}
const sameReportInput = (left: ProfileReport, right: ProfileReport) => left.profiles === right.profiles
  && left.defaultProfileId === right.defaultProfileId && left.recordingFrameRate === right.recordingFrameRate;

/** The client's profile settings as one report; the same arrays while they are unchanged. */
export function profileReport(client: T3Client): ProfileReport {
  const prefs = browserProfilePrefs(client), rate = Number(obj(client.local.clientSettings).browserRecordingFrameRate);
  return { profiles: prefs.browserProfiles, defaultProfileId: prefs.browserDefaultProfileId, recordingFrameRate: rate === 60 ? 60 : 30 };
}

const operable = (config: Obj, scopes: readonly string[]) => obj(obj(config.environment).capabilities).serverBrowser === true && scopes.includes('preview:operate');
const reportedOf = new WeakMap<T3Client, ReportedProfiles>();

/** startBrowserProfileReporting's report, over the focused environment and the fleet's connected ones. */
export function reportBrowserProfiles(client: T3Client, native: Native | null | undefined, source: EnvironmentFleet = fleet): void {
  if (!native?.available || !client.preferencesLoaded) return;
  let reported = reportedOf.get(client);
  if (!reported) { reported = new Map(); reportedOf.set(client, reported); }
  const input = profileReport(client);
  const configs = new Map<string, object>(), calls = new Map<string, (payload: Obj) => Promise<unknown>>();
  if (client.ready && client.connection === 'connected' && client.environmentId && operable(client.config, client.scopes)) {
    configs.set(client.environmentId, client.config);
    calls.set(client.environmentId, payload => client.rpc(native, 'preview.reportProfiles', payload));
  }
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || !entry.environmentId || entry.environmentId === client.environmentId || !operable(entry.config, entry.scopes)) continue;
    configs.set(entry.environmentId, entry.config);
    calls.set(entry.environmentId, async payload => {
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method: 'preview.reportProfiles', payload, generation: entry.generation });
      if (!reply.ok) throw new Error(str(reply.error?.message));
    });
  }
  for (const environmentId of takeDueProfileReports(reported, configs, input)) {
    void calls.get(environmentId)!({ profiles: input.profiles, defaultProfileId: input.defaultProfileId, recordingFrameRate: input.recordingFrameRate })
      .catch(error => { if (letGo(error)) throw error; });
  }
}
