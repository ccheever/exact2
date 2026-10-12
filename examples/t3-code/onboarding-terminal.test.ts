import { expect, test } from 'bun:test';
import { OnboardingTerminal, resolveOnboardingProviderInstallCommand as install, resolveOnboardingProviderLoginCommand as login, type SetupSession } from './onboarding-terminal';
import type { Obj } from './domain';
const session: SetupSession = { environmentId: 'env', terminalId: 'onboarding-codex-fixture', driver: 'codex', providerInstanceId: 'custom', cwd: '/tmp', command: 'printf fixture' };
function deferred() { let resolve = () => {}; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; }
test('resolves native installer commands for the environment platform', () => {
  expect(install('claudeAgent', 'darwin')).toBe('curl -fsSL https://claude.ai/install.sh | bash');
  expect(install('codex', 'linux')).toBe('curl -fsSL https://chatgpt.com/codex/install.sh | sh');
  expect(install('codex', 'windows')).toBe('irm https://chatgpt.com/codex/install.ps1 | iex');
  expect(install('claudeAgent', 'windows')).toBe('irm https://claude.ai/install.ps1 | iex');
});
test('uses selected instance binary and quotes shell metacharacters', () => {
  const provider = { instanceId: 'custom', driver: 'codex' };
  const settings = (binaryPath: string) => ({ providerInstances: { custom: { config: { binaryPath } } } });
  expect(login(provider, settings('/safe/bin/codex'), 'darwin')).toBe('/safe/bin/codex login');
  expect(login(provider, settings("~/my cli/it's-codex"), 'darwin')).toBe(`~/'my cli/it'"'"'s-codex' login`);
  expect(login(provider, settings('C:\\Program Files\\codex.exe'), 'windows')).toBe("& 'C:\\Program Files\\codex.exe' login");
  expect(login({ driver: 'claudeAgent', instanceId: 'claude' }, {}, 'darwin')).toBe('claude auth login');
  expect(login(provider, settings('bad; echo nope'), 'unknown')).toBe('codex login');
});
test('pretypes a command without Enter and closes with history deletion', async () => {
  const calls: { method: string; input: Obj }[] = [];
  const terminal = new OnboardingTerminal(async (_, method, input) => { calls.push({ method, input }); });
  await terminal.open(session);
  expect(terminal.state.kind).toBe('ready');
  expect(calls.map(call => call.method)).toEqual(['terminal.open', 'terminal.write']);
  expect(calls[1]?.input.data).toBe('printf fixture');
  expect(calls[0]?.input.providerInstanceId).toBe('custom');
  await terminal.close();
  expect(calls[2]?.input.deleteHistory).toBe(true);
});
test('cancelled open cannot write into or close its replacement', async () => {
  const gate = deferred(), calls: string[] = [];
  const terminal = new OnboardingTerminal(async (_, method, input) => { calls.push(`${method}:${input.terminalId}`); if (method === 'terminal.open' && input.terminalId === session.terminalId) await gate.promise; });
  const first = terminal.open(session); await Promise.resolve();
  const second = terminal.open({ ...session, terminalId: 'replacement' });
  gate.resolve(); await first; await second;
  expect(calls).toEqual([`terminal.open:${session.terminalId}`, `terminal.close:${session.terminalId}`, 'terminal.open:replacement', 'terminal.write:replacement']);
});
test('open failure retries, write failure leaves the terminal available', async () => {
  let fail = 'terminal.open';
  const terminal = new OnboardingTerminal(async (_, method) => { if (method === fail) throw Error('fixture'); });
  await terminal.open(session); expect(terminal.view()).toMatchObject({ retry: true, ready: false });
  fail = 'terminal.write'; await terminal.open(session); expect(terminal.view()).toMatchObject({ retry: false, ready: true });
  expect(terminal.view().status).toBe('Run `printf fixture` in this terminal.');
});

// Original providerReadiness.logic.test.ts cases (MIT), with plain decoded settings.
test('uses the selected Codex account binary', () => {
  expect(login({ driver: 'codex', instanceId: 'work' }, { providerInstances: { work: { config: { binaryPath: '/opt/codex-work/bin/codex' } } } }, 'linux')).toBe('/opt/codex-work/bin/codex login');
});
test('uses the selected Claude account binary', () => {
  expect(login({ driver: 'claudeAgent', instanceId: 'work' }, { providerInstances: { work: { config: { binaryPath: '/opt/claude-work/bin/claude' } } } }, 'linux')).toBe('/opt/claude-work/bin/claude auth login');
});
const legacyLogin = (binaryPath: string, platform: string, driver = 'codex') => login({ driver, instanceId: driver }, { providers: { [driver]: { binaryPath } } }, platform);
test('quotes a Codex path with spaces for PowerShell', () => { expect(legacyLogin('C:\\Program Files\\Codex & Tools\\codex.exe', 'windows')).toBe("& 'C:\\Program Files\\Codex & Tools\\codex.exe' login"); });
test('quotes a Claude path with shell metacharacters on POSIX', () => { expect(legacyLogin('/opt/Claude Tools/$current/claude', 'linux', 'claudeAgent')).toBe("'/opt/Claude Tools/$current/claude' auth login"); });
test.each([
  ['~/my tools/codex', "~/'my tools/codex' login"], ['~\\my tools/codex', "~/'my tools/codex' login"],
  ["~/tools/codex's build", `~/'tools/codex'"'"'s build' login`], ["~\\tools\\codex's build", `~/'tools\\codex'"'"'s build' login`],
  ['~/tools/codex; echo unsafe', "~/'tools/codex; echo unsafe' login"],
])('keeps the home prefix expandable while quoting %s', (path, expected) => { expect(legacyLogin(path, 'linux')).toBe(expected); });
test.each(['darwin', 'linux'])('quotes backslashes in a Codex path on %s', platform => { expect(legacyLogin('/opt/codex\\work/codex', platform)).toBe("'/opt/codex\\work/codex' login"); });
test('keeps a plain Windows path unquoted', () => { expect(legacyLogin('C:\\Tools\\codex.exe', 'windows')).toBe('C:\\Tools\\codex.exe login'); });
test('uses the default command when an old server reports an unknown shell', () => { expect(legacyLogin('/opt/Codex Tools/codex', 'unknown')).toBe('codex login'); });
test('uses the PowerShell installer on Windows environments', () => {
  expect(install('codex', 'windows')).toBe('irm https://chatgpt.com/codex/install.ps1 | iex');
  expect(install('claudeAgent', 'windows')).toBe('irm https://claude.ai/install.ps1 | iex');
});
test.each(['darwin', 'linux', 'unknown'])('uses the shell installer on %s', platform => {
  expect(install('codex', platform)).toBe('curl -fsSL https://chatgpt.com/codex/install.sh | sh');
  expect(install('claudeAgent', platform)).toBe('curl -fsSL https://claude.ai/install.sh | bash');
});
