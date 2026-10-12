// T3 Code 1e2ecbd975, providerReadiness.logic and AgentInstallTerminal (MIT).
import { obj, str, type Obj } from './domain';
export type OnboardingDriver = 'claudeAgent' | 'codex';
export function resolveOnboardingProviderInstallCommand(driver: OnboardingDriver, platform: string): string {
  const root = driver === 'claudeAgent' ? 'https://claude.ai' : 'https://chatgpt.com/codex';
  return platform === 'windows' ? `irm ${root}/install.ps1 | iex` : `curl -fsSL ${root}/install.sh | ${driver === 'claudeAgent' ? 'bash' : 'sh'}`;
}
function quoteBinary(binary: string, fallback: string, platform: string): string {
  if (/^[A-Za-z0-9_./:\\-]+$/.test(binary) && (platform === 'windows' || !binary.includes('\\'))) return binary;
  if (platform === 'windows') return `& '${binary.replaceAll("'", "''")}'`;
  if (platform !== 'darwin' && platform !== 'linux') return fallback;
  const quote = (value: string) => `'${value.replaceAll("'", `'"'"'`)}'`;
  return binary.startsWith('~/') || binary.startsWith('~\\') ? `~/${quote(binary.slice(2))}` : quote(binary);
}
export function resolveOnboardingProviderLoginCommand(provider: Obj, settings: Obj, platform: string): string {
  const driver = str(provider.driver);
  if (driver !== 'claudeAgent' && driver !== 'codex') return driver;
  const instance = obj(settings.providerInstances)[str(provider.instanceId)];
  const config = instance ? obj(obj(instance).config) : obj(obj(settings.providers)[driver]);
  const fallback = driver === 'claudeAgent' ? 'claude' : 'codex';
  const binary = typeof config.binaryPath === 'string' && config.binaryPath.trim() ? config.binaryPath : fallback;
  return `${quoteBinary(binary, fallback, platform)}${driver === 'claudeAgent' ? ' auth' : ''} login`;
}
export type SetupSession = { environmentId: string; terminalId: string; driver: OnboardingDriver; providerInstanceId: string; cwd: string; command: string };
export type SetupState = { kind: 'closed' } | { kind: 'preparing' | 'ready' | 'openFailed' | 'writeFailed'; session: SetupSession };
export const ONBOARDING_THREAD = 'onboarding-agent-setup';
export class OnboardingTerminal {
  state: SetupState = { kind: 'closed' };
  private queue: Promise<void> = Promise.resolve();
  private generation = 0;
  constructor(private readonly request: (environmentId: string, method: string, input: Obj) => Promise<unknown>) {}
  open(session: SetupSession): Promise<void> {
    const previous = this.state;
    const generation = ++this.generation;
    this.state = { kind: 'preparing', session };
    this.queue = this.queue.then(async () => {
      if (previous.kind !== 'closed') await this.dispose(previous.session);
      if (generation !== this.generation) return;
      try { await this.request(session.environmentId, 'terminal.open', { threadId: ONBOARDING_THREAD, terminalId: session.terminalId, cwd: session.cwd, providerInstanceId: session.providerInstanceId }); }
      catch { if (generation === this.generation) this.state = { kind: 'openFailed', session }; return; }
      if (generation !== this.generation) return;
      try {
        await this.request(session.environmentId, 'terminal.write', { threadId: ONBOARDING_THREAD, terminalId: session.terminalId, data: session.command });
        if (generation === this.generation) this.state = { kind: 'ready', session };
      } catch { if (generation === this.generation) this.state = { kind: 'writeFailed', session }; }
    });
    return this.queue;
  }
  close(): Promise<void> {
    const previous = this.state;
    ++this.generation;
    this.state = { kind: 'closed' };
    this.queue = this.queue.then(async () => { if (previous.kind !== 'closed') await this.dispose(previous.session); });
    return this.queue;
  }
  private async dispose(session: SetupSession): Promise<void> {
    await this.request(session.environmentId, 'terminal.close', { threadId: ONBOARDING_THREAD, terminalId: session.terminalId, deleteHistory: true }).catch(() => {});
  }
  view() {
    const state = this.state, session = state.kind === 'closed' ? null : state.session;
    return { open: !!session, ready: state.kind === 'ready' || state.kind === 'writeFailed', retry: state.kind === 'openFailed',
      status: state.kind === 'ready' ? 'Review the command, then press Enter to run it.' : state.kind === 'writeFailed' ? `Run \`${session?.command}\` in this terminal.` : state.kind === 'openFailed' ? 'Could not open the setup terminal.' : 'Preparing command...',
      environmentId: session?.environmentId ?? '', terminalId: session?.terminalId ?? '', driver: session?.driver ?? '', cwd: session?.cwd ?? '', providerInstanceId: session?.providerInstanceId ?? '', threadId: ONBOARDING_THREAD };
  }
}
