// upstream 365aa87982 apps/mobile/src/lib/appLinking.ts and Stack.tsx linking paths.
// @ref llp/1106.003-pairing-and-transport.decision.md#app-links
const aliases: Record<string, string> = {
  '/settings/new-threads': '/settings/server/SettingsEnvironmentNewThreads',
  '/settings/source-control': '/settings/server/SettingsEnvironmentSourceControl',
  '/settings/agent-behavior': '/settings/server/SettingsEnvironmentAgentBehavior',
  '/settings/maintenance': '/settings/server/SettingsEnvironmentMaintenance',
  '/settings/environment-new': '/connections/new',
  '/settings/keyboard': '/settings/preferences/SettingsKeyboard',
  '/settings/follow-ups': '/settings/preferences/SettingsFollowUp',
  '/settings/project-grouping': '/settings/preferences/SettingsProjectGrouping',
  '/settings/organization': '/settings/preferences/SettingsOrganization',
  '/settings/scheduled-tasks': '/settings/scheduled',
  '/settings/scheduled-tasks/new': '/settings/scheduled/new',
};

const publicPath = /^(?:\/|\/connections(?:\/new)?|\/connect-onboarding|\/settings(?:\/(?:environments(?:\/[^/]+)?|new-threads|source-control|agent-behavior|provider-accounts|maintenance|notifications|thread-preferences|about|environment-new|archive|appearance|project-grouping|organization|project|keyboard|follow-ups|scheduled-tasks(?:\/new)?|client-storage|diagnostics|open-source-licenses(?:\/[^/]+)?|usage|legal|auth|waitlist))?|\/threads\/[^/]+\/[^/]+(?:\/(?:terminal|devices|browser|review|review-comment|files(?:\/.+)?|attachments\/[^/]+|git(?:\/(?:commit|branches))?|git-confirm))?|\/new(?:\/(?:draft(?:\/(?:environment|branch|files\/.+|attachments\/[^/]+|settings))?|add-project(?:\/(?:repository|destination|local|new))?))?)$/;

/** Exact supplies a normalized location. It has already removed the scheme and fragment. */
export function mobileAppLink(input: string, requestRoute: string, cold = false) {
  const result = (location: string, ignore = false) => ({ location, ignore, requestRoute });
  if (input.includes('expo-development-client') || /^\/expo-sharing(?:[/?]|$)/.test(input)) return result('/', !cold);
  // Exact maps both a scheme-only wake and a Home link to '/'. Preserve the current
  // stack on warm delivery. An explicit in-app Home action still uses the router.
  if (/^\/+$/.test(input)) return result('/', !cold);
  if (!input.startsWith('/') || input.startsWith('//') || /[\r\n\\]/.test(input)) return result('/unmatched-link');
  const queryAt = input.indexOf('?');
  const path = (queryAt < 0 ? input : input.slice(0, queryAt)).replace(/\/+$/, '') || '/';
  try { path.split('/').forEach(part => decodeURIComponent(part)); } catch { return result('/unmatched-link'); }
  const query = queryAt < 0 ? '' : input.slice(queryAt);
  if (!publicPath.test(path)) return result('/unmatched-link');
  const reviewComment = /^(\/threads\/[^/]+\/[^/]+)\/review-comment$/.exec(path);
  if (reviewComment) return result(`${reviewComment[1]}/review/comment${query}`);
  const alias = aliases[path];
  if (alias) return result(alias + query);
  const file = /^\/threads\/([^/]+)\/([^/]+)\/files\/(.+)$/.exec(path);
  if (file) {
    try {
      const encoded = file.slice(1).map(value => encodeURIComponent(decodeURIComponent(value!)));
      return result(`/threads/${encoded[0]}/${encoded[1]}/files/${encoded[2]}${query}`);
    } catch { return result('/unmatched-link'); }
  }
  return result(path + query);
}
