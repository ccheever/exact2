// The manual update path of a server that cannot update itself (T3 Code MIT, see LICENSE-T3;
// reference 1e2ecbd975, commit 3b0093d716: packages/contracts/src/environment.ts
// ServerInstallation with ForwardCompatibleOptional, apps/web/src/versionSkew.ts
// manualServerUpdateCommand, apps/web/src/components/ServerUpdateAction.tsx).
// Port changes: Effect schemas become plain decoders; useCopyToClipboard becomes the
// `copyText` native op, whose failure carries ClipboardWriteError's message.
import { obj, str } from './domain';

/** Proven ownership for a manual update; unknown installs omit this descriptor. */
export type ServerInstallation = { kind: 'npx' | 'pnpm-dlx' | 'bunx' } | { kind: 'npm-global'; prefix: string };

/**
 * ForwardCompatibleOptional(ServerInstallation): a kind this build does not know, or an
 * npm-global without a prefix, decodes as absent instead of failing the config.
 */
export function decodeServerInstallation(raw: unknown): ServerInstallation | undefined {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) return undefined;
  const value = obj(raw), kind = value.kind;
  if (kind === 'npx' || kind === 'pnpm-dlx' || kind === 'bunx') return { kind };
  if (kind === 'npm-global' && typeof value.prefix === 'string' && value.prefix.trim()) return { kind, prefix: value.prefix.trim() };
  return undefined;
}

/** The installation a server config advertises (`environment.capabilities.serverInstallation`). */
export const configInstallation = (config: unknown): ServerInstallation | undefined =>
  decodeServerInstallation(obj(obj(obj(config).environment).capabilities).serverInstallation);

/** manualServerUpdateCommand: the command to hand users whose server cannot update itself. */
export function manualServerUpdateCommand(targetVersion: string, installation?: ServerInstallation): string {
  if (installation?.kind === 'npm-global') {
    const prefix = `'${installation.prefix.split("'").join("'\\''")}'`;
    return `npm install --global --prefix ${prefix} t3@${targetVersion}`;
  }
  const runner = installation?.kind === 'pnpm-dlx' ? 'pnpm dlx' : installation?.kind === 'bunx' ? 'bunx' : 'npx';
  return `${runner} t3@${targetVersion}`;
}

/** ServerUpdateAction's sentence where a desktop-managed server without remote app updates would show a button. */
export const DESKTOP_MANAGED_NOTE = 'Update the desktop app on that machine to update this server.';

/** True when the server's version belongs to a desktop app that cannot be updated from here. */
export function desktopManagedOnly(capabilities: unknown): boolean {
  const value = obj(capabilities);
  return str(value.serverSelfUpdate) === 'desktop-managed' && value.desktopAppUpdate !== true;
}

/** ServerUpdateAction's label: the manual copy labels when `selfUpdate` is absent, else the caller's label. */
export function serverUpdateActionLabel(selfUpdate: string, installation: ServerInstallation | undefined, label = 'Update'): string {
  if (selfUpdate) return label;
  return installation?.kind === 'npm-global' ? 'Copy update command' : 'Copy relaunch command';
}

/** The icon form's accessible name: `<label> for <server label>`. */
export const serverUpdateAriaLabel = (actionLabel: string, serverLabel: string) => `${actionLabel} for ${serverLabel}`;

export type ManualUpdateCopy = { command: string; label: string; title: string; description: string; failureTitle: string; failureMessage: string };

/** Everything the manual path shows: the command, the label, the success toast and the failure toast. */
export function manualUpdateCopy(targetVersion: string, installation: ServerInstallation | undefined, serverLabel: string): ManualUpdateCopy {
  const command = manualServerUpdateCommand(targetVersion, installation), update = installation?.kind === 'npm-global';
  return {
    command,
    label: serverUpdateActionLabel('', installation),
    title: update ? 'Update command copied' : 'Relaunch command copied',
    description: update
      ? `Run \`${command}\` on ${serverLabel}, then restart t3 with your usual options.`
      : `Stop t3 on ${serverLabel}, then relaunch with \`${command}\` using the same subcommand and options. This does not update an installed t3 command.`,
    failureTitle: 'Could not copy update command',
    // useCopyToClipboard's ClipboardWriteError, its target named as the reference names it.
    failureMessage: `Failed to copy ${update ? 'update command' : 'relaunch command'} to the clipboard.`,
  };
}
