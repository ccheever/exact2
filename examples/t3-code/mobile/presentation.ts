// upstream 365aa87982 ConnectionEnvironmentRow.tsx / EnvironmentMachineSymbol.tsx.
// @ref llp/1106.003-pairing-and-transport.decision.md#decision
import { mobileSnapshot } from './client';
import { mobileTheme, withAlpha } from './design';
import { statusText } from './shared/connections';

const machineSymbols: Record<string, string> = {
  server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer',
  laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio',
};

export function connectionView(snapshot: Awaited<ReturnType<typeof mobileSnapshot>>, scheme: string) {
  const theme = mobileTheme(scheme), colors = theme.colors;
  return {
    nativeAvailable: snapshot.nativeAvailable, revision: snapshot.revision,
    ready: snapshot.ready, busy: snapshot.busy, error: snapshot.error,
    environmentId: snapshot.environmentId, environmentLabel: snapshot.environmentLabel,
    status: snapshot.statusMessage, routing: snapshot.routing, projectCount: snapshot.projects.length, threadCount: snapshot.threads.length,
    environments: snapshot.environments.map(row => {
      const unsupported = row.state === 'unsupported', enabled = row.enabled && !unsupported;
      const phase = enabled || unsupported ? row.state : 'available';
      const retrying = enabled && ['connecting', 'reconnecting'].includes(phase);
      const neutral = ['available', 'unsupported'].includes(phase);
      const statusColor = phase === 'connected' ? colors.statusConnected : retrying ? colors.statusRetrying
        : neutral ? colors.iconMuted : colors.dangerForeground;
      return {
        id: row.key, label: row.label, url: row.origin, enabled: row.enabled, unsupported,
        status: !row.enabled && !unsupported ? 'Off' : statusText(phase, row.error),
        statusColor, statusHaloColor: withAlpha(statusColor, retrying ? 0.5 : neutral ? 0.42 : 0.48),
        machineSymbol: machineSymbols[row.machine] ?? machineSymbols.server,
        relayManaged: row.relayManaged, hasError: enabled && !!row.error, retrying,
      };
    }),
  };
}
