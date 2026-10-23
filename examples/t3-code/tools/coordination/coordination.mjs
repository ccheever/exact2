#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import { Coordination, defaultRoot } from './state.mjs';

export async function main(argv, overrides) {
  const [verb, ...rest] = argv;
  const separator = rest.indexOf('--');
  const command = separator < 0 ? [] : rest.splice(separator).slice(1);
  const options = {};
  while (rest.length) {
    const key = rest.shift();
    if (!key.startsWith('--') || !rest.length) throw new Error(`expected --name value: ${key}`);
    options[key.slice(2)] = rest.shift();
  }
  const required = key => options[key] ?? (() => { throw new Error(`--${key} required`); })();
  if (!verb || verb === 'help') return { state: defaultRoot, commands: [
    'init --thread COORDINATOR',
    'register --issue ENG-N --thread WORKER',
    'bind-process-host --thread COORDINATOR --proof FILE (verified live legacy launcher on this boot)',
    'status',
    'grant-publish --task ID --thread COORDINATOR',
    'publish --task ID --thread WORKER -- VERIFY COMMAND...',
    'release-publish --task ID --thread WORKER',
    'allocate --task ID --thread WORKER --output target/PATH --budget-gib N',
    'build --task ID --thread WORKER --output target/PATH --budget-gib N -- COMMAND...',
    'build --task ID --thread WORKER --existing-output target/PATH --budget-gib N -- COMMAND...',
    'resource-claim --task ID --thread WORKER --kind port|storage --value HOST:IDENTIFIER',
    'resource-release --task ID --thread WORKER --kind port|storage --value HOST:IDENTIFIER',
    'run --task ID --thread WORKER -- COMMAND... (foreground processes only)',
    'process-audit --task ID --thread WORKER_OR_COORDINATOR --proof FILE',
    'stop --task ID --thread WORKER --pid PID',
    'preserve --task ID --thread WORKER --evidence FILE (repeat for more files)',
    'cleanup --task ID --thread WORKER',
    'recover --task ID --thread COORDINATOR --proof FILE',
    'retire --task ID --thread COORDINATOR --proof FILE',
    'device-enroll --thread COORDINATOR --host HOST --udid UDID --class phone|ipad --runtime RUNTIME --inventory FILE --origin created|audited-unused',
    'device-enroll --thread COORDINATOR --task ID --host HOST --udid UDID --class phone|ipad --runtime RUNTIME --inventory FILE --origin created (assign exact new device; inventory must include matching class/runtime)',
    'device-claim --task ID --thread WORKER --host HOST --class phone|ipad --runtime RUNTIME',
    'device-release --task ID --thread WORKER --host HOST --udid UDID --inventory FILE',
    'device-surplus --thread COORDINATOR --proof FILE (released local simulator; retain the compatible pool)',
    'device-retire --thread COORDINATOR --host HOST --udid UDID --inventory FILE',
    'recover-mutex (only after inspecting its owner.json; dead recorded process required)',
  ] };
  const c = new Coordination(options.worktree ?? process.cwd(), overrides);
  const task = () => required('task'), thread = () => required('thread');
  const load = key => JSON.parse(readFileSync(required(key), 'utf8'));
  switch (verb) {
    case 'init': return c.init(thread());
    case 'register': return c.register(required('issue'), thread());
    case 'bind-process-host': return c.bindProcessHost(thread(), load('proof'));
    case 'status': return c.status();
    case 'grant-publish': return c.grant(task(), thread());
    case 'release-publish': return c.releaseTurn(task(), thread());
    case 'publish': return c.publish(task(), thread(), command);
    case 'allocate': return c.recordOutput(task(), thread(), required('output'), Number(required('budget-gib')));
    case 'build': return c.run(task(), thread(), 'native-build', command, { output: options.output, existingOutput: options['existing-output'], budgetGiB: Number(required('budget-gib')) });
    case 'resource-claim': return c.resource(task(), thread(), required('kind'), required('value'));
    case 'resource-release': return c.resource(task(), thread(), required('kind'), required('value'), true);
    case 'run': return c.run(task(), thread(), 'process', command);
    case 'process-audit': return c.auditRun(task(), thread(), load('proof'));
    case 'stop': return c.stop(task(), thread(), Number(required('pid')));
    case 'preserve': return c.preserve(task(), thread(), [required('evidence')]);
    case 'cleanup': return c.cleanup(task(), thread());
    case 'recover': return c.recover(task(), thread(), load('proof'));
    case 'retire': return c.retire(task(), thread(), load('proof'));
    case 'device-enroll': {
      const origin = required('origin');
      if (!['created', 'audited-unused'].includes(origin)) throw new Error('device origin must attest creation or explicit ownership audit');
      return c.enroll(thread(), { host: required('host'), udid: required('udid'), class: required('class'), runtime: required('runtime') }, load('inventory'), origin === 'created', options.task);
    }
    case 'device-claim': return c.claimDevice(task(), thread(), required('host'), required('class'), required('runtime'));
    case 'device-release': return c.releaseDevice(task(), thread(), required('host'), required('udid'), load('inventory'));
    case 'device-surplus': return c.designateDeviceSurplus(thread(), load('proof'));
    case 'device-retire': return c.retireDevice(thread(), required('host'), required('udid'), load('inventory'));
    case 'recover-mutex': return c.recoverMutex();
    default: throw new Error(`unknown command ${verb}; run help`);
  }
}
if (import.meta.main) {
  try {
    const result = await main(process.argv.slice(2));
    console.log(JSON.stringify(result, null, 2));
    if (result?.code !== undefined) process.exitCode = result.code ?? 1;
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
