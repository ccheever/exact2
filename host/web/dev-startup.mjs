import {spawn} from 'node:child_process';

async function boundedExit(exit) {
  let timer;
  try { await Promise.race([exit, new Promise(ok => { timer = setTimeout(ok, 1000); })]); }
  finally { clearTimeout(timer); }
}

/** Own startup builds before the resident loop has initialized its shutdown state. */
export function startupBuilds() {
  let child = null, exit = null, cancelled = false, stopping = null, residentStop = null;
  const stop = () => {
    if (cancelled) return stopping;
    cancelled = true;
    stopping = (async () => {
      if (residentStop) return residentStop();
      const build = child;
      if (build) {
        const kill = signal => { try { process.kill(-build.pid, signal); } catch (error) { if (error.code !== 'ESRCH') throw error; } };
        kill('SIGTERM');
        await boundedExit(exit.catch(() => {}));
        // The launcher can exit before Cargo or a tool that ignored SIGTERM.
        kill('SIGKILL');
        await boundedExit(exit.catch(() => {}));
      }
      process.exit(0);
    })();
    return stopping;
  };
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, stop);
  return {
    async run(command, args, options) {
      if (cancelled) await stopping;
      child = spawn(command, args, {...options, detached:true});
      exit = new Promise((ok, reject) => { child.once('error', reject); child.once('exit', code => ok(code ?? 1)); });
      const code = await exit;
      if (cancelled) await stopping;
      child = null; exit = null;
      return code;
    },
    handoff(stop) { residentStop = stop; },
  };
}
